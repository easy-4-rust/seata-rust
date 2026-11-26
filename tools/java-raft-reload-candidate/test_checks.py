import copy,importlib.util,unittest
from pathlib import Path
import checks

class CompletionEvidenceTests(unittest.TestCase):
 def valid(self):return dict(leader=True,term=3,reload_complete_term=3,term_before=3,term_after=3,raft_term_before=3,raft_term_after=3,raft_leader_before=True,raft_leader_after=True,raft_leader_id_before='host:1',raft_leader_id_after='host:1',raft_peer_id='host:1',node_state='STATE_LEADER',leader_id='host:1')
 def test_missing_completion_rejected(self):
  with self.assertRaises(RuntimeError):checks.verify_completion(dict(leader=True,term=3))
 def test_failed_or_old_completion_rejected(self):
  for completed in [0,2,4]:
   with self.subTest(completed=completed),self.assertRaises(RuntimeError):checks.verify_completion(dict(leader=True,term=3,reload_complete_term=completed))
 def test_current_success_accepted(self):
  self.assertTrue(checks.verify_completion(self.valid()))
 def test_torn_term_or_raft_identity_rejected(self):
  for key,value in [('term_before',2),('term_after',4),('raft_term_before',2),('raft_term_after',4),('raft_leader_before',False),('raft_leader_after',False),('raft_leader_id_before','other:2'),('raft_leader_id_after','other:2'),('node_state','STATE_FOLLOWER'),('leader_id','other:2')]:
   state=self.valid();state[key]=value
   with self.subTest(key=key),self.assertRaises(RuntimeError):checks.verify_completion(state)
 def test_follower_marker_cannot_claim_current_completion(self):
  with self.assertRaises(RuntimeError):checks.verify_completion(dict(leader=False,term=3,reload_complete_term=3))
 def test_zero_term_cannot_claim_completion(self):
  with self.assertRaises(RuntimeError):checks.verify_completion(dict(leader=True,term=0,reload_complete_term=0))

if __name__=='__main__':unittest.main()
