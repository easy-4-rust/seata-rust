"""Owned loopback wire proxy: unchanged TC, real SDK loop, explicit fault/provenance records."""
import hashlib
import json
from pathlib import Path
import socket
import struct
import threading
import time

def frame_with_body(frame,body):
    head=int.from_bytes(frame[7:9],'big');out=bytearray(frame[:head]+body);out[3:7]=len(out).to_bytes(4,'big');return bytes(out)

def mutate(frame,mode):
    head=int.from_bytes(frame[7:9],'big');body=frame[head:]
    if mode in ['short-result','wrong-child-type']:
        if len(body)!=18 or body[:2]!=b'\x00\x3c' or body[6:8]!=b'\x00\x02' or body[8:10]!=b'\x00\x10' or body[13:15]!=b'\x00\x10':raise RuntimeError('Need exact two-status Java merge before fault')
        if mode=='short-result':return frame_with_body(frame,b'\x00\x3c'+struct.pack('>iH',7,1)+body[8:13])
        changed=bytearray(body);changed[13:15]=b'\x00\x08';return frame_with_body(frame,changed)
    out=bytearray(frame)
    if mode=='foreign-id':out[12:16]=(int.from_bytes(frame[12:16],'big')+10000).to_bytes(4,'big')
    elif mode=='request-envelope':out[9]=0
    elif mode=='oneway-envelope':out[9]=2
    elif mode=='branch-reply-type':return frame_with_body(frame,b'\x00\x04\x01\x00\x00\x00'+b'\x00'*9)
    else:raise RuntimeError('Unknown proxy fault')
    return bytes(out)

class Connection:
    def __init__(self,number,client,backend):self.number=number;self.client=client;self.backend=backend;self.closed=False;self.lock=threading.Lock()
    def close(self):
        with self.lock:
            if self.closed:return
            self.closed=True
            for stream in [self.client,self.backend]:
                try:stream.shutdown(socket.SHUT_RDWR)
                except OSError:pass
                stream.close()

