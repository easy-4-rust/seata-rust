"""Read actual V1 Go and default V2 Java frames without rewriting recorded bytes."""
import hashlib,importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('shared_lock_primitive_reader',ROOT/'tools/go-java-v1-real-at/verify.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);Reader=base.Reader
def decode(row):
 frame=bytes.fromhex(row['frame_hex'])
 if len(frame)<16 or frame[:2]!=b'\xda\xda' or frame[2] not in [1,2] or int.from_bytes(frame[3:7],'big')!=len(frame) or frame[10:12]!=b'\1\0':raise RuntimeError('Actual native frame malformed')
 head=int.from_bytes(frame[7:9],'big');version=frame[2]
 if not 16<=head<=len(frame):raise RuntimeError('Actual head length invalid')
 body=frame[head:];code=int.from_bytes(body[:2],'big') if body else None;kind=frame[9];identifier=int.from_bytes(frame[12:16],'big',signed=True)
 if (row['id'],row['kind'],row['payload_type'],row['sha256'])!=(identifier,kind,code,hashlib.sha256(frame).hexdigest()):raise RuntimeError('Actual metadata/hash differs')
 result=dict(frame=frame,version=version,code=code,kind=kind,id=identifier,connection=row['connection'],sequence=row['sequence'])
 if code is None:
  if kind not in [3,4]:raise RuntimeError('Bodyless non-heartbeat frame')
  return result
 r=Reader(body[2:])
 if code in [101,103]:
  for _ in range(4):r.string()
  if code==103:result['resource']=r.string(4)
 elif code in [102,104]:
  if version==2:
   result['result']=r.integer(1)
   if result['result']==0:result['message']=r.string()
  flag=r.integer(1)
  if flag not in [0,1]:raise RuntimeError('Malformed original identification flag')
  result['identified']=flag==1;result['server_version']=r.string()
 elif code==1:result.update(timeout=r.integer(4,True),name=r.string())
 elif code in [7,9,15]:result.update(xid=r.string(),extra_data=r.string())
 elif code==11:result.update(xid=r.string(),branch_type=r.integer(1),resource=r.string(),lock_key=r.string(4),application_data=r.string(4))
 elif code in [3,5]:result.update(xid=r.string(),branch_id=r.integer(8,True),branch_type=r.integer(1),resource=r.string(),application_data=r.string(4))
 elif code==13:result.update(xid=r.string(),branch_id=r.integer(8,True),status=r.integer(1),resource=r.string(),application_data=r.string(4),branch_type=r.integer(1))
 elif code in [2,4,6,8,10,12,14,16]:
  result['result']=r.integer(1)
  if result['result']==0:result['message']=r.string()
  result['error']=r.integer(1)
  if code==2:result.update(xid=r.string(),extra_data=r.string())
  elif code==12:result['branch_id']=r.integer(8,True)
  elif code in [4,6]:result.update(xid=r.string(),branch_id=r.integer(8,True),status=r.integer(1))
  elif code!=14:result['status']=r.integer(1)
 else:raise RuntimeError('Unexpected original actor message type '+str(code))
 r.end();return result
