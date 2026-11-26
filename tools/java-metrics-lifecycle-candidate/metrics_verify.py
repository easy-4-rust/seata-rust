"""Bind each non-reset registry snapshot to that TC process and real published events."""
import hashlib,json
from pathlib import Path
FIELDS={'active','failed','two_phase_timeout','failed_timer_count','failed_timer_total','after_committed','after_committed_summary','after_committed_timer_count','after_committed_timer_total'}
def validate(value,kind,pid,source,xid=None):
 if value.get('pid')!=pid or value.get('registry_class')!='org.apache.seata.metrics.registry.compact.CompactRegistry' or value.get('subscriber_sources')!=[source]:raise RuntimeError('Actual metrics process/registry/subscriber differs')
 metrics=value.get('metrics',{});events=value.get('events')
 if set(metrics)!=FIELDS or any(type(v) is not int or v<0 for v in metrics.values()) or not isinstance(events,list):raise RuntimeError('Incomplete or negative actual registry snapshot')
 if kind in ['baseline','restart','second_restart']:
  if events or any(metrics.values()):raise RuntimeError('Fresh TC process does not have its own zero metric baseline')
  return value
 identifier=int(xid.rsplit(':',1)[1])
 for event in events:
  if event.get('transaction_id')!=identifier or (event.get('role'),event.get('application_id'),event.get('group'),event.get('name'))!=('tc','real-at-full-sdk','real_at','real-mixed-commit-timeout'):raise RuntimeError('Metrics event original transaction/labels differ')
  if type(event.get('begin_time')) is not int or type(event.get('retry_global')) is not bool or type(event.get('retry_branch')) is not bool:raise RuntimeError('Original metrics event timing/flags missing')
 expected={name:0 for name in FIELDS}
 if kind=='timeout':
  begin=[e for e in events if e['status']=='Begin' and e['end_time'] is None]
  done=[e for e in events if e['status']=='CommitRetryTimeout' and e['end_time'] is not None]
  if len(begin)!=1 or len(done)!=1 or done[0]['begin_time']!=begin[0]['begin_time']:raise RuntimeError('Actual Begin/natural timeout completion events missing')
  duration=done[0]['end_time']-done[0]['begin_time']
  if duration<0:raise RuntimeError('Actual timeout completion duration invalid')
  expected.update(failed=1,two_phase_timeout=1,failed_timer_count=1,failed_timer_total=duration*1000)
 elif kind in ['lost','final']:
  ongoing=[e for e in events if e['status']=='CommitRetryTimeout' and e['end_time'] is None and e['retry_global'] and not e['retry_branch']]
  if len(ongoing)!=1:raise RuntimeError('Actual nullable retry-timeout doing event missing')
  if kind=='lost':
   if events!=ongoing:raise RuntimeError('Partial commit unexpectedly produced completed metrics event')
  else:
   done=[e for e in events if e['status']=='AfterCommitted' and e['end_time'] is not None and e['retry_global'] and not e['retry_branch']]
   if len(done)!=1 or any(e['status'] not in ['CommitRetryTimeout','Committed','AfterCommitted'] for e in events):raise RuntimeError('Actual final completion metrics event missing')
   duration=done[0]['end_time']-done[0]['begin_time']
   if duration<0:raise RuntimeError('Actual after-commit completion duration invalid')
   expected.update(after_committed=1,after_committed_summary=1,after_committed_timer_count=1,after_committed_timer_total=duration*1000)
 else:raise RuntimeError('Unknown metrics verification stage')
 if metrics!=expected:raise RuntimeError('Real registry counters/timer samples differ from actual event lifecycle: '+str(metrics)+' expected='+str(expected))
 return value

def derive(directory,expected_source=None):
 directory=Path(directory);runtime=json.loads((directory/'runtime.json').read_bytes());phase=json.loads((directory/'phase-one.json').read_bytes());pids=runtime['tc_pids'];source=expected_source or 'file:/'+(directory/'metrics-overlay').as_posix()+'/'
 if len(pids)!=3 or len(set(pids))!=3:raise RuntimeError('Metrics baselines must belong to three distinct actual TC processes')
 values={}
 for pid in pids:
  path=directory/f'metrics-baseline-{pid}.json';value=json.loads(path.read_bytes());validate(value,'baseline',pid,source);values[path.name]=value
 for name,index in [('timeout',0),('restart',1),('lost',1),('second_restart',2),('final',2)]:
  path=directory/f'metrics-{name}-{pids[index]}.json';value=json.loads(path.read_bytes());validate(value,name,pids[index],source,phase['xid']);values[path.name]=value
 begin=json.loads((directory/'console-timeout.json').read_bytes())['data'][0]['beginTime']
 for value in values.values():
  for event in value['events']:
   if event['status']!='AfterCommitted' and event['begin_time']!=begin:raise RuntimeError('Actual metrics original begin timestamp changed across FILE restoration')
 for filename,value in values.items():
  kind=filename.split('-')[1];pid=value['pid'];path=directory/f'metrics-observation-{kind}-{pid}.json';receipt=json.loads(path.read_bytes())
  if receipt.get('pid')!=pid or receipt.get('kind')!=kind or receipt.get('snapshot_sha256')!=hashlib.sha256((directory/filename).read_bytes()).hexdigest():raise RuntimeError('Actual registry observation receipt differs from raw snapshot')
  if kind in ['lost','final']:
   stage='operator_loss' if kind=='lost' else 'operator_replay';marker='ignore unfinished completion metrics, transactionId='+phase['xid'].rsplit(':',1)[1]+', status=CommitRetryTimeout'
   lines=[line for line in (directory/stage/'tc-stdout.log').read_text(encoding='utf-8',errors='replace').splitlines() if marker in line]
   if len(lines)!=1 or receipt.get('markers_before_request')!=1 or receipt.get('marker')!=marker or receipt.get('marker_line_sha256')!=hashlib.sha256(lines[0].encode()).hexdigest():raise RuntimeError('Registry was not observed after actual candidate subscriber processed this nullable event')
  elif receipt.get('markers_before_request')!=0 or receipt.get('marker') is not None or receipt.get('marker_line_sha256') is not None:raise RuntimeError('Unexpected processing barrier for this metric observation')
 return dict(snapshots=values,tc_pids=pids,original_begin_time=begin,registry_reads_reset_windows=False)
