"""Loopback-only RM observer; duplicate one original callback or lose one final reply and close its session."""
import argparse
import hashlib
import datetime
import json
import os
from pathlib import Path
import socket
import struct
import threading
import time

MAX_FRAME=65536


def exact(source,size,clean=False):
    reader=getattr(source,'recv',None) or source.read
    result=bytearray()
    while len(result)<size:
        part=reader(size-len(result))
        if not part:
            if clean and not result:return None
            raise EOFError('Truncated native frame')
        result.extend(part)
    return bytes(result)


def read_frame(source):
    fixed=exact(source,16,True)
    if fixed is None:return None
    full,head=struct.unpack('>IH',fixed[3:9])
    if fixed[:3]!=b'\xda\xda\x01' or not 16<=head<=full<=MAX_FRAME or fixed[9]>4 or fixed[10:12]!=b'\x01\x00':
        raise ValueError('Invalid bounded native envelope')
    if (fixed[9] in [3,4] and full!=head) or (fixed[9] not in [3,4] and full<head+2):raise ValueError('Invalid native body length')
    return fixed+exact(source,full-16)


def identity(payload,at):
    if at+2>len(payload):raise ValueError('Missing identity length')
    length=struct.unpack('>h',payload[at:at+2])[0];at+=2
    if length<=0 or at+length+8>len(payload):raise ValueError('Invalid original identity')
    xid=payload[at:at+length].decode('utf-8');at+=length
    branch=struct.unpack('>q',payload[at:at+8])[0]
    if branch<=0:raise ValueError('Invalid original branch ID')
    return xid,branch,at+8


class CallbackRule:
    def __init__(self,mode,role='RM'):
        if mode not in ['basic','duplicate','reconnect']:raise ValueError('Unknown RM proxy scenario')
        if role not in ['RM','TM'] or role=='TM' and mode!='basic':raise ValueError('TM observation must be passthrough')
        self.mode=mode;self.role=role;self.roles={};self.callbacks={};self.held={};self.duplicate_key=None
        self.faults=0;self.released=0;self.closed=0;self.lock=threading.Lock()

    def complete(self,connections):
        return self.faults==0 if self.mode=='basic' else self.faults==1 and (self.released==1 if self.mode=='duplicate' else self.closed==1 and connections>=2)

    def inspect(self,direction,connection,frame):
        if len(frame)<16:raise ValueError('Missing native header')
        full,head=struct.unpack('>IH',frame[3:9]);request_type=frame[9];request_id=struct.unpack('>i',frame[12:16])[0]
        if full!=len(frame) or not 16<=head<=full<=MAX_FRAME:raise ValueError('Invalid observed frame length')
        payload=frame[head:];kind=struct.unpack('>H',payload[:2])[0] if len(payload)>=2 else None
        event=dict(event='frame',role=self.role,direction=direction,connection=connection,request_type=request_type,request_id=request_id,kind=kind,frame_hex=frame.hex(),action='forward',delivery_count=1)
        with self.lock:
            if self.role=='TM':
                if direction=='c2s' and request_type==0 and kind==101:self.roles[connection]='TM'
                allowed=(request_type in [3,4] or direction=='c2s' and request_type==0 and kind in [101,1,7,9,15] or direction=='s2c' and request_type==1 and kind in [102,2,8,10,16])
                if not allowed:raise ValueError('Unexpected message on the isolated TM observer')
                return event,[frame],False
            if direction=='c2s' and request_type==0 and kind==103:self.roles[connection]='RM'
            if direction=='c2s' and request_type==0 and kind==101:raise ValueError('TM must use its separate direct endpoint')
            key=(connection,request_id)
            if direction=='s2c' and request_type==0 and kind in [3,5]:
                if len(self.callbacks)>=256 or connection not in self.roles:raise ValueError('Unregistered or over-budget callback session')
                xid,branch,_=identity(payload,2)
                original=self.callbacks.get(key)
                if original and original['payload']!=payload:raise ValueError('Callback ID changed original identity/body')
                self.callbacks[key]=dict(kind=kind,xid=xid,branch=branch,payload=payload)
                if self.mode=='duplicate' and self.faults==0:
                    self.faults=1;self.duplicate_key=key;event.update(action='duplicate_original',delivery_count=2)
                    return event,[frame,frame],False
            if direction=='c2s' and request_type==1 and kind in [4,6]:
                original=self.callbacks.get(key)
                if not original or original['kind']+1!=kind:raise ValueError('Reply has no original same-connection callback')
                if payload[2:4]==b'\x01\x00':
                    xid,branch,end=identity(payload,4)
                    if end+1!=len(payload) or (xid,branch)!=(original['xid'],original['branch']):raise ValueError('Reply changed original identity')
                    terminal=payload[-1]==(5 if kind==4 else 8)
                else:terminal=False
                if self.mode=='duplicate' and key==self.duplicate_key:
                    if key not in self.held:
                        self.held[key]=frame;event.update(action='hold',delivery_count=0)
                        return event,[],False
                    first=self.held.pop(key)
                    if first!=frame:raise ValueError('Duplicate delivery replies disagree')
                    if self.released:raise ValueError('More replies than injected copies')
                    self.released=1;event.update(action='release_original',delivery_count=1,released_frame_hex=first.hex())
                    return event,[first],False
                if self.mode=='reconnect' and self.faults==0 and terminal:
                    self.faults=1;self.closed=1;event.update(action='drop_and_close',delivery_count=0)
                    return event,[],True
            return event,[frame],False


