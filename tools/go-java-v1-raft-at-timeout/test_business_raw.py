"""Corrupt real evidence copies on D:; never modify certified run artifacts."""
import hashlib,json,os,shutil,tempfile,unittest
from pathlib import Path
from business_evidence import derive
from verify import validate

class RawFileEvidenceGuards(unittest.TestCase):
 def setUp(self):
  temporary=tempfile.TemporaryDirectory(prefix='file-at-evidence-',dir='D:/Temp');self.addCleanup(temporary.cleanup);self.root=Path(temporary.name)
  source=Path(os.environ['SEATA_RAFT_AT_EVIDENCE'])
  files=['phase-one.json','runtime.json','business-tables.txt']
  for name in ['timeout','restart','lost','second_restart','final']:files += [name+'.tsv','console-'+name.replace('_','-')+'.json','native-'+name+'.json','native-'+name+'-frames.json']
  for stage in ['initial','operator_loss','operator_replay']:
   files += [stage+'/'+leaf for leaf in ['proxy-events.json','proxy-result.json','sdk-stdout.log','tc-stdout.log','tc-process.json']]
   if stage!='initial':files.append(stage+'/operator.json')
  for name in files:
   target=self.root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/name,target)
 def read(self):return validate(derive(self.root))
 def mutate(self,stage,action):
  path=self.root/stage/'proxy-events.json';events=json.loads(path.read_bytes());action(events)
  for index,event in enumerate(events):event['sequence']=index+1
  path.write_text(json.dumps(events),encoding='utf-8')
 def change_kind(self,stage,code,kind):
  def action(events):
   target=next(e for e in events if e['event']=='wire' and e['payload_type']==code)
   frame=bytearray.fromhex(target['frame_hex']);frame[9]=kind;target.update(frame_hex=frame.hex(),kind=kind,sha256=hashlib.sha256(frame).hexdigest())
  self.mutate(stage,action)
 def test_actual_control(self):self.read()
 def test_wrong_rollback_request_envelope(self):
  self.change_kind('operator_replay',5,1)
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_rollback_reply_envelope(self):
  self.change_kind('operator_replay',6,0)
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_registration_envelopes(self):
  self.change_kind('operator_replay',103,1);self.change_kind('operator_replay',104,0)
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_branch_registration_envelopes(self):
  self.change_kind('initial',11,1);self.change_kind('initial',12,0)
  with self.assertRaises(RuntimeError):self.read()
 def change_xid(self,code,skip):
  def action(events):
   target=next(e for e in events if e['event']=='wire' and e['payload_type']==code);frame=bytearray.fromhex(target['frame_hex']);start=int.from_bytes(frame[7:9],'big')+2+skip;size=int.from_bytes(frame[start:start+2],'big');offset=start+2+size-1;frame[offset]=ord('8') if frame[offset]==ord('9') else ord('9');target.update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  self.mutate('initial',action)
 def test_original_tm_rollback_target_must_match(self):
  self.change_xid(9,0)
  with self.assertRaises(RuntimeError):self.read()
 def test_begin_reply_must_return_original_xid(self):
  self.change_xid(2,2)
  with self.assertRaises(RuntimeError):self.read()
 def test_native_trace_disagreement(self):
  path=self.root/'native-restart-frames.json';events=json.loads(path.read_bytes());events[0]['sha256']='0'*64;path.write_text(json.dumps(events),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_native_lock_reply(self):
  path=self.root/'native-restart.json';value=json.loads(path.read_bytes());frame=bytearray.fromhex(value['frames'][-1]['frame_hex']);frame[-1]=1;value['frames'][-1].update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest());value['lockable']=True;path.write_text(json.dumps(value),encoding='utf-8');(self.root/'native-restart-frames.json').write_text(json.dumps(value['frames']),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_missing_business_sql(self):
  (self.root/'restart.tsv').write_bytes(b'')
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_business_row_identity(self):
  path=self.root/'second_restart.tsv';value=path.read_text(encoding='utf-8').replace('business\t1\t1000','business\t2\t1000');path.write_text(value,encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_tc_pid_must_match_actual_startup(self):
  path=self.root/'runtime.json';value=json.loads(path.read_bytes());value['tc_pids'][1]=999;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_original_undo_branch(self):
  path=self.root/'restart.tsv';rows=[line.split('\t') for line in path.read_text(encoding='utf-8').splitlines()]
  for row in rows:
   if row[0]=='undo':row[2]='9'
  path.write_text('\n'.join('\t'.join(row) for row in rows)+'\n',encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_callback_precedes_operator(self):
  def action(events):
   command=next(e for e in events if e['event']=='operator-command-start');events.remove(command);events.append(command)
  self.mutate('operator_replay',action)
  with self.assertRaises(RuntimeError):self.read()
 def test_successful_response_not_actually_lost(self):
  self.mutate('operator_loss',lambda events:events.__setitem__(slice(None),[e for e in events if e['event']!='successful-rollback-response-lost']))
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_console_branch(self):
  path=self.root/'console-restart.json';value=json.loads(path.read_bytes());value['data'][0]['branchSessionVOs'][0]['branchId']='9';path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_tc_database_mode_cannot_claim_file(self):
  for stage in ['initial','operator_loss','operator_replay']:
   path=self.root/stage/'tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'use session store mode: raft',b'use session store mode: db'))
  with self.assertRaises(RuntimeError):self.read()
 def test_missing_natural_scheduler_event(self):
  path=self.root/'initial/tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'has changed the status to RollbackRetryTimeout',b'no natural transition'))
  with self.assertRaises(RuntimeError):self.read()
 def test_unrelated_http500_not_accepted(self):
  path=self.root/'operator_loss/tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'Caused by: java.util.concurrent.TimeoutException:',b'Caused by: unrelated failure:'))
  with self.assertRaises(RuntimeError):self.read()
if __name__=='__main__':unittest.main()
