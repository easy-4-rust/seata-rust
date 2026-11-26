"""Corrupt copies of real wire/SQL evidence; these copies are unit fixtures, never certification."""
import hashlib,json,os,shutil,tempfile,unittest
from pathlib import Path
from evidence import derive
from verify import validate

class RawEvidenceGuards(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='seata-timeout-unit-',dir='D:/Temp');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
  official=os.environ.get('SEATA_AT_TIMEOUT_EVIDENCE');source=Path(official or 'D:/codex-builds/real-at-timeout-trial-20261113/run_20261113_002535_f5b2b3a9')
  files=['phase-one.json','runtime.json']+[n+'.tsv' for n in ['timeout','restart','lost','second_restart','final']]+['console-'+n+'.json' for n in ['timeout','restart','lost','second-restart','final']]+['initial/tc-stdout.log','operator_loss/tc-stdout.log']
  for name in ['initial','operator_loss','operator_replay']:
   files += [name+'/'+leaf for leaf in ['proxy-events.json','proxy-result.json','sdk-stdout.log']]
   if name!='initial':files.append(name+'/operator.json')
  for name in files:
   path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/name,path)
  if not official:
   # The preliminary trial predates operator markers. Add boundaries only to this unit fixture.
   for stage in ['operator_loss','operator_replay']:
    path=self.root/stage/'proxy-events.json';events=json.loads(path.read_bytes());index=next(i for i,e in enumerate(events) if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==5);events.insert(index,dict(event='operator-command-start',xid=json.loads((self.root/'phase-one.json').read_bytes())['xid']))
    for i,e in enumerate(events):e['sequence']=i+1
    path.write_text(json.dumps(events),encoding='utf-8')
 def read(self):return validate(derive(self.root))
 def mutate_events(self,stage,action):
  path=self.root/stage/'proxy-events.json';events=json.loads(path.read_bytes());action(events)
  for i,event in enumerate(events):event['sequence']=i+1
  path.write_text(json.dumps(events),encoding='utf-8')
 def test_actual_control(self):self.read()
 def test_empty_sql(self):
  (self.root/'restart.tsv').write_bytes(b'')
  with self.assertRaises(RuntimeError):self.read()
 def test_wrong_branch_in_sql(self):
  path=self.root/'second_restart.tsv';lines=path.read_text(encoding='utf-8').splitlines();lines=[line.replace(line.split('\t')[2],'999',1) if line.startswith('branch\t') else line for line in lines];path.write_text('\n'.join(lines)+'\n',encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_registration_missing(self):
  self.mutate_events('operator_replay',lambda events:events.__setitem__(slice(None),[e for e in events if e.get('payload_type') not in [103,104]]))
  with self.assertRaisesRegex(RuntimeError,'Callback precedes accepted real resource registration'):self.read()
 def test_reply_request_id_wrong(self):
  def change(events):
   for event in events:
    if event['event']=='wire' and event['payload_type']==6:
     frame=bytearray.fromhex(event['frame_hex']);frame[12:16]=(9999).to_bytes(4,'big');event.update(frame_hex=frame.hex(),id=9999,sha256=hashlib.sha256(frame).hexdigest())
  self.mutate_events('operator_replay',change)
  with self.assertRaises(RuntimeError):self.read()
 def test_false_response_loss(self):
  self.mutate_events('operator_loss',lambda events:events.__setitem__(slice(None),[e for e in events if e['event']!='successful-rollback-response-lost']))
  with self.assertRaisesRegex(RuntimeError,'Actual registered callback/drop evidence differs'):self.read()
 def test_auto_replay_before_operator(self):
  def change(events):
   command=next(e for e in events if e['event']=='operator-command-start');events.remove(command);events.append(command)
  self.mutate_events('operator_replay',change)
  with self.assertRaisesRegex(RuntimeError,'Timeout17 replayed before the operator command'):self.read()
 def test_fake_console17(self):
  path=self.root/'console-restart.json';value=json.loads(path.read_bytes());value['data'][0]['status']=9;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_no_scheduler_transition(self):
  (self.root/'initial/tc-stdout.log').write_text('not a natural transition',encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_sdk_pid_wrong(self):
  path=self.root/'runtime.json';value=json.loads(path.read_bytes());value['sdk_pids'][1]=999;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_unrelated_http500(self):
  (self.root/'operator_loss/tc-stdout.log').write_bytes(b'unrelated server failure')
  with self.assertRaises(RuntimeError):self.read()
 def test_response_loss_in_wrong_stage(self):
  self.mutate_events('operator_loss',lambda events:events.__setitem__(slice(None),[e for e in events if e['event']!='successful-rollback-response-lost']))
  def add(events):
   reply=next(e for e in events if e.get('payload_type')==6);events.append(dict(event='successful-rollback-response-lost',connection=reply['connection'],id=reply['id'],frame_hex=reply['frame_hex']))
  self.mutate_events('operator_replay',add)
  with self.assertRaises(RuntimeError):self.read()

if __name__=='__main__':unittest.main()
