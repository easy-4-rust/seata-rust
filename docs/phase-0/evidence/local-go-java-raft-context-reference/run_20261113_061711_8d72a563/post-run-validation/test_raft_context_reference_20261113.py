import hashlib,importlib.util,json,os,shutil,tempfile,unittest
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');SOURCE=Path(os.environ.get('SEATA_CTX_REFERENCE_EVIDENCE',str(ROOT/'docs/phase-0/evidence/local-go-java-raft-context-reference/run_20261113_061711_8d72a563')))
spec=importlib.util.spec_from_file_location('actual_context_reference_validation',ROOT/'tools/go-java-v1-raft-context-update/reference_evidence.py');e=importlib.util.module_from_spec(spec);spec.loader.exec_module(e)
class ContextReferenceGuards(unittest.TestCase):
 def setUp(self):
  temporary=tempfile.TemporaryDirectory(prefix='context-red-unit-',dir='D:/Temp');self.root=Path(temporary.name);assert self.root.resolve().is_relative_to(Path('D:/Temp').resolve());self.addCleanup(temporary.cleanup)
  names=['phase-one.json','runtime.json','cluster-timeout.json','cluster-election_one.json','cluster-restored.json']
  for name in ['timeout','restart','rejected']:names += [name+'.tsv','prepared-context-'+name+'.tsv','console-'+name+'.json','native-'+name+'.json','native-'+name+'-frames.json']
  for stage in ['initial','operator_rejected']:names += [stage+'/'+name for name in ['proxy-events.json','proxy-result.json','sdk-stdout.log']]
  names.append('operator_rejected/operator.json')
  for name in names:
   path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(SOURCE/name,path)
 def read(self):return e.derive(self.root)
 def change(self,name,action):
  path=self.root/name;value=json.loads(path.read_bytes());action(value);path.write_text(json.dumps(value),encoding='utf-8')
 def test_actual_reference_defect_control(self):self.assertFalse(self.read()['recovery_certified'])
 def test_missing_original_updated_report_is_not_reproduction(self):
  def action(events):
   events[:]=[v for v in events if v.get('payload_type')!=13]
   for index,v in enumerate(events):v['sequence']=index+1
  self.change('initial/proxy-events.json',action)
  with self.assertRaises(Exception):self.read()
 def test_missing_successful_report_ack_is_not_reproduction(self):
  def action(events):
   events[:]=[v for v in events if v.get('payload_type')!=14]
   for index,v in enumerate(events):v['sequence']=index+1
  self.change('initial/proxy-events.json',action)
  with self.assertRaises(Exception):self.read()
 def test_healthy_replicas_cannot_be_called_defective(self):
  reported=json.loads((SOURCE/'proof.json').read_bytes())['acknowledged_report_tcc_context']
  for name in ['timeout','election_one','restored']:
   def action(value):
    for node in value['nodes'].values():
     for branch in node['records'][0]['branches']:
      if branch['type']==1:branch['application_data']=reported
   self.change('cluster-'+name+'.json',action)
  with self.assertRaises(Exception):self.read()
 def test_without_actual_business_rejection_no_claim(self):
  path=self.root/'operator_rejected/sdk-stdout.log';path.write_bytes(path.read_bytes().replace(b'REAL_TCC_CONTEXT_REJECTED',b'NO_CONTEXT_REJECTION'))
  with self.assertRaises(Exception):self.read()
 def test_confirmed_business_is_not_missing_context_failure(self):
  path=self.root/'rejected.tsv';path.write_text(path.read_text().replace('commit_account\t1\t10\t0','commit_account\t1\t0\t10').replace('\tprepared\t0','\tcommitted\t1'),encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_actual_prepared_token_must_match_sql(self):
  path=self.root/'prepared-context-restart.tsv';path.write_text(path.read_text().replace('\t10\t','\t11\t'),encoding='utf-8')
  with self.assertRaises(Exception):self.read()
 def test_without_actual_election_no_claim(self):
  self.change('cluster-election_one.json',lambda v:v['nodes'][str(v['leader'])].update(raft_term_after=9999))
  with self.assertRaises(Exception):self.read()
 def test_original_leader_must_have_applied_new_report(self):
  registered=json.loads((SOURCE/'proof.json').read_bytes())['registered_tcc_context']
  def action(value):
   for branch in value['nodes'][str(value['leader'])]['records'][0]['branches']:
    if branch['type']==1:branch['application_data']=registered
  self.change('cluster-timeout.json',action)
  with self.assertRaises(Exception):self.read()
 def test_callback_containing_new_context_is_not_old_context_reproduction(self):
  reported=json.loads((SOURCE/'proof.json').read_bytes())['acknowledged_report_tcc_context']
  def action(events):
   for event in events:
    if event.get('event')!='wire' or event.get('payload_type')!=3:continue
    value=e.wire.decode(event)
    if value['branch_type']!=1:continue
    frame=value['frame'];old=json.loads((SOURCE/'proof.json').read_bytes())['registered_tcc_context'].encode();raw=reported.encode();updated=bytearray(frame[:-len(old)-4]+len(raw).to_bytes(4,'big')+raw);updated[3:7]=len(updated).to_bytes(4,'big');event.update(frame_hex=updated.hex(),sha256=hashlib.sha256(updated).hexdigest())
  self.change('operator_rejected/proxy-events.json',action)
  with self.assertRaises(Exception):self.read()
if __name__=='__main__':unittest.main()
