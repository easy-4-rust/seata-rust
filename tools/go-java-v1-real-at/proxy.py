"""Transparent bidirectional-fragmenting recorder; no synthetic RM/business responses."""
import hashlib,importlib.util
import time
import socket
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('real_at_proxy_parent',ROOT/'tools/go-java-v1-inflight/proxy.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
class RealAtProxy(base.InflightProxy):
 def __init__(self,runtime,evidence):
  super().__init__(runtime,evidence);self.reconnected=False;self.replayed=False;self.rollback_frame=None;self.replay_pending=False
 def start(self):super().start();self.launch(self.control)
 def pump(self,connection,to_tc):
  stream=connection.client if to_tc else connection.backend;pending=bytearray()
  def planned():return to_tc and (self.runtime/'phase-one-exit.ready').exists()
  try:
   while not self.stop_event.is_set() and not connection.closed:
    try:data=stream.recv(32768)
    except socket.timeout:continue
    if not data:
     if planned():self.record(event='expected-phase-one-process-exit',connection=connection.number,transport='EOF')
     return
    pending.extend(data)
    while len(pending)>=16:
     total=int.from_bytes(pending[3:7],'big');head=int.from_bytes(pending[7:9],'big')
     if pending[:3]!=b'\xda\xda\x01' or not 16<=head<=total<=8*1024*1024 or pending[10:12]!=b'\x01\x00':raise RuntimeError('Invalid actual full-SDK/TC frame')
     if len(pending)<total:break
     frame=bytes(pending[:total]);del pending[:total];self.process(connection,to_tc,frame)
  except (OSError,RuntimeError) as error:
   if not connection.closed and not self.stop_event.is_set():
    if isinstance(error,OSError) and getattr(error,'errno',None) in [104,10054] and planned():self.record(event='expected-phase-one-process-exit',connection=connection.number,transport='reset')
    elif isinstance(error,OSError) and (self.runtime/'sdk-finished.ready').exists():self.record(event='sdk-exit-close',connection=connection.number)
    else:self.errors.append(repr(error))
  finally:connection.close()
 def control(self):
  while not self.stop_event.wait(.01):
   if not self.reconnected and (self.runtime/'reconnect.request').exists():
    self.reconnected=True;self.record(event='forced-reconnect',connection=1);self.connections[0].close()
   if not self.replayed and (self.runtime/'repeat-rollback.request').exists():
    if self.rollback_frame is None:self.errors.append('No actual TC rollback frame to replay');return
    self.replayed=True;self.replay_pending=True;connection=self.connections[-1]
    try:self.send(connection,connection.client,self.rollback_frame);self.record(event='duplicate-tc-request',connection=connection.number,frame_hex=self.rollback_frame.hex())
    except OSError as error:self.errors.append(repr(error));return
 def process(self,connection,to_tc,frame):
  head=int.from_bytes(frame[7:9],'big');body=frame[head:]
  kind=frame[9];code=int.from_bytes(body[:2],'big') if body else None
  if to_tc and self.replay_pending and code==6:
   self.replay_pending=False;self.record(event='duplicate-rm-reply',connection=connection.number,frame_hex=frame.hex());self.marker('repeat-rollback-reply.ready');return
  self.record(event='wire',connection=connection.number,direction='sdk-to-tc' if to_tc else 'tc-to-sdk',id=int.from_bytes(frame[12:16],'big',signed=True),kind=kind,payload_type=code,frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  if not to_tc and code==5:self.rollback_frame=frame
  self.send(connection,connection.backend if to_tc else connection.client,frame)
  if not to_tc and code==104 and connection.number>1:
   if len(body)<3 or body[2]!=1:raise RuntimeError('Actual RM reregistration was rejected')
   self.marker('resource-reregistered.ready')
