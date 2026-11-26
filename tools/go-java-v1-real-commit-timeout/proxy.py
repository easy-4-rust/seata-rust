"""Only withhold real Commit requests or one actual TCC Commit success response."""
import hashlib,importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('commit_proxy_parent',ROOT/'tools/go-java-v1-real-at/proxy.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
spec=importlib.util.spec_from_file_location('commit_wire',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)

class CommitProxy(base.RealAtProxy):
 def __init__(self,runtime,evidence,mode):
  super().__init__(runtime,evidence);self.mode=mode;self.lost=False;self.requests={}
 def start(self):self.launch(self.accept)
 def process(self,connection,to_tc,frame):
  head=int.from_bytes(frame[7:9],'big');body=frame[head:];code=int.from_bytes(body[:2],'big') if body else None;identifier=int.from_bytes(frame[12:16],'big',signed=True)
  event=dict(event='wire',connection=connection.number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=identifier,kind=frame[9],payload_type=code,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest());self.record(**event)
  if not to_tc and code==3:
   self.requests[(connection.number,identifier)]=wire.decode(event)
   if self.mode=='deny-commit':self.record(event='commit-request-withheld',connection=connection.number,id=identifier,frame_hex=frame.hex());return
  if to_tc and code==4 and self.mode=='lose-tcc-ack' and not self.lost:
   request=self.requests.get((connection.number,identifier))
   if request is None:raise RuntimeError('Actual Commit response lacks original request')
   if request['branch_type']==1:
    result=wire.decode(event)
    if result['result']!=1 or result['status']!=5 or (result['xid'],result['branch_id'])!=(request['xid'],request['branch_id']):raise RuntimeError('Loss requires actual successful original TCC response')
    self.lost=True;self.record(event='successful-tcc-response-lost',connection=connection.number,id=identifier,frame_hex=frame.hex());return
  self.send(connection,connection.backend if to_tc else connection.client,frame)
  if not to_tc and code==104:
   if body[2]!=1:raise RuntimeError('Actual RM registration rejected')
   self.marker('rm-identified.ready')
