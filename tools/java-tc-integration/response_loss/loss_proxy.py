"""Loopback-only test proxy: withhold one correlated successful mutating response; never rewrite frames."""
import argparse
import datetime
import json
import os
from pathlib import Path
import socket
import struct
import threading

MAX_FRAME = 8*1024*1024


def exact(source, size, clean_eof=False):
    read = getattr(source,'recv',None) or source.read
    out = bytearray()
    while len(out)<size:
        part=read(size-len(out))
        if not part:
            if clean_eof and not out: return None
            raise EOFError('Truncated native frame')
        out.extend(part)
    return bytes(out)


def read_frame(source):
    header=exact(source,16,True)
    if header is None: return None
    full=struct.unpack('>I',header[3:7])[0]
    head=struct.unpack('>H',header[7:9])[0]
    if header[:3]!=b'\xda\xda\x01' or not 16<=head<=full<=MAX_FRAME or header[9]>4:
        raise ValueError('Invalid V1 envelope')
    if header[9] in (3,4):
        if full!=head: raise ValueError('Unexpected heartbeat body')
    elif full<head+2 or header[10:12]!=b'\x01\x00':
        raise ValueError('Unsupported native payload envelope')
    return header+exact(source,full-16)


def string16(payload, at):
    if len(payload)<at+2: raise ValueError('Truncated XID length')
    length=struct.unpack('>h',payload[at:at+2])[0]
    if length<0 or len(payload)<at+2+length: raise ValueError('Invalid XID field')
    return payload[at+2:at+2+length].decode('utf-8')


class DropRule:
    def __init__(self, response_kind):
        if response_kind not in (12,8,10): raise ValueError('Only BranchRegister/Commit/Rollback loss is supported')
        self.response_kind=response_kind
        self.drop_count=0
        self.roles={}
        self.requests={}
        self.request_rows=[]
        self.lock=threading.Lock()

    def inspect(self,direction,connection,frame):
        head=struct.unpack('>H',frame[7:9])[0]
        request_type=frame[9]
        request_id=struct.unpack('>I',frame[12:16])[0]
        payload=frame[head:]
        kind=struct.unpack('>H',payload[:2])[0] if len(payload)>=2 else None
        with self.lock:
            xid=None;branch_id=None;status=None
            if direction=='c2s' and request_type==0:
                if kind in (101,103): self.roles[connection]='TM' if kind==101 else 'RM'
                if kind in (7,9,11,15,17,21): xid=string16(payload,2)
                self.requests[(connection,request_id)]=(kind,xid)
                self.request_rows.append({'connection':connection,'role':self.roles.get(connection),'kind':kind,'request_id':request_id,'xid':xid})
            pending=self.requests.get((connection,request_id)) if direction=='s2c' and request_type==1 else None
            if pending: xid=pending[1]
            success=len(payload)>=4 and payload[2:4]==b'\x01\x00'
            if direction=='s2c' and request_type==0 and kind in (3,5):
                xid=string16(payload,2)
                at=4+len(xid.encode('utf-8'))
                if len(payload)<at+8: raise ValueError('Truncated callback identity')
                branch_id=struct.unpack('>q',payload[at:at+8])[0]
            if direction=='c2s' and request_type==1 and kind in (4,6) and success:
                xid=string16(payload,4)
                at=6+len(xid.encode('utf-8'))
                if len(payload)<at+8: raise ValueError('Truncated callback result identity')
                branch_id=struct.unpack('>q',payload[at:at+8])[0]
            if direction=='s2c' and request_type==1 and success:
                if kind==2: xid=string16(payload,4)
                if kind==12 and len(payload)==12: branch_id=struct.unpack('>q',payload[4:])[0]
                if kind in (8,10,16,18) and len(payload)==5: status=payload[4]
            valid=branch_id is not None and branch_id>0 if kind==12 else status==(9 if kind==8 else 11) if kind in (8,10) else False
            dropped=bool(direction=='s2c' and request_type==1 and kind==self.response_kind and self.drop_count==0
                         and pending and pending[0]==kind-1 and success and valid)
            if dropped: self.drop_count+=1
            return {'event':'frame','direction':direction,'connection':connection,'role':self.roles.get(connection),
                    'request_type':request_type,'kind':kind,'request_id':request_id,'xid':xid,
                    'branch_id':branch_id,'global_status':status,'dropped':dropped,'frame_hex':frame.hex()}


