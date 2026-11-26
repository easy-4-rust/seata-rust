import copy,json,unittest
from pathlib import Path
from raft_stop_binding_20261113 import verify_stop_binding
OUT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation/docs/phase-0/evidence/local-go-java-raft-at-timeout/run_20261113_052816_8ea31012')
def read(name):return json.loads((OUT/name).read_bytes())
class StopBindingGuards(unittest.TestCase):
 def good(self,label='election_one'):
  receipt=read('leader-stop-'+label+'.json');before=read('cluster-'+('timeout' if label=='election_one' else 'restored')+'.json');after=read('cluster-'+label+'.json');process=json.loads(Path(receipt['process_file']).read_bytes());return receipt,before,after,process
 def test_both_actual_stopped_leaders(self):
  for label in ['election_one','election_two']:verify_stop_binding(*self.good(label))
 def test_follower_cannot_replace_stopped_leader_identity(self):
  receipt,before,after,process=self.good();node=next(k for k in before['nodes'] if int(k) not in [before['leader'],after['leader']]);identity=before['processes'][node];receipt.update(node=int(node),pid=identity['pid'],process_file=identity['process_file']);process=json.loads(Path(identity['process_file']).read_bytes())
  with self.assertRaises(RuntimeError):verify_stop_binding(receipt,before,after,process)
 def test_before_sample_cannot_belong_to_another_process(self):
  receipt,before,after,process=self.good();receipt['before']['pid']+=1
  with self.assertRaises(RuntimeError):verify_stop_binding(receipt,before,after,process)
 def test_before_sample_cannot_claim_another_term(self):
  receipt,before,after,process=self.good()
  for key in ['term','term_before','term_after','raft_term_before','raft_term_after','reload_complete_term']:receipt['before'][key]+=1
  with self.assertRaises(RuntimeError):verify_stop_binding(receipt,before,after,process)
if __name__=='__main__':unittest.main()