class Sink:
    def __init__(self,path):
        self.stream=Path(path).open('x',encoding='utf-8',newline='\n');self.lock=threading.Lock();self.rows=0;self.raw_bytes=0
    def emit(self,row):
        with self.lock:
            count=len(row.get('frame_hex',''))//2
            if self.rows>=2048 or self.raw_bytes+count>16*1024*1024:raise ValueError('Observer evidence budget exceeded')
            # 本机Python3.12的monotonic只有15.625ms；高分辨率QPC避免把先后报文记成相同时刻。
            # Windows跨进程perf_counter依据 https://docs.python.org/3.12/library/time.html#time.perf_counter
            row=dict(time_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),monotonic_ns=time.perf_counter_ns(),index=self.rows,**row)
            self.rows+=1;self.raw_bytes+=count;self.stream.write(json.dumps(row,separators=(',',':'))+'\n');self.stream.flush()
    def close(self):self.stream.close()


def expected_peer_reset(error,authorization):
    if not isinstance(error,(ConnectionResetError,ConnectionAbortedError)) or authorization is None:return None
    deadline=time.monotonic()+1
    while True:
        try:raw=Path(authorization).read_bytes();value=json.loads(raw);break
        except PermissionError:
            if time.monotonic()>=deadline:return None
            time.sleep(.005)
        except (OSError,ValueError):return None
    if set(value)!={'phase','writer_pid','tc_pid','monotonic_ns'} or value['phase']!='joint-kill':return None
    if any(type(value[k]) is not int or value[k]<=0 for k in ['writer_pid','tc_pid','monotonic_ns']) or value['writer_pid']==value['tc_pid']:return None
    elapsed=time.perf_counter_ns()-value['monotonic_ns']
    if not 0<=elapsed<=10_000_000_000:return None
    return hashlib.sha256(raw).hexdigest()


