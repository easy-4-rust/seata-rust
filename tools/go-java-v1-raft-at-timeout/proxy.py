"""Route original SDK connections to the selected owned current leader; preserve all frames."""
import importlib.util,socket
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('raft_at_parent_proxy',ROOT/'tools/go-java-v1-real-at-timeout/proxy.py');parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)

class RaftTimeoutProxy(parent.TimeoutProxy):
 def __init__(self,runtime,evidence,mode,port):
  if port not in [28291,28293,28295]:raise ValueError('Owned native leader port required')
  super().__init__(runtime,evidence,mode);self.backend_port=port
 def accept(self):
  while not self.stop_event.is_set():
   try:client,address=self.listener.accept()
   except socket.timeout:continue
   except OSError:return
   if address[0]!='127.0.0.1':client.close();continue
   try:backend=socket.create_connection(('127.0.0.1',self.backend_port),timeout=3)
   except OSError as error:client.close();self.errors.append(repr(error));continue
   client.settimeout(.2);backend.settimeout(.2)
   connection=parent.base.base.base.Connection(len(self.connections)+1,client,backend);self.connections.append(connection)
   self.record(event='connected',connection=connection.number,client_port=address[1],backend_port=self.backend_port)
   self.launch(self.pump,connection,True);self.launch(self.pump,connection,False)
