"""Read-only native queries on a TM-only channel; never registers a resource or ends a transaction."""
import hashlib,json,socket,struct

def string(value,size=2):
 data=value.encode('utf-8');return len(data).to_bytes(size,'big')+data
def frame(code,identifier,body):
 payload=code.to_bytes(2,'big')+body;return b'\xda\xda\x01'+struct.pack('>IHBBBi',16+len(payload),16,0,1,0,identifier)+payload
def exact(stream,size):
 data=bytearray()
 while len(data)<size:
  part=stream.recv(size-len(data))
  if not part:raise RuntimeError('Truncated actual native query response')
  data.extend(part)
 return bytes(data)
def exchange(stream,code,identifier,body,expected,events):
 request=frame(code,identifier,body);stream.sendall(request);events.append(dict(direction='observer-to-tc',frame_hex=request.hex(),sha256=hashlib.sha256(request).hexdigest()))
 header=exact(stream,16);total=int.from_bytes(header[3:7],'big');head=int.from_bytes(header[7:9],'big')
 if header[:3]!=b'\xda\xda\x01' or not 16<=head<=total<=8*1024*1024 or header[9:12]!=b'\x01\x01\x00' or int.from_bytes(header[12:16],'big',signed=True)!=identifier:raise RuntimeError('Native query envelope/identity differs')
 response=header+exact(stream,total-16);payload=response[head:]
 if len(payload)<3 or int.from_bytes(payload[:2],'big')!=expected:raise RuntimeError('Native query response type differs')
 events.append(dict(direction='tc-to-observer',frame_hex=response.hex(),sha256=hashlib.sha256(response).hexdigest()));return payload[2:]
def query(stream,xid,resource,lock_key,status,lockable,events=None):
 if not xid or not resource or not lock_key or status not in [1,5,15,16,17]:raise RuntimeError('Native observer identity/expectation missing')
 events=[] if events is None else events;register=b''.join(string(s) for s in ['1.5.2','owned-retention-observer','default_tx_group',''])
 identified=exchange(stream,101,1,register,102,events)
 if identified[0]!=1:raise RuntimeError('Native observer TM was rejected')
 state=exchange(stream,15,2,string(xid)+string(''),16,events)
 if state!=bytes([1,0,status]):raise RuntimeError('Original native global status differs')
 # ATCore.lockQuery delegates directly to isLockable; a distinct non-persisted identity avoids a new Begin.
 observer='127.0.0.1:28091:999999999000000001'
 if observer==xid:raise RuntimeError('Lock observer identity must differ from target')
 body=string(observer)+b'\0'+string(resource)+string(lock_key,4)+string('{}',4)
 result=exchange(stream,21,3,body,22,events)
 if result!=bytes([1,0,0,int(lockable)]):raise RuntimeError('Original native lock ownership differs')
 return dict(xid=xid,status=status,resource=resource,lock_key=lock_key,observer_xid=observer,lockable=lockable,target_mutations=0,frames=events)
def observe(xid,resource,lock_key,status,lockable,trace_path=None):
 events=[]
 try:
  with socket.create_connection(('127.0.0.1',28091),timeout=5) as stream:return query(stream,xid,resource,lock_key,status,lockable,events)
 finally:
  if trace_path is not None:
   with trace_path.open('x',encoding='utf-8') as output:output.write(json.dumps(events,indent=2)+'\n')
