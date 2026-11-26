"""Negative guards for genuine persisted rollback-timeout recovery evidence."""
import copy
import unittest
from verify import validate

class EvidenceGuards(unittest.TestCase):
 def good(self):
  phase=dict(xid='owned:28091:123',branch_id=456,balance=900,undo_status=0,undo_size=2,undo_sha256='44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a',undo_hex='7b7d',process_id=1)
  return dict(phase=phase,snapshots={name:dict(balance=balance,global_status=17 if name!='final' else None,branches=1 if name!='final' else 0,locks=1 if name!='final' else 0,undo_status=status,xid=phase['xid'],branch_id=456,undo_hex='7b7d' if status==0 else '7b7d' if status==1 else None) for name,balance,status in [('timeout',900,0),('restart',900,0),('lost',1000,None),('second_restart',1000,None),('final',1000,1)]},processes=[1,2,3],database_starts=['a','b','c'],request_withheld=2,successful_response_lost=1,registered_stages=[True,True,True],original_branch=True,callback_correlations=True,tm_rollbacks=1,tm_begins=1,operator_results=[500,200],proxy_errors=[])
 def test_control(self):validate(self.good())
 def test_fake_timeout(self):
  value=self.good();value['snapshots']['timeout']['global_status']=9
  with self.assertRaises(RuntimeError):validate(value)
 def test_empty_snapshot(self):
  value=self.good();value['snapshots']['restart']={}
  with self.assertRaises(RuntimeError):validate(value)
 def test_changed_undo(self):
  value=self.good();value['snapshots']['restart']['undo_hex']='00'
  with self.assertRaises(RuntimeError):validate(value)
 def test_wrong_identity(self):
  value=self.good();value['snapshots']['second_restart']['branch_id']=999
  with self.assertRaises(RuntimeError):validate(value)
 def test_compensation_missing(self):
  value=self.good();value['snapshots']['lost']['balance']=900
  with self.assertRaises(RuntimeError):validate(value)
 def test_lost_response_not_real(self):
  value=self.good();value['successful_response_lost']=0
  with self.assertRaises(RuntimeError):validate(value)
 def test_no_second_process(self):
  value=self.good();value['processes']=[1,1,3]
  with self.assertRaises(RuntimeError):validate(value)
 def test_wrong_request_id(self):
  value=self.good();value['callback_correlations']=False
  with self.assertRaises(RuntimeError):validate(value)
 def test_no_registration(self):
  value=self.good();value['registered_stages'][1]=False
  with self.assertRaises(RuntimeError):validate(value)
 def test_target_replayed(self):
  value=self.good();value['tm_rollbacks']=2
  with self.assertRaises(RuntimeError):validate(value)
 def test_proxy_error(self):
  value=self.good();value['proxy_errors']=['unexpected reset']
  with self.assertRaises(RuntimeError):validate(value)
 def test_final_not_complete(self):
  value=self.good();value['snapshots']['final']['branches']=1
  with self.assertRaises(RuntimeError):validate(value)

if __name__=='__main__':unittest.main()
