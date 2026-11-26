import unittest
from finalization import checked_success
class FinalizationTests(unittest.TestCase):
 def test_input_drift_cannot_leave_success(self):
  errors=[]
  def changed():raise RuntimeError('owned input changed')
  self.assertFalse(checked_success(True,errors,changed))
  self.assertEqual(['Final input verification failed: owned input changed'],errors)
 def test_incomplete_or_cleanup_failure_cannot_pass(self):
  self.assertFalse(checked_success(False,[],lambda:None))
  self.assertFalse(checked_success(True,['owned process remains'],lambda:None))
 def test_complete_verified_run_can_pass(self):
  self.assertTrue(checked_success(True,[],lambda:None))
if __name__=='__main__':unittest.main()
