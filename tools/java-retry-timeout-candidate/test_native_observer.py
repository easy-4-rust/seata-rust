"""Read-only native observer controls; synthetic streams are unit fixtures only."""
import io,unittest
from native_observer import frame,query
from file_runtime import has_reload_exception

class Stream:
 def __init__(self,data):self.input=io.BytesIO(data);self.written=bytearray()
 def recv(self,size):return self.input.read(min(size,3))
 def sendall(self,data):self.written.extend(data)

def response(code,identifier,body):
 value=bytearray(frame(code,identifier,body));value[9]=1;return bytes(value)

class NativeObserverGuards(unittest.TestCase):
 def data(self,status=17,lock=False):return response(102,1,b'\x01\x00\x00')+response(16,2,bytes([1,0,status]))+response(22,3,bytes([1,0,0,int(lock)]))
 def test_only_registration_status_and_lock_queries(self):
  stream=Stream(self.data());proof=query(stream,'127.0.0.1:28091:123','mysql://owned','orders:1',17,False);self.assertEqual(proof['status'],17);self.assertFalse(proof['lockable']);self.assertEqual(len(proof['frames']),6)
  wire=bytes(stream.written);codes=[]
  while wire:
   size=int.from_bytes(wire[3:7],'big');codes.append(int.from_bytes(wire[16:18],'big'));wire=wire[size:]
  self.assertEqual(codes,[101,15,21])
 def test_wrong_status_rejected(self):
  with self.assertRaises(RuntimeError):query(Stream(self.data(status=15)),'x','r','k',17,False)
 def test_status_request_contains_java_extra_data_field(self):
  stream=Stream(self.data());query(stream,'x','r','k',17,False);data=bytes(stream.written);first=int.from_bytes(data[3:7],'big');second=data[first:];size=int.from_bytes(second[3:7],'big')
  self.assertEqual(second[16:size],bytes.fromhex('000f0001780000'))
 def test_wrong_lock_rejected(self):
  with self.assertRaises(RuntimeError):query(Stream(self.data(lock=True)),'x','r','k',17,False)
 def test_foreign_response_id_rejected(self):
  data=bytearray(self.data());data[12:16]=(99).to_bytes(4,'big')
  with self.assertRaises(RuntimeError):query(Stream(data),'x','r','k',17,False)
 def test_wrong_response_envelope_rejected(self):
  data=bytearray(self.data());data[9]=2
  with self.assertRaises(RuntimeError):query(Stream(data),'x','r','k',17,False)
 def test_truncated_response_rejected(self):
  with self.assertRaises(RuntimeError):query(Stream(self.data()[:-1]),'x','r','k',17,False)
 def test_loading_exception_class_is_not_throwing_it(self):
  self.assertFalse(has_reload_exception(b'[class,load] org.apache.seata.common.exception.ShouldNeverHappenException source: jar:nested:original.jar'))
 def test_actual_reload_exception_is_rejected(self):
  self.assertTrue(has_reload_exception(b'Caused by: org.apache.seata.common.exception.ShouldNeverHappenException: NOT properly handled CommitRetryTimeout'))

if __name__=='__main__':unittest.main()