class SdkProxy:
    def __init__(self,runtime,evidence):
        self.runtime=Path(runtime);self.evidence=Path(evidence);self.stop_event=threading.Event();self.lock=threading.Lock();self.events=[];self.errors=[];self.connections=[];self.threads=[]
        self.batch_count=0;self.ordinary_faults=0;self.modes={};self.forced_disconnect=False
        self.listener=socket.socket();self.listener.bind(('127.0.0.1',28092));self.listener.listen(4);self.listener.settimeout(.2)
    def record(self,**row):
        with self.lock:row['sequence']=len(self.events)+1;self.events.append(row)
    def marker(self,name):
        path=self.runtime/name
        if not path.exists():path.write_bytes(b'owned proxy observation\n')
    def start(self):self.launch(self.accept);self.launch(self.watch_disconnect)
    def launch(self,function,*args):
        thread=threading.Thread(target=function,args=args,daemon=True);self.threads.append(thread);thread.start()
    def accept(self):
        while not self.stop_event.is_set():
            try:client,address=self.listener.accept()
            except socket.timeout:continue
            except OSError:return
            if address[0]!='127.0.0.1':client.close();continue
            try:backend=socket.create_connection(('127.0.0.1',28091),timeout=3)
            except OSError as error:client.close();self.errors.append(repr(error));continue
            client.settimeout(.2);backend.settimeout(.2)
            connection=Connection(len(self.connections)+1,client,backend);self.connections.append(connection)
            self.record(event='connected',connection=connection.number,client_port=address[1])
            self.launch(self.pump,connection,True);self.launch(self.pump,connection,False)
    def watch_disconnect(self):
        while not self.stop_event.wait(.02):
            if not self.forced_disconnect and (self.runtime/'disconnect.request').exists():
                if not self.connections:self.errors.append('Disconnect without connection');return
                self.forced_disconnect=True;self.record(event='forced-disconnect',connection=1);self.connections[0].close()
    def send(self,connection,stream,data):
        for part in [data[:1],data[1:15],data[15:]]:
            if part:stream.sendall(part);time.sleep(.002)
    def wait_observed_unknown(self,label,connection):
        deadline=time.monotonic()+5
        while not (self.runtime/('unknown-'+label+'.ready')).exists():
            if connection.closed or self.stop_event.is_set():raise RuntimeError('Connection closed before unknown acknowledgment')
            if time.monotonic()>deadline:raise RuntimeError('SDK never reported typed unknown for '+label)
            time.sleep(.01)
    def process(self,connection,to_tc,frame):
        head=int.from_bytes(frame[7:9],'big');body=frame[head:];kind=frame[9];request_id=int.from_bytes(frame[12:16],'big',signed=True);payload_type=int.from_bytes(body[:2],'big') if body else None
        self.record(event='wire',connection=connection.number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=request_id,kind=kind,payload_type=payload_type,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
        if to_tc:
            if kind in [3,4]:
                connection.backend.sendall(frame);return
            mode='normal'
            if payload_type==59:
                self.batch_count+=1;mode={2:'short-result',3:'wrong-child-type',4:'foreign-id'}.get(self.batch_count,'normal')
            elif payload_type==15 and self.batch_count>=4 and self.ordinary_faults<3:
                mode=['request-envelope','oneway-envelope','branch-reply-type'][self.ordinary_faults];self.ordinary_faults+=1
            self.modes[(connection.number,request_id)]=mode
            # 固定TC的ProtocolDetectHandler会拒绝不足magic/version的首包；该独立上游问题保留。
            # 本场景只将TC->SDK方向故意分片，不能声称双向任意分片已经兼容。
            connection.backend.sendall(frame);return
        mode=self.modes.get((connection.number,request_id),'normal')
        if kind==1:
            if mode=='normal':
                self.send(connection,connection.client,frame+frame);self.record(event='duplicate/coalesced',connection=connection.number,id=request_id)
            else:
                bad=mutate(frame,mode);self.send(connection,connection.client,bad)
                self.record(event='injected',connection=connection.number,id=request_id,mode=mode,frame_hex=bad.hex())
                if mode!='foreign-id':self.wait_observed_unknown(mode,connection)
                self.send(connection,connection.client,frame);self.record(event='late-valid' if mode!='foreign-id' else 'correct-id-followup',connection=connection.number,id=request_id,mode=mode)
        else:self.send(connection,connection.client,frame)
        if payload_type==102:
            if len(body)<3 or body[2]!=1:raise RuntimeError('Actual TC did not identify SDK TM')
            self.marker(f'registered{connection.number}.ready')
        if kind==4:self.marker(f'heartbeat{connection.number}.ready')
    def pump(self,connection,to_tc):
        stream=connection.client if to_tc else connection.backend;pending=bytearray()
        try:
            while not self.stop_event.is_set() and not connection.closed:
                try:data=stream.recv(32768)
                except socket.timeout:continue
                if not data:return
                pending.extend(data)
                while len(pending)>=16:
                    total=int.from_bytes(pending[3:7],'big');head=int.from_bytes(pending[7:9],'big')
                    if pending[:3]!=b'\xda\xda\x01' or not 16<=head<=total<=8*1024*1024 or pending[10:12]!=b'\x01\x00':raise RuntimeError('Invalid actual SDK/TC frame')
                    if len(pending)<total:break
                    frame=bytes(pending[:total]);del pending[:total];self.process(connection,to_tc,frame)
        except (OSError,RuntimeError) as error:
            if not connection.closed and not self.stop_event.is_set():
                if isinstance(error,OSError) and (self.runtime/'sdk-finished.ready').exists():self.record(event='sdk-exit-close',connection=connection.number)
                else:self.errors.append(repr(error))
        finally:connection.close()
    def close(self):
        self.stop_event.set();self.listener.close()
        for connection in self.connections:connection.close()
        for thread in list(self.threads):thread.join(timeout=1)
        (self.evidence/'proxy-events.json').write_text(json.dumps(self.events,indent=2)+'\n',encoding='utf-8')
        (self.evidence/'proxy-result.json').write_text(json.dumps(dict(connections=len(self.connections),forced_disconnect=self.forced_disconnect,batches=self.batch_count,ordinary_faults=self.ordinary_faults,errors=self.errors),indent=2)+'\n',encoding='utf-8')