class ProxyServer:
    def __init__(self,listen,upstream,mode,events,stop_file,role='RM',expected_peer_exit_file=None):
        self.expected_peer_exit_file=expected_peer_exit_file
        if expected_peer_exit_file is not None and Path(expected_peer_exit_file).resolve().drive.upper() not in ['D:','E:']:raise ValueError('Peer exit control escaped development drives')
        self.upstream=upstream;self.rule=CallbackRule(mode,role);self.sink=Sink(events);self.stop_file=Path(stop_file)
        self.errors=[];self.sessions=[];self.sockets=[];self.lock=threading.Lock()
        self.listener=socket.socket();self.listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);self.listener.bind(('127.0.0.1',listen));self.listener.listen(8);self.listener.settimeout(.2)
        clock=time.get_clock_info('perf_counter')
        self.sink.emit(dict(event='ready',role=role,pid=os.getpid(),listen_port=self.listener.getsockname()[1],upstream_port=upstream,mode=mode,
                            clock=dict(name='perf_counter_ns',implementation=clock.implementation,resolution_seconds=clock.resolution,monotonic=clock.monotonic,adjustable=clock.adjustable)))

    def session(self,client,connection):
        closing=threading.Event();upstream=None
        def close_sockets():
            closing.set()
            for stream in [client,upstream]:
                if stream:
                    try:stream.shutdown(socket.SHUT_RDWR)
                    except OSError:pass
        try:
            with client,socket.create_connection(('127.0.0.1',self.upstream),timeout=3) as upstream:
                client.settimeout(35);upstream.settimeout(35)
                with self.lock:self.sockets.extend([client,upstream])
                def pump(source,target,direction):
                    raw=None
                    try:
                        while not closing.is_set():
                            raw=read_frame(source)
                            if raw is None:break
                            row,sends,close=self.rule.inspect(direction,connection,raw);self.sink.emit(row)
                            for forwarded in sends:target.sendall(forwarded)
                            if close:
                                self.sink.emit(dict(event='fault_close',connection=connection));close_sockets();break
                    except Exception as error:
                        if not closing.is_set():
                            authorization=expected_peer_reset(error,self.expected_peer_exit_file)
                            if authorization is not None:self.sink.emit(dict(event='expected_peer_reset',direction=direction,connection=connection,error_type=type(error).__name__,authorization_sha256=authorization))
                            else:self.errors.append(str(error));self.sink.emit(dict(event='error',direction=direction,connection=connection,message=str(error),frame_hex=raw.hex() if raw else ''))
                    finally:close_sockets()
                threads=[threading.Thread(target=pump,args=(client,upstream,'c2s')),threading.Thread(target=pump,args=(upstream,client,'s2c'))]
                for thread in threads:thread.start()
                for thread in threads:thread.join()
        except Exception as error:
            self.errors.append(str(error));self.sink.emit(dict(event='error',connection=connection,message=str(error)))
        finally:close_sockets()

    def serve(self):
        start=time.monotonic();connections=0
        try:
            while not self.stop_file.exists():
                if time.monotonic()-start>50:raise TimeoutError('Owned RM proxy run budget expired')
                try:client,_=self.listener.accept()
                except socket.timeout:continue
                connections+=1
                if connections>8:client.close();raise ValueError('RM connection budget exhausted')
                thread=threading.Thread(target=self.session,args=(client,connections));thread.start();self.sessions.append(thread)
        except Exception as error:self.errors.append(str(error));self.sink.emit(dict(event='error',message=str(error)))
        finally:
            self.listener.close()
            with self.lock:
                for stream in self.sockets:
                    try:stream.shutdown(socket.SHUT_RDWR)
                    except OSError:pass
            for thread in self.sessions:thread.join(timeout=4)
            if any(thread.is_alive() for thread in self.sessions):self.errors.append('Owned RM proxy session not joined')
            expected=self.rule.complete(connections)
            succeeded=not self.errors and expected and not self.rule.held and bool(self.rule.roles)
            self.sink.emit(dict(event='summary',role=self.rule.role,succeeded=succeeded,mode=self.rule.mode,connections=connections,faults=self.rule.faults,released=self.rule.released,closed=self.rule.closed,errors=self.errors))
            self.sink.close()
        if not succeeded:raise RuntimeError('RM proxy scenario was not fully observed')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--listen-port',type=int,required=True);parser.add_argument('--upstream-port',type=int,required=True)
    parser.add_argument('--mode',choices=['basic','duplicate','reconnect'],required=True);parser.add_argument('--events',type=Path,required=True);parser.add_argument('--stop-file',type=Path,required=True)
    parser.add_argument('--role',choices=['RM','TM'],default='RM');parser.add_argument('--expected-peer-exit-file',type=Path)
    args=parser.parse_args();ProxyServer(args.listen_port,args.upstream_port,args.mode,args.events,args.stop_file,args.role,args.expected_peer_exit_file).serve()