class EventSink:
    def __init__(self,path=None):
        self.rows=[]
        self.lock=threading.Lock()
        self.stream=Path(path).open('x',encoding='utf-8',newline='\n') if path else None

    def emit(self,row):
        row={'time_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),**row}
        with self.lock:
            self.rows.append(row)
            if self.stream:
                self.stream.write(json.dumps(row,separators=(',',':'))+'\n');self.stream.flush()

    def close(self):
        if self.stream: self.stream.close()


class ProxyServer:
    def __init__(self,listen_port,upstream_port,response_kind,sink,io_timeout=15):
        self.sink=sink
        self.upstream_port=upstream_port
        self.timeout=io_timeout
        self.rule=DropRule(response_kind)
        self.errors=[]
        self.listener=socket.socket()
        self.listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
        self.listener.bind(('127.0.0.1',listen_port));self.listener.listen(4);self.listener.settimeout(io_timeout)
        self.port=self.listener.getsockname()[1]
        self.sink.emit({'event':'ready','pid':os.getpid(),'port':self.port,'upstream_port':upstream_port,'drop_response_kind':response_kind})

    def session(self,client,connection):
        try:
            with client, socket.create_connection(('127.0.0.1',self.upstream_port),timeout=3) as upstream:
                client.settimeout(self.timeout);upstream.settimeout(self.timeout)
                def pump(source,destination,direction):
                    try:
                        while True:
                            frame=read_frame(source)
                            if frame is None: break
                            event=self.rule.inspect(direction,connection,frame);self.sink.emit(event)
                            if not event['dropped']: destination.sendall(frame)
                    except Exception as error:
                        self.errors.append(str(error));self.sink.emit({'event':'error','connection':connection,'direction':direction,'message':str(error)})
                    finally:
                        try: destination.shutdown(socket.SHUT_WR)
                        except OSError: pass
                threads=[threading.Thread(target=pump,args=(client,upstream,'c2s')),
                         threading.Thread(target=pump,args=(upstream,client,'s2c'))]
                for thread in threads: thread.start()
                for thread in threads: thread.join()
        except Exception as error:
            self.errors.append(str(error));self.sink.emit({'event':'error','connection':connection,'message':str(error)})

    def serve(self):
        sessions=[]
        try:
            # The probe opens exactly TM+RM. Readiness checks must inspect the listener, not consume a slot.
            for connection in range(2):
                client,_=self.listener.accept()
                thread=threading.Thread(target=self.session,args=(client,connection));thread.start();sessions.append(thread)
        except Exception as error:
            self.errors.append(str(error));self.sink.emit({'event':'error','message':str(error)})
        finally:
            self.listener.close()
            for thread in sessions: thread.join()
            succeeded=not self.errors and self.rule.drop_count==1 and set(self.rule.roles.values())=={'TM','RM'}
            self.sink.emit({'event':'summary','succeeded':succeeded,'drop_count':self.rule.drop_count,
                            'roles':sorted(self.rule.roles.values()),'requests':self.rule.request_rows,'errors':self.errors})
        if not succeeded: raise RuntimeError('Proxy did not complete the exact loss scenario')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--listen-port',required=True,type=int)
    parser.add_argument('--upstream-port',required=True,type=int)
    parser.add_argument('--drop-response-kind',required=True,type=int,choices=[12,8,10])
    parser.add_argument('--events',required=True,type=Path)
    args=parser.parse_args()
    sink=EventSink(args.events)
    try: ProxyServer(args.listen_port,args.upstream_port,args.drop_response_kind,sink).serve()
    finally: sink.close()
