"""Actual SDK/TC response-loss controls; both wire directions deliberately fragmented."""
import hashlib
import importlib.util
import json
from pathlib import Path
import time
import threading
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('inflight_proxy_parent',ROOT/'tools/go-java-v1-sdk-profile/proxy.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
class InflightProxy(base.SdkProxy):
 def __init__(self,runtime,evidence):
  super().__init__(runtime,evidence);self.faults=[];self.rollback_faulted=False;self.send_locks={}
 def send(self,connection,stream,data):
  # 晚到线程与心跳pump共享socket；整帧分片须连续发送，发送锁与连接关闭锁独立。
  key=(connection.number,stream is connection.client)
  with self.lock:lock=self.send_locks.setdefault(key,threading.Lock())
  with lock:super().send(connection,stream,data)
 def start(self):self.launch(self.accept)
 def process(self,connection,to_tc,frame):
  head=int.from_bytes(frame[7:9],'big');body=frame[head:];kind=frame[9];identifier=int.from_bytes(frame[12:16],'big',signed=True);code=int.from_bytes(body[:2],'big') if body else None
  self.record(event='wire',connection=connection.number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=identifier,kind=kind,payload_type=code,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  if to_tc:
   mode='normal'
   if code==1:
    length=int.from_bytes(body[6:8],'big');name=body[8:8+length].decode('ascii')
    if name=='inflight-begin-close':mode='begin-close'
    elif name=='inflight-begin-timeout':mode='begin-timeout'
   elif code==7:mode='commit-close'
   elif code==9 and (self.runtime/'arm-rollback-close.ready').exists() and not self.rollback_faulted:mode='rollback-close';self.rollback_faulted=True
   if kind in [0,2]:self.modes[(connection.number,identifier)]=mode
   self.send(connection,connection.backend,frame);return
  mode=self.modes.get((connection.number,identifier),'normal')
  if kind==1 and mode!='normal':
   if len(body)<4 or body[2]!=1:raise RuntimeError('Fault requires actual successful TC response')
   proof=dict(label=mode,connection=connection.number,id=identifier,tc_response_hex=frame.hex())
   if code==2:
    length=int.from_bytes(body[4:6],'big');xid=body[6:6+length].decode('utf-8')
    if not xid or len(body)<6+length+2:raise RuntimeError('Actual TC XID response is incomplete')
    proof['xid']=xid
   self.faults.append(proof);self.record(event='response-suppressed',**proof)
   (self.runtime/(mode+'.json')).write_text(json.dumps(proof,indent=2)+'\n',encoding='utf-8')
   if mode.endswith('-close'):connection.close()
   else:self.launch(self.late_response,connection,frame,identifier)
   return
  self.send(connection,connection.client,frame)
  if code==102:
   if body[2]!=1:raise RuntimeError('TM registration not accepted')
   self.marker(f'registered{connection.number}.ready')
  if kind==4:self.marker(f'heartbeat{connection.number}.ready')
 def late_response(self,connection,frame,identifier):
  deadline=time.monotonic()+35
  while not (self.runtime/'timeout-observed.ready').exists():
   if self.stop_event.wait(.02) or connection.closed:return
   if time.monotonic()>deadline:self.errors.append('SDK did not observe natural timeout');return
  try:self.send(connection,connection.client,frame);self.record(event='late-original-response',connection=connection.number,id=identifier);self.marker('late-timeout.ready')
  except OSError as error:self.errors.append(repr(error))
 def close(self):
  self.stop_event.set();self.listener.close()
  for connection in self.connections:connection.close()
  for thread in list(self.threads):thread.join(timeout=1)
  (self.evidence/'proxy-events.json').write_text(json.dumps(self.events,indent=2)+'\n',encoding='utf-8')
  (self.evidence/'proxy-result.json').write_text(json.dumps(dict(connections=len(self.connections),faults=self.faults,errors=self.errors,fragmented_directions=['sdk-to-tc','tc-to-sdk']),indent=2)+'\n',encoding='utf-8')
