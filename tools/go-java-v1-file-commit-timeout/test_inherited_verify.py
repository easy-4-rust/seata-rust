"""Reject wrong persisted commit outcomes before accepting real mixed-mode evidence."""
import copy,unittest
from verify import parent
validate=parent.validate

class CommitGuards(unittest.TestCase):
 def good(self):
  phase=dict(xid='owned:28091:123',branch_id=456,tcc_branch_id=455,undo_hex='7b7d',undo_size=2,undo_sha256='44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a',undo_status=0,balance=900,process_id=1)
  rows={}
  for name in ['timeout','restart','lost','second_restart','final']:
   before=name in ['timeout','restart'];final=name=='final'
   rows[name]=dict(xid=phase['xid'],balance=900,status=None if final else 16,at_branch=456 if before else None,tcc_branch=None if final else 455,locks=0,undo_hex='7B7D' if before else None,undo_status=0 if before else None,reserved=10 if before else 0,confirmed=0 if before else 10,effect_count=0 if before else 1,tcc_phase='prepared' if before else 'committed',tcc_xid=phase['xid'],tcc_effect_branch=455)
  return dict(phase=phase,snapshots=rows,sdk_pids=[1,2,3],tc_pids=[4,5,6],database_starts=['a','b','c'],withheld=2,lost=1,tm_begins=1,tm_commits=1,tm_rollbacks=0,stages_registered=[True]*3,callback_correlations=True,original_branches=True,operator_results=[(200,'500'),(200,'200')],proxy_errors=[])
 def test_control(self):validate(self.good())
 def test_fake16(self):
  v=self.good();v['snapshots']['timeout']['status']=8
  with self.assertRaises(RuntimeError):validate(v)
 def test_double_business_effect(self):
  v=self.good();v['snapshots']['final']['confirmed']=20;v['snapshots']['final']['effect_count']=2
  with self.assertRaises(RuntimeError):validate(v)
 def test_wrong_persistent_identity(self):
  v=self.good();v['snapshots']['second_restart']['tcc_effect_branch']=999
  with self.assertRaises(RuntimeError):validate(v)
 def test_at_not_cleaned(self):
  v=self.good();v['snapshots']['lost']['undo_hex']='7b7d'
  with self.assertRaises(RuntimeError):validate(v)
 def test_missing_at_branch_before_restart(self):
  v=self.good();v['snapshots']['timeout']['at_branch']=None
  with self.assertRaises(RuntimeError):validate(v)
 def test_locks_incorrect(self):
  v=self.good();v['snapshots']['restart']['locks']=1
  with self.assertRaises(RuntimeError):validate(v)
 def test_target_replayed(self):
  v=self.good();v['tm_commits']=2
  with self.assertRaises(RuntimeError):validate(v)
 def test_wrong_callback(self):
  v=self.good();v['callback_correlations']=False
  with self.assertRaises(RuntimeError):validate(v)
 def test_undo_changed(self):
  v=self.good();v['snapshots']['restart']['undo_hex']='00'
  with self.assertRaises(RuntimeError):validate(v)
 def test_wrong_operator_result(self):
  v=self.good();v['operator_results'][0]=(200,'200')
  with self.assertRaises(RuntimeError):validate(v)
 def test_same_process(self):
  v=self.good();v['sdk_pids'][1]=1
  with self.assertRaises(RuntimeError):validate(v)
 def test_unreserved_prepare(self):
  v=self.good();v['snapshots']['timeout']['reserved']=0
  with self.assertRaises(RuntimeError):validate(v)

if __name__=='__main__':unittest.main()
