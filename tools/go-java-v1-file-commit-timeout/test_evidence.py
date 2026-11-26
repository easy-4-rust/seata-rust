"""Mutate real evidence copies while preserving frame hashes and event sequence legality."""
import copy,hashlib,json,os,shutil,tempfile,unittest
from pathlib import Path
from evidence import derive
from verify import validate

class FileMixedRawGuards(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='seata-commit-unit-',dir='D:/Temp');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
  source=Path(os.environ.get('SEATA_FILE_COMMIT_EVIDENCE','E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation/docs/phase-0/evidence/local-go-java-file-commit-timeout/run_20261113_033951_6113e4dc'))
  names=['phase-one.json','runtime.json','business-tables.txt','application.yml']+[n+'.tsv' for n in ['timeout','restart','lost','second_restart','final']]+['console-'+n+'.json' for n in ['timeout','restart','lost','second-restart','final']]
  for stage in ['initial','operator_loss','operator_replay']:
   names += [stage+'/'+n for n in ['proxy-events.json','proxy-result.json','sdk-stdout.log','tc-stdout.log','tc-process.json']]
   if stage!='initial':names.append(stage+'/operator.json')
  for name in ['timeout','restart','lost','second_restart','final']:names += ['native-'+name+'.json','native-'+name+'-frames.json']
  for name in ['initial-after','operator_loss-after','operator_replay-after','operator_loss-before','operator_replay-before']:
   names.append('journal-'+name+'.json');shutil.copytree(source/'journals'/name,self.root/'journals'/name)
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
  path=self.root/'operator_loss/tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'wait response error:',b'unrelated failure:'))
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
 def test_at_branch_status_preserved_across_restart(self):
  path=self.root/'console-restart.json';value=json.loads(path.read_bytes());branch=next(b for b in value['data'][0]['branchSessionVOs'] if b['branchType']=='AT');branch['status']=5;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_original_at_phase_one_report_status_bound(self):
  def change(events):
   event=next(e for e in events if e.get('payload_type')==13 and bytes.fromhex(e['frame_hex'])[-1]==0);frame=bytearray.fromhex(event['frame_hex']);start=int.from_bytes(frame[7:9],'big')+2;size=int.from_bytes(frame[start:start+2],'big');frame[start+2+size+8]=5;self.update_frame(event,frame)
  self.mutate('initial',change)
  with self.assertRaises(RuntimeError):self.read()
 def test_original_tcc_action_context_preserved(self):
  path=self.root/'console-second-restart.json';value=json.loads(path.read_bytes());value['data'][0]['branchSessionVOs'][0]['applicationData']='{"changed":"owned unit fixture"}';path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_phase_one_reports_require_success_acknowledgements(self):
  self.mutate('initial',lambda events:events.__setitem__(slice(None),[e for e in events if e.get('payload_type')!=14]))
  with self.assertRaises(RuntimeError):self.read()
 def test_report_acknowledgement_precedes_first_commit_callback(self):
  def move(events):
   acknowledgements=[e for e in events if e.get('payload_type')==14];events[:]=[e for e in events if e.get('payload_type')!=14]+acknowledgements
  self.mutate('initial',move)
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_tcc_callback_context_preserved(self):
  def change(events):
   event=next(e for e in events if e.get('payload_type')==3);frame=bytearray.fromhex(event['frame_hex']);offset=frame.rfind(b'owned-real-commit-action');self.assertGreater(offset,0);frame[offset]=ord('x');self.update_frame(event,frame)
  self.mutate('operator_replay',change)
  with self.assertRaises(RuntimeError):self.read()
 def change_xid(self,code,skip):
  def change(events):
   event=next(e for e in events if e.get('payload_type')==code);frame=bytearray.fromhex(event['frame_hex']);start=int.from_bytes(frame[7:9],'big')+2+skip;length=int.from_bytes(frame[start:start+2],'big');offset=start+2+length-1;frame[offset]=ord('8') if frame[offset]==ord('9') else ord('9');self.update_frame(event,frame)
  self.mutate('initial',change)
 def test_original_tm_commit_target(self):
  self.change_xid(7,0)
  with self.assertRaises(RuntimeError):self.read()
 def test_original_begin_reply_xid(self):
  self.change_xid(2,2)
  with self.assertRaises(RuntimeError):self.read()
 def test_native_trace_disagreement(self):
  path=self.root/'native-restart-frames.json';value=json.loads(path.read_bytes());value[0]['sha256']='0'*64;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_native_commit_lock_must_be_free(self):
  path=self.root/'native-restart.json';value=json.loads(path.read_bytes());frame=bytearray.fromhex(value['frames'][-1]['frame_hex']);frame[-1]=0;value['frames'][-1].update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest());value['lockable']=False;path.write_text(json.dumps(value),encoding='utf-8');(self.root/'native-restart-frames.json').write_text(json.dumps(value['frames']),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_native_status_response_id(self):
  path=self.root/'native-restart.json';value=json.loads(path.read_bytes());frame=bytearray.fromhex(value['frames'][3]['frame_hex']);frame[12:16]=(999).to_bytes(4,'big');value['frames'][3].update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest());path.write_text(json.dumps(value),encoding='utf-8');(self.root/'native-restart-frames.json').write_text(json.dumps(value['frames']),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_tc_database_configuration_rejected(self):
  path=self.root/'application.yml';path.write_bytes(path.read_bytes().replace(b'  store:\n    mode: file',b'  store:\n    mode: db'))
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_backend_must_be_file(self):
  path=self.root/'operator_replay/tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'use session store mode: file',b'use session store mode: db'))
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_tc_startup_pid(self):
  path=self.root/'runtime.json';value=json.loads(path.read_bytes());value['tc_pids'][1]=999;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_stopped_journal_bytes(self):
  (self.root/'journals/initial-after/28091/root.data').write_bytes(b'owned corrupted fixture')
  with self.assertRaises(RuntimeError):self.read()
 def test_restart_uses_previously_stopped_journal(self):
  path=self.root/'journals/operator_replay-before/28091/root.data';data=path.read_bytes()+b'x';path.write_bytes(data);audit=self.root/'journal-operator_replay-before.json';value=json.loads(audit.read_bytes());row=next(row for row in value['files'] if row['path']=='28091/root.data');row.update(bytes=len(data),sha256=hashlib.sha256(data).hexdigest());audit.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_mysql_cannot_store_tc_sessions(self):
  path=self.root/'business-tables.txt'
  with path.open('a',encoding='utf-8') as stream:stream.write('global_table\n')
  with self.assertRaises(RuntimeError):self.read()
 def test_retained_branch_type(self):
  path=self.root/'console-lost.json';value=json.loads(path.read_bytes());value['data'][0]['branchSessionVOs'][0]['branchType']='AT';path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_retained_branch_resource(self):
  path=self.root/'console-lost.json';value=json.loads(path.read_bytes());value['data'][0]['branchSessionVOs'][0]['resourceId']='changed-owned-action';path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_original_at_undo_branch(self):
  path=self.root/'restart.tsv';rows=[line.split('\t') for line in path.read_text(encoding='utf-8').splitlines()]
  for row in rows:
   if row[0]=='undo':row[2]='9'
  path.write_text('\n'.join('\t'.join(row) for row in rows)+'\n',encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_journal_path_traversal(self):
  path=self.root/'journal-operator_loss-before.json';value=json.loads(path.read_bytes());value['files'][0]['path']='28091/../../outside';path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_completed_at_branch_not_retained(self):
  at=next(branch for branch in json.loads((self.root/'console-timeout.json').read_bytes())['data'][0]['branchSessionVOs'] if branch['branchType']=='AT');path=self.root/'console-lost.json';value=json.loads(path.read_bytes());value['data'][0]['branchSessionVOs'].append(at);path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()

if __name__=='__main__':unittest.main()
