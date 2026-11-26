import copy,importlib.util,json,os,unittest
from pathlib import Path
from raft_commit_context_binding_20261113 import verify_context_binding
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=Path(os.environ.get('SEATA_RAFT_COMMIT_CERT_EVIDENCE',str(ROOT/'docs/phase-0/evidence/local-go-java-raft-commit-timeout/run_20261113_054436_bda6b22b')))
spec=importlib.util.spec_from_file_location('context_binding_actual_wire',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)
def registrations(directory):
 events=json.loads((directory/'initial/proxy-events.json').read_bytes());requests={};values={}
 for event in events:
  if event['event']!='wire':continue
  value=wire.decode(event);key=(value['connection'],value['id'])
  if event['direction']=='sdk-to-tc' and value['code']==11:
   frame=value['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);xid=reader.string();kind=reader.integer(1);resource=reader.string();lock=reader.string(4);context=reader.string(4);reader.end()
   if key in requests:raise RuntimeError('Duplicate actual branch registration')
   requests[key]=dict(xid=xid,branch_type=kind,resource=resource,lock_key=lock,application_data=context if context else None)
  if event['direction']=='tc-to-sdk' and value['code']==12:
   if key not in requests or value['result']!=1:raise RuntimeError('Actual context registration rejected/unmatched')
   row=dict(requests[key],branch_id=value['branch_id'])
   if row['branch_type'] in values:raise RuntimeError('Duplicate actual branch kind')
   values[row['branch_type']]=row
 if set(values)!={0,1}:raise RuntimeError('Actual AT/TCC registrations missing')
 return values
class OriginalContextGuards(unittest.TestCase):
 def good(self):return json.loads((OUT/'proof.json').read_bytes()),registrations(OUT)
 def test_actual_registered_reported_and_replicated_context(self):verify_context_binding(*self.good())
 def test_all_replicas_cannot_agree_on_wrong_context(self):
  proof,registered=self.good()
  for bundle in proof['cluster'].values():
   for node in bundle['nodes'].values():
    for record in node['records']:
     for branch in record['branches']:
      if branch['type']==1:branch['application_data']='same-but-wrong-context'
  with self.assertRaises(RuntimeError):verify_context_binding(proof,registered)
 def test_original_report_must_bind_replicated_context(self):
  proof,registered=self.good();proof['business']['original_branch_reports'][1]['application_data']='wrong-report-context'
  with self.assertRaises(RuntimeError):verify_context_binding(proof,registered)
 def test_changed_prepare_context_is_outside_selected_unchanged_context_scope(self):
  proof,registered=self.good();registered[1]['application_data']='different-registration-context'
  with self.assertRaises(RuntimeError):verify_context_binding(proof,registered)
if __name__=='__main__':unittest.main()
