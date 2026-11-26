"""Drop only actual rollback requests or the first actual successful RM acknowledgement."""
import hashlib,importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('timeout_proxy_parent',ROOT/'tools/go-java-v1-real-at/proxy.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)

class TimeoutProxy(base.RealAtProxy):
 def __init__(self,runtime,evidence,mode):
  super().__init__(runtime,evidence);self.mode=mode;self.lost=False;self.requests={}
 def start(self):self.launch(self.accept)
 def process(self,connection,to_tc,frame):
  head=int.from_bytes(frame[7:9],'big');body=frame[head:];code=int.from_bytes(body[:2],'big') if body else None;identifier=int.from_bytes(frame[12:16],'big',signed=True)
  self.record(event='wire',connection=connection.number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=identifier,kind=frame[9],payload_type=code,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  if not to_tc and code==5:
   self.requests[(connection.number,identifier)]=frame
   if self.mode=='deny-rollback':
    self.record(event='rollback-request-withheld',connection=connection.number,id=identifier,frame_hex=frame.hex());return
  if to_tc and code==6 and self.mode=='lose-ack' and not self.lost:
   if (connection.number,identifier) not in self.requests or len(body)<4 or body[2]!=1:raise RuntimeError('Loss requires original request and actual successful RM acknowledgement')
   self.lost=True;self.record(event='successful-rollback-response-lost',connection=connection.number,id=identifier,frame_hex=frame.hex());return
  self.send(connection,connection.backend if to_tc else connection.client,frame)
  if not to_tc and code==104:
   if body[2]!=1:raise RuntimeError('Real RM registration rejected')
   self.marker('rm-identified.ready')
