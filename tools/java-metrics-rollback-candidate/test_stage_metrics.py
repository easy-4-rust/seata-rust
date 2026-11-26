"""Unit-only rollback event examples; real evidence is checked separately, without synthesis."""
import copy,unittest
from metrics_verify import validate,FIELDS

class RollbackStageMetrics(unittest.TestCase):
 def good(self,kind):
  def event(status,end,retry=False,begin=1000):return dict(transaction_id=7,role='tc',application_id='real-at-full-sdk',group='real_at',name='real-at-natural-timeout',status=status,begin_time=begin,end_time=end,retry_global=retry,retry_branch=False)
  value=dict(pid=1,registry_class='org.apache.seata.metrics.registry.compact.CompactRegistry',subscriber_sources=['file:/D:/unit/'],metrics={key:0 for key in FIELDS},events=[])
  if kind=='timeout':
   value['events']=[event('Begin',None),event('RollbackRetryTimeout',6100,True)];value['metrics'].update(failed=1,two_phase_timeout=1,failed_timer_count=1,failed_timer_total=5100000)
  if kind in ['lost','final']:value['events']=[event('RollbackRetryTimeout',None,True)]
  if kind=='final':
   value['events'] += [event('Rollbacked',9003,True),event('AfterRollbacked',9003,True,9000)];value['metrics'].update(after_rollbacked=1,after_rollbacked_summary=1,after_rollbacked_timer_count=1,after_rollbacked_timer_total=3000)
  return value
 def check(self,kind,value=None):return validate(value or self.good(kind),kind,1,'file:/D:/unit/','127.0.0.1:28091:7')
 def test_original_rollback_timeout_is_counted(self):self.check('timeout')
 def test_nullable_rollback_reentry_is_not_completion(self):self.check('lost')
 def test_real_after_rollback_completion_is_counted(self):self.check('final')
 def test_fresh_registry_baseline(self):self.check('baseline')
 def test_commit_doing_event_cannot_claim_rollback(self):
  value=self.good('lost');value['events'][0]['status']='CommitRetryTimeout'
  with self.assertRaises(RuntimeError):self.check('lost',value)
 def test_final_requires_after_rollbacked(self):
  value=self.good('final');value['events'][-1]['status']='AfterCommitted'
  with self.assertRaises(RuntimeError):self.check('final',value)
 def test_retry_reentry_cannot_count_failure(self):
  value=self.good('lost');value['metrics']['failed']=1
  with self.assertRaises(RuntimeError):self.check('lost',value)
 def test_final_duration_must_match_event(self):
  value=self.good('final');value['metrics']['after_rollbacked_timer_total']=0
  with self.assertRaises(RuntimeError):self.check('final',value)
 def test_begin_before_loss_must_be_original(self):
  value=self.good('timeout');value['events'][1]['begin_time']=1001
  with self.assertRaises(RuntimeError):self.check('timeout',value)
 def test_wrong_transaction_rejected(self):
  value=self.good('lost');value['events'][0]['transaction_id']=8
  with self.assertRaises(RuntimeError):self.check('lost',value)
 def test_doing_end_time_must_stay_nullable(self):
  value=self.good('lost');value['events'][0]['end_time']=6000
  with self.assertRaises(RuntimeError):self.check('lost',value)
 def test_retry_flags_must_match_real_reentry(self):
  value=self.good('lost');value['events'][0]['retry_branch']=True
  with self.assertRaises(RuntimeError):self.check('lost',value)

if __name__=='__main__':unittest.main()
