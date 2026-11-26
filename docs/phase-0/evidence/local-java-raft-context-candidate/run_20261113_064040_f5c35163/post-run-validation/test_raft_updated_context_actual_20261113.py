"""Corrupt owned D: copies of actual changed-Prepare evidence, never primary files."""
from pathlib import Path
import copy,hashlib,importlib.util,json,os,shutil,tempfile,unittest
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');SOURCE=Path(os.environ.get('SEATA_UPDATED_CONTEXT_ACTUAL',ROOT/'docs/phase-0/evidence/local-java-raft-context-candidate/run_20261113_064040_f5c35163'))
s=importlib.util.spec_from_file_location('actual_updated_context_gate',ROOT/'tools/go-java-v1-raft-context-update/context_evidence.py');e=importlib.util.module_from_spec(s);s.loader.exec_module(e)
class ActualContextGuards(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='context-actual-unit-',dir='D:/Temp');self.root=Path(self.temp.name);assert self.root.resolve().is_relative_to(Path('D:/Temp').resolve());self.addCleanup(self.temp.cleanup);self.proof=json.loads((SOURCE/'proof.json').read_bytes())
  names=['runtime.json']+['prepared-context-'+stage+'.tsv' for stage in ['timeout','restart','lost','second_restart','final']]
  for stage in ['initial','operator_loss','operator_replay']:names.extend([stage+'/proxy-events.json',stage+'/sdk-stdout.log'])
  for path in SOURCE.glob('node_*_generation_*/*'):
   if path.name in ['process.json','tc-stdout.log']:names.append(path.relative_to(SOURCE).as_posix())
  for name in names:
   target=self.root/name;target.parent.mkdir(parents=True,exist_ok=True);raw=(SOURCE/name).read_bytes()
   if name.endswith('.log'):raw=raw.replace(SOURCE.as_posix().encode(),self.root.as_posix().encode())
   target.write_bytes(raw)
 def read(self):return e.derive(self.root,self.proof)
 def mutate_events(self,stage,fn):
  path=self.root/stage/'proxy-events.json';events=json.loads(path.read_bytes());fn(events);path.write_text(json.dumps(events),encoding='utf-8')
 def test_actual_control(self):self.read()
 def test_all_replicas_old_context_cannot_pass_mutual_consistency(self):
  old=self.proof['updated_context']['registered_tcc_context']
  for bundle in self.proof['cluster'].values():
   for node in bundle['nodes'].values():
    for record in node['records']:
     for branch in record['branches']:
      if branch['type']==1:branch['application_data']=old
  with self.assertRaises(RuntimeError):self.read()
 def test_one_restored_replica_loses_update(self):
  node=next(iter(self.proof['cluster']['restored']['nodes'].values()));branch=next(b for b in node['records'][0]['branches'] if b['type']==1);branch['application_data']=self.proof['updated_context']['registered_tcc_context']
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_replayed_callback_using_old_context_rejected(self):
  old=self.proof['updated_context']['registered_tcc_context'].encode();reported=self.proof['updated_context']['acknowledged_report_tcc_context'].encode()
  def fn(events):
   for event in events:
    if event['event']!='wire':continue
    value=e.wire.decode(event)
    if value['code']==3 and value['branch_type']==1:
     frame=bytearray(value['frame'][:-len(reported)-4]+len(old).to_bytes(4,'big')+old);frame[3:7]=len(frame).to_bytes(4,'big');event.update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  self.mutate_events('operator_replay',fn)
  with self.assertRaises(RuntimeError):self.read()
 def test_original_sql_token_changed(self):
  path=self.root/'prepared-context-final.tsv';path.write_text(path.read_text().replace(self.proof['updated_context']['updated_prepare_values']['prepared-token'],'0'*64),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_prepare_marker_belongs_to_wrong_process(self):
  path=self.root/'initial/sdk-stdout.log';text=path.read_text();pid=json.loads((self.root/'runtime.json').read_bytes())['sdk_pids'][0];text=text.replace(' '+str(pid)+'\n',' '+str(pid+1)+'\n');path.write_text(text,encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_accepted_marker_belongs_to_wrong_process(self):
  path=self.root/'operator_loss/sdk-stdout.log';text=path.read_text();pid=json.loads((self.root/'runtime.json').read_bytes())['sdk_pids'][1];text=text.replace(' '+str(pid)+'\n',' '+str(pid+1)+'\n');path.write_text(text,encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_context_handler_cannot_load_from_original_jar(self):
  path=next(self.root.glob('node_*_generation_*/tc-stdout.log'));lines=path.read_text(encoding='utf-8',errors='replace').splitlines();lines=[line.split(' source: ')[0]+' source: jar:nested:/original.jar/' if 'UpdateBranchSessionExecute source: ' in line else line for line in lines];path.write_text('\n'.join(lines),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_unacknowledged_report_cannot_certify(self):
  next(row for row in self.proof['business']['original_branch_reports'] if row['branch_type']==1)['accepted']=False
  with self.assertRaises(RuntimeError):self.read()
 def test_registration_already_has_updated_values_not_changed_prepare(self):
  reported=self.proof['updated_context']['acknowledged_report_tcc_context'].encode();old=self.proof['updated_context']['registered_tcc_context'].encode()
  def fn(events):
   for event in events:
    if event['event']!='wire':continue
    value=e.wire.decode(event)
    if value['code']==11 and value['branch_type']==1:
     frame=bytearray(value['frame'][:-len(old)-4]+len(reported).to_bytes(4,'big')+reported);frame[3:7]=len(frame).to_bytes(4,'big');event.update(frame_hex=frame.hex(),sha256=hashlib.sha256(frame).hexdigest())
  self.mutate_events('initial',fn)
  with self.assertRaises(RuntimeError):self.read()
if __name__=='__main__':unittest.main()
