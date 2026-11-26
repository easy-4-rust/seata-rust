"""Negative checks on owned copies of actual candidate runs, never primary artifacts."""
from pathlib import Path
import hashlib,json,os,shutil,tempfile,unittest
import evidence
class ActualSharedLockCandidateGuards(unittest.TestCase):
 def setUp(self):
  source=Path(os.environ.get('SEATA_SHARED_LOCK_CANDIDATE_EVIDENCE',evidence.ROOT/'docs/phase-0/evidence/local-java-go-at-shared-lock-candidate/run_20261113_074509_ba63c0d4'));self.temporary=tempfile.TemporaryDirectory(prefix='shared-lock-positive-',dir='D:/Temp');self.root=Path(self.temporary.name);assert self.root.resolve().is_relative_to(Path('D:/Temp').resolve());self.addCleanup(self.temporary.cleanup)
  def translate(value):
   if isinstance(value,str):return value.replace(str(source),str(self.root)).replace(source.as_posix(),self.root.as_posix())
   if isinstance(value,list):return [translate(v) for v in value]
   if isinstance(value,dict):return {k:translate(v) for k,v in value.items()}
   return value
  for path in source.rglob('*'):
   if not path.is_file():continue
   relative=path.relative_to(source)
   if relative.parts[0]=='source-inputs' or path.suffix=='.exe':continue
   target=self.root/relative;target.parent.mkdir(parents=True,exist_ok=True)
   if path.suffix=='.json':target.write_text(json.dumps(translate(json.loads(path.read_bytes()))),encoding='utf-8')
   elif path.suffix=='.log':target.write_text(path.read_text(encoding='utf-8',errors='replace').replace(source.as_posix(),self.root.as_posix()),encoding='utf-8')
   else:shutil.copyfile(path,target)
 def read(self):return evidence.derive(self.root)
 def change_json(self,name,fn):
  path=self.root/name;value=json.loads(path.read_bytes());fn(value);path.write_text(json.dumps(value),encoding='utf-8')
 def change_frame(self,actor,code,fn):
  def change(events):
   for event in events:
    if event['event']!='wire' or event['payload_type']!=code:continue
    frame=bytearray(bytes.fromhex(event['frame_hex']));head=int.from_bytes(frame[7:9],'big');fn(frame,head);event.update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  self.change_json(actor+'/proxy-events.json',change)
 def test_actual_control(self):self.read()
 def test_initial_business_row_required(self):
  (self.root/'initial.tsv').write_text('1\t999\n0\n',encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_conflict_must_not_leave_local_effect(self):
  self.change_json('go/blocked.json',lambda v:v.update(balance=800))
  with self.assertRaises(Exception):self.read()
 def test_blocked_database_must_keep_only_java_undo(self):
  path=self.root/'blocked.tsv';path.write_text(path.read_text()+path.read_text().splitlines()[1]+'\n',encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_explicit_lock_conflict_code_cannot_be_io(self):
  def fn(frame,head):
   if frame[head+2]==0:frame[-9]=3
  self.change_frame('go',12,fn)
  with self.assertRaises(Exception):self.read()
 def test_new_transaction_cannot_reuse_rejected_xid(self):
  blocked=json.loads((self.root/'go/blocked.json').read_bytes());self.change_json('go/phase-one.json',lambda v:v.update(xid=blocked['xid']))
  with self.assertRaises(Exception):self.read()
 def test_resource_identity_must_match_actual_java(self):
  self.change_json('case.json',lambda v:v.update(shared_resource=v['shared_resource']+'-other'))
  with self.assertRaises(Exception):self.read()
 def test_lock_key_must_match_actual_java(self):
  self.change_json('case.json',lambda v:v.update(java_lock_key='ACCOUNTS:1;'))
  with self.assertRaises(Exception):self.read()
 def test_duplicate_open_must_reject(self):
  self.change_json('go/duplicate-open.json',lambda v:v.update(rejected=False))
  with self.assertRaises(Exception):self.read()
 def test_phase_one_report_ack_must_be_success(self):
  self.change_frame('go',14,lambda frame,head:frame.__setitem__(-1,1))
  with self.assertRaises(Exception):self.read()
 def test_new_begin_waits_for_rejected_transaction_end_ack(self):
  def fn(events):
   ack=next(row for row in events if row['event']=='wire' and row['payload_type']==10);events.remove(ack);idx=[i for i,row in enumerate(events) if row['event']=='wire' and row['payload_type']==1][1];events.insert(idx+1,ack)
   for i,row in enumerate(events):row['sequence']=i+1
  self.change_json('go/proxy-events.json',fn)
  with self.assertRaisesRegex(RuntimeError,'New Go Begin precedes acknowledged termination'):self.read()
 def test_report_ack_must_precede_actual_phase_two(self):
  def fn(events):
   ack=next(row for row in events if row['event']=='wire' and row['payload_type']==14);events.remove(ack);idx=next(i for i,row in enumerate(events) if row['event']=='wire' and row['payload_type'] in [3,5]);events.insert(idx+1,ack)
   for i,row in enumerate(events):row['sequence']=i+1
  self.change_json('go/proxy-events.json',fn)
  with self.assertRaisesRegex(RuntimeError,'One acknowledged original phase-one report before actual callback'):self.read()
 def test_go_original_callback_reply_must_succeed(self):
  action=json.loads((self.root/'case.json').read_bytes())['go_end_action'];code=4 if action=='commit' else 6;self.change_frame('go',code,lambda frame,head:frame.__setitem__(-1,0))
  with self.assertRaises(Exception):self.read()
 def test_java_client_protocol_version_not_normalized(self):
  self.change_frame('java',101,lambda frame,head:frame.__setitem__(2,1))
  with self.assertRaises(Exception):self.read()
 def test_final_undo_must_be_empty(self):
  (self.root/'final.tsv').write_text('1\t900\n1\n',encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_native_query_must_not_use_target_as_lock_owner(self):
  self.change_json('query-go-held.json',lambda v:v.update(observer_xid=v['xid']))
  with self.assertRaises(Exception):self.read()
 def test_original_tc_lock_manager_source_required(self):
  path=self.root/'tc/tc-stdout.log';lines=path.read_text(encoding='utf-8',errors='replace').splitlines();path.write_text('\n'.join(line.split(' source: ')[0]+' source: file:/D:/wrong/' if ' org.apache.seata.server.storage.file.lock.FileLockManager source: ' in line else line for line in lines),encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_compiled_java_actor_bytes_bound(self):
  path=self.root/'compiled/SharedLockJavaActor.class';path.write_bytes(path.read_bytes()+b'x')
  with self.assertRaises(Exception):self.read()
if __name__=='__main__':unittest.main()
