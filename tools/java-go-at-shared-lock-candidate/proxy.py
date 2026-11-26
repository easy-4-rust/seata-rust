"""Transparent owned loopback recorder; no synthetic responses or field translation."""
import hashlib,json,socket,threading
from pathlib import Path
class WireProxy:
 def __init__(self,evidence,port):
  self.evidence=Path(evidence);self.listener=socket.socket();self.listener.bind(('127.0.0.1',port));self.listener.listen(8);self.listener.settimeout(.2);self.closed=threading.Event();self.lock=threading.Lock();self.events=[];self.errors=[];self.streams=[];self.threads=[];self.connections=0
 def record(self,**row):
  with self.lock:row['sequence']=len(self.events)+1;self.events.append(row)
 def launch(self,fn,*args):
  thread=threading.Thread(target=fn,args=args,daemon=True);self.threads.append(thread);thread.start()
 def start(self):self.launch(self.accept)
 def accept(self):
  while not self.closed.is_set():
   try:client,address=self.listener.accept()
   except socket.timeout:continue
   except OSError:return
   try:backend=socket.create_connection(('127.0.0.1',28091),timeout=3)
   except OSError as error:client.close();self.errors.append(repr(error));continue
   self.connections+=1;number=self.connections;client.settimeout(.2);backend.settimeout(.2);self.streams.extend([client,backend]);self.record(event='connected',connection=number,backend_port=28091,client_port=address[1]);self.launch(self.pump,number,client,backend,True);self.launch(self.pump,number,backend,client,False)
 def pump(self,number,source,target,to_tc):
  pending=bytearray()
  try:
   while not self.closed.is_set():
    try:raw=source.recv(32768)
    except socket.timeout:continue
    if not raw:
     if pending:raise RuntimeError('EOF within actual frame')
     return
    pending.extend(raw)
    while len(pending)>=16:
     length=int.from_bytes(pending[3:7],'big');head=int.from_bytes(pending[7:9],'big')
     if pending[:2]!=b'\xda\xda' or pending[2] not in [1,2] or not 16<=head<=length<=8*1024*1024 or pending[10:12]!=b'\x01\x00':raise RuntimeError('Invalid actual native envelope')
     if len(pending)<length:break
     frame=bytes(pending[:length]);del pending[:length];body=frame[head:];self.record(event='wire',connection=number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=int.from_bytes(frame[12:16],'big',signed=True),kind=frame[9],payload_type=int.from_bytes(body[:2],'big') if body else None,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest());target.sendall(frame)
  except (OSError,RuntimeError) as error:
   if not self.closed.is_set():self.errors.append(repr(error))
 def close(self):
  self.closed.set();self.listener.close()
  for stream in self.streams:
   try:stream.shutdown(socket.SHUT_RDWR)
   except OSError:pass
   stream.close()
  for thread in list(self.threads):thread.join(timeout=1)
  for name,value in [('proxy-events.json',self.events),('proxy-result.json',dict(connections=self.connections,errors=self.errors,field_translation=False,fault_injection=False))]:
   with (self.evidence/name).open('x',encoding='utf-8') as out:json.dump(value,out,indent=2)
