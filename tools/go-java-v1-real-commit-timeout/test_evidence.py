"""Mutate real evidence copies while preserving frame hashes and event sequence legality."""
import copy,hashlib,json,os,shutil,tempfile,unittest
from pathlib import Path
from evidence import derive
from verify import validate

class MixedRawGuards(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='seata-commit-unit-',dir='D:/Temp');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
  source=Path(os.environ.get('SEATA_COMMIT_TIMEOUT_EVIDENCE','D:/codex-builds/real-commit-timeout-trial-20261113/run_20261113_004336_3570c478'))
  names=['phase-one.json','runtime.json','initial/tc-stdout.log','operator_loss/tc-stdout.log']+[n+'.tsv' for n in ['timeout','restart','lost','second_restart','final']]+['console-'+n+'.json' for n in ['timeout','restart','lost','second-restart','final']]
  for stage in ['initial','operator_loss','operator_replay']:
   names += [stage+'/'+n for n in ['proxy-events.json','proxy-result.json','sdk-stdout.log']]
   if stage!='initial':names.append(stage+'/operator.json')
  for name in names:
   path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/name,path)
 def read(self):return validate(derive(self.root))
 def mutate(self,stage,function):
  path=self.root/stage/'proxy-events.json';events=json.loads(path.read_bytes());function(events)
  for i,e in enumerate(events):e['sequence']=i+1
  path.write_text(json.dumps(events),encoding='utf-8')
 def update_frame(self,event,frame):event.update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest(),id=int.from_bytes(frame[12:16],'big',signed=True),kind=frame[9])
 def test_actual_control(self):self.read()
 def test_empty_sql(self):
  (self.root/'restart.tsv').write_bytes(b'')
  with self.assertRaises(RuntimeError):self.read()
 def test_unrelated_operator_failure(self):
  (self.root/'operator_loss/tc-stdout.log').write_bytes(b'unrelated failure')
  with self.assertRaisesRegex(RuntimeError,'First operator lacks exact original TCC response timeout'):self.read()
 def test_double_effect(self):
  path=self.root/'final.tsv';text=path.read_text(encoding='utf-8').replace('commit_account\t1\t0\t10','commit_account\t1\t0\t20').replace('\tcommitted\t1','\tcommitted\t2');path.write_text(text,encoding='utf-8')
  with self.assertRaisesRegex(RuntimeError,'TCC business was not committed exactly once'):self.read()
 def test_at_undo_not_removed(self):
  phase=json.loads((self.root/'phase-one.json').read_bytes());path=self.root/'lost.tsv'
  with path.open('a',encoding='utf-8') as stream:stream.write('undo\t'+phase['xid']+'\t'+str(phase['branch_id'])+'\t0\t'+phase['undo_hex']+'\n')
  with self.assertRaisesRegex(RuntimeError,'Real AT commit did not clean Undo'):self.read()
 def test_rm_registration_missing(self):
  self.mutate('operator_replay',lambda events:events.__setitem__(slice(None),[e for e in events if e.get('payload_type') not in [103,104]]))
  with self.assertRaisesRegex(RuntimeError,'Callback precedes actual resource identification'):self.read()
 def test_wrong_commit_request_id(self):
  def change(events):
   for event in events:
    if event.get('payload_type')==4:
     frame=bytearray.fromhex(event['frame_hex']);frame[12:16]=(999).to_bytes(4,'big');self.update_frame(event,frame)
  self.mutate('operator_replay',change)
  with self.assertRaisesRegex(RuntimeError,'Commit response correlation differs'):self.read()
 def test_wrong_commit_envelope(self):
  def change(events):
   for event in events:
    if event.get('payload_type')==3:
     frame=bytearray.fromhex(event['frame_hex']);frame[9]=2;self.update_frame(event,frame)
  self.mutate('operator_replay',change)
  with self.assertRaises(RuntimeError):self.read()
 def test_loss_in_wrong_stage(self):
  self.mutate('operator_loss',lambda events:events.__setitem__(slice(None),[e for e in events if e['event']!='successful-tcc-response-lost']))
  def add(events):
   row=next(e for e in events if e.get('payload_type')==4);events.append(dict(event='successful-tcc-response-lost',connection=row['connection'],id=row['id'],frame_hex=row['frame_hex']))
  self.mutate('operator_replay',add)
  with self.assertRaisesRegex(RuntimeError,'First operator must lose only one TCC success'):self.read()
 def test_replay_before_operator(self):
  def move(events):
   command=next(e for e in events if e['event']=='operator-command-start');events.remove(command);events.append(command)
  self.mutate('operator_replay',move)
  with self.assertRaisesRegex(RuntimeError,'Retry16 replayed before operator command'):self.read()
 def test_fake_console16(self):
  path=self.root/'console-restart.json';value=json.loads(path.read_bytes());value['data'][0]['status']=8;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaisesRegex(RuntimeError,'console original16'):self.read()
 def test_no_natural_transition(self):
  (self.root/'initial/tc-stdout.log').write_bytes(b'No natural timeout transition')
  with self.assertRaisesRegex(RuntimeError,'Natural16 scheduler transition missing'):self.read()
 def test_partial_operator_result(self):
  path=self.root/'operator_loss/operator.json';value=json.loads(path.read_bytes());body=json.loads(value['body']);body['code']='200';value['body']=json.dumps(body);path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaisesRegex(RuntimeError,'Actual operator/transport result differs'):self.read()
 def test_duplicate_tcc_response_cannot_replace_at(self):
  phase=json.loads((self.root/'phase-one.json').read_bytes())
  from evidence import wire
  def replace(events):
   tcc=next(e for e in events if e.get('payload_type')==4 and wire.decode(e)['branch_id']==phase['tcc_branch_id']);index=next(i for i,e in enumerate(events) if e.get('payload_type')==4 and wire.decode(e)['branch_id']==phase['branch_id']);changed=copy.deepcopy(tcc)
   old=bytearray.fromhex(changed['frame_hex']);head=int.from_bytes(old[7:9],'big');extra=b'\x00\x01x\x00\x00';frame=old[:head]+extra+old[head:];frame[3:7]=len(frame).to_bytes(4,'big');frame[7:9]=(head+len(extra)).to_bytes(2,'big');self.update_frame(changed,frame);events[index]=changed
  self.mutate('operator_loss',replace)
  with self.assertRaises(RuntimeError):self.read()

if __name__=='__main__':unittest.main()
