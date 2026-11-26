"""Destroy copies of real registry/event observations on D: without touching run evidence."""
import hashlib,json,os,shutil,tempfile,unittest
from pathlib import Path
from metrics_verify import derive

class RealMetricsGuards(unittest.TestCase):
 def setUp(self):
  temp=tempfile.TemporaryDirectory(prefix='metrics-lifecycle-unit-',dir='D:/Temp');self.addCleanup(temp.cleanup);self.root=Path(temp.name)
  official=os.environ.get('SEATA_METRICS_EVIDENCE');source=Path(official or 'E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation/docs/phase-0/evidence/local-java-metrics-lifecycle-candidate/run_20261113_042734_82868d53')
  self.expected_source='file:/'+(source/'metrics-overlay').as_posix()+'/'
  for path in list(source.glob('metrics-*.json'))+[source/'runtime.json',source/'phase-one.json',source/'console-timeout.json']:
   if path.name=='metrics-overlay-manifest.json':continue
   shutil.copyfile(path,self.root/path.name)
  for stage in ['initial','operator_loss','operator_replay']:
   (self.root/stage).mkdir();shutil.copyfile(source/stage/'tc-stdout.log',self.root/stage/'tc-stdout.log')
  if not official:
   # Preliminary run lacked ordering receipts. Add only unit-fixture metadata, never certification.
   for path in self.root.glob('metrics-*.json'):
    value=json.loads(path.read_bytes());parts=path.stem.split('-');kind=parts[1];pid=value['pid'];marker=None;line_hash=None
    if kind in ['lost','final']:
     stage='operator_loss' if kind=='lost' else 'operator_replay';identifier=json.loads((self.root/'phase-one.json').read_bytes())['xid'].rsplit(':',1)[1];marker='ignore unfinished completion metrics, transactionId='+identifier+', status=CommitRetryTimeout';line=next(line for line in (self.root/stage/'tc-stdout.log').read_text(encoding='utf-8',errors='replace').splitlines() if marker in line);line_hash=hashlib.sha256(line.encode()).hexdigest()
    receipt=dict(pid=pid,kind=kind,marker=marker,marker_line_sha256=line_hash,markers_before_request=1 if marker else 0,snapshot_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    (self.root/f'metrics-observation-{kind}-{pid}.json').write_text(json.dumps(receipt),encoding='utf-8')
 def read(self):return derive(self.root,self.expected_source)
 def mutate(self,kind,action):
  path=next(self.root.glob('metrics-'+kind+'-*.json'));value=json.loads(path.read_bytes());action(value);path.write_text(json.dumps(value),encoding='utf-8')
  receipt=self.root/f"metrics-observation-{kind}-{value['pid']}.json"
  if receipt.exists():
   data=json.loads(receipt.read_bytes());data['snapshot_sha256']=hashlib.sha256(path.read_bytes()).hexdigest();receipt.write_text(json.dumps(data),encoding='utf-8')
 def test_actual_control(self):self.read()
 def test_partial_snapshot_requires_handler_skip_marker(self):
  path=self.root/'operator_loss/tc-stdout.log';path.write_bytes(path.read_bytes().replace(b'ignore unfinished completion metrics',b'no handler processing evidence'))
  with self.assertRaises(RuntimeError):self.read()
 def test_snapshot_must_follow_skip_marker(self):
  path=next(self.root.glob('metrics-observation-lost-*.json'));value=json.loads(path.read_bytes());value['markers_before_request']=0;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_negative_active_counter_rejected(self):
  self.mutate('lost',lambda v:v['metrics'].update(active=-1))
  with self.assertRaises(RuntimeError):self.read()
 def test_reentry_must_not_invent_failure(self):
  self.mutate('lost',lambda v:v['metrics'].update(failed=1))
  with self.assertRaises(RuntimeError):self.read()
 def test_reentry_must_not_invent_zero_duration_sample(self):
  self.mutate('lost',lambda v:v['metrics'].update(failed_timer_count=1))
  with self.assertRaises(RuntimeError):self.read()
 def test_skipping_all_completed_events_is_not_accepted(self):
  self.mutate('timeout',lambda v:v['metrics'].update(failed=0,failed_timer_count=0,failed_timer_total=0,two_phase_timeout=0))
  with self.assertRaises(RuntimeError):self.read()
 def test_final_completion_requires_real_timer_sample(self):
  self.mutate('final',lambda v:v['metrics'].update(after_committed_timer_count=0))
  with self.assertRaises(RuntimeError):self.read()
 def test_duration_must_match_actual_event(self):
  self.mutate('timeout',lambda v:v['metrics'].update(failed_timer_total=v['metrics']['failed_timer_total']+1000))
  with self.assertRaises(RuntimeError):self.read()
 def test_actual_doing_event_required(self):
  self.mutate('lost',lambda v:v.update(events=[]))
  with self.assertRaises(RuntimeError):self.read()
 def test_original_transaction_identity_required(self):
  self.mutate('lost',lambda v:v['events'][0].update(transaction_id=999))
  with self.assertRaises(RuntimeError):self.read()
 def test_original_begin_time_preserved(self):
  self.mutate('lost',lambda v:v['events'][0].update(begin_time=1))
  with self.assertRaises(RuntimeError):self.read()
 def test_fresh_process_requires_own_zero_baseline(self):
  path=next(self.root.glob('metrics-baseline-*.json'));value=json.loads(path.read_bytes());value['metrics']['failed']=1;path.write_text(json.dumps(value),encoding='utf-8')
  with self.assertRaises(RuntimeError):self.read()
 def test_real_candidate_subscriber_source_required(self):
  self.mutate('lost',lambda v:v.update(subscriber_sources=['file:/D:/unrelated/']))
  with self.assertRaises(RuntimeError):self.read()
 def test_snapshot_pid_must_match_actual_process(self):
  self.mutate('lost',lambda v:v.update(pid=999))
  with self.assertRaises(RuntimeError):self.read()
if __name__=='__main__':unittest.main()
