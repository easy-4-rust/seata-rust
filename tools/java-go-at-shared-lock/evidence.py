"""Require two original AT phase-one commits and independently queried lock namespaces."""
import hashlib,importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
wire=module('shared_lock_raw_wire',Path(__file__).resolve().parent/'wire.py');helper=module('shared_lock_replay_helper',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py')
def verify_namespace(value):
 if value['java_xid']==value['go_xid'] or value['java_branch']==value['go_branch']:raise RuntimeError('Independent original global/branch identities required')
 if value['java_resource']==value['go_resource']:raise RuntimeError('Resource namespace divergence not reproduced')
 if (value['java_balance'],value['both_balance'],value['final_balance'])!=(900,800,800):raise RuntimeError('Two actual committed phase-one updates not observed')
 if value['java_undo_branches']!=[value['java_branch']] or sorted(value['both_undo_branches'])!=sorted([value['java_branch'],value['go_branch']]) or value['final_undo_rows']!=0:raise RuntimeError('Original durable AT Undo identities/cleanup differ')
 if value['before']['java-java'] is not False or value['before']['go-java'] is not True:raise RuntimeError('Actual same-key resource-only query does not prove namespace isolation')
 if value['after']['java-java'] is not False or value['after']['go-go'] is not False or not all(value['final'].values()):raise RuntimeError('Actual original locks not held/released as claimed')
 if value['tm_counts']!={'java':[1,1,0],'go':[1,1,0]} or value['original_accepted_branches'] is not True:raise RuntimeError('Original actual TM/branch lifecycle differs')
 return value

def derive(directory):
 directory=Path(directory);runtime=json.loads((directory/'runtime.json').read_bytes());phases={label:json.loads((directory/label/'phase-one.json').read_bytes()) for label in ['java','go']};registered={};tm_counts={};wire_counts={};branches=[]
 if len(set([runtime['tc_pid'],runtime['java_pid'],runtime['go_pid']]))!=3 or runtime['java_exit_code'] or runtime['go_exit_code']:raise RuntimeError('Actual original processes differ')
 for label in ['java','go']:
  phase=phases[label];events=json.loads((directory/label/'proxy-events.json').read_bytes());state=json.loads((directory/label/'proxy-result.json').read_bytes());process=json.loads((directory/label/'process.json').read_bytes())
  if state['errors'] or state['field_translation'] or state['fault_injection'] or process['pid']!=runtime[label+'_pid'] or phase['process_id']!=runtime[label+'_pid']:raise RuntimeError('Actual transparent proxy/process binding differs')
  if [event['sequence'] for event in events]!=list(range(1,len(events)+1)) or any(event['event'] not in ['wire','connected'] for event in events):raise RuntimeError('Actual original event sequence differs')
  requests={};rm=[];accepted_rm=[];branches_requested={};accepted=[];callbacks=[];replies=[];counts=[0,0,0]
  for event in events:
   if event['event']!='wire':continue
   value=wire.decode(event);key=(value['connection'],value['id']);code=value['code'];to_tc=event['direction']=='sdk-to-tc'
   if code in [1,7,9] and to_tc:counts[[1,7,9].index(code)]+=1
   if to_tc and code in [1,7,11,13,15,21,101,103]:
    if (code,*key) in requests:raise RuntimeError('Duplicate original request identity')
    requests[(code,*key)]=value
   if code in [2,8,12,14,16,22,102,104]:
    original={2:1,8:7,12:11,14:13,16:15,22:21,102:101,104:103}[code]
    if to_tc or (original,*key) not in requests:raise RuntimeError('Response not correlated with original request')
   if to_tc and code==103:rm.append(value)
   if not to_tc and code==104:
    request=requests[(103,*key)]
    if not value['identified']:raise RuntimeError('Real RM not identified')
    accepted_rm.append(dict(resource=request['resource'],connection=value['connection'],sequence=value['sequence']))
   if to_tc and code==11:branches_requested[key]=value
   if not to_tc and code==12:
    request=branches_requested[key]
    if value['result']!=1 or request['branch_type']!=0 or (request['xid'],value['branch_id'])!=(phase['xid'],phase['branch_id']):raise RuntimeError('Real accepted AT branch differs')
    accepted.append(request);branches.append(dict(actor=label,xid=phase['xid'],branch_id=phase['branch_id'],resource=request['resource'],lock_key=request['lock_key']))
   if not to_tc and code==3:
    if (value['xid'],value['branch_id'],value['branch_type'])!=(phase['xid'],phase['branch_id'],0):raise RuntimeError('Actual AT Commit callback identity differs')
    callbacks.append(value)
   if to_tc and code==4:replies.append(value)
  if len(accepted)!=1 or len(callbacks)!=1 or len(replies)!=1:raise RuntimeError('One original branch and Commit callback/reply required')
  request=accepted[0];callback=callbacks[0];reply=replies[0]
  if callback['resource']!=request['resource'] or not any(row['resource']==request['resource'] and row['connection']==callback['connection'] and row['sequence']<callback['sequence'] for row in accepted_rm):raise RuntimeError('Original callback must bind accepted real RM resource')
  if (reply['connection'],reply['id'],reply['xid'],reply['branch_id'],reply['result'],reply['status'])!=(callback['connection'],callback['id'],phase['xid'],phase['branch_id'],1,5):raise RuntimeError('Actual original successful RM reply differs')
  registered[label]=request;tm_counts[label]=counts;wire_counts[label]=len([event for event in events if event['event']=='wire'])
 if phases['java']['resource']!=registered['java']['resource']:raise RuntimeError('Java DataSourceProxy resource differs from actual wire')
 def sql_rows(name):
  rows=[line.split('\t') for line in (directory/(name+'.tsv')).read_text().splitlines()]
  if not rows or len(rows[0])!=2 or rows[0][0]!='1':raise RuntimeError('Independent physical row observation missing')
  for row in rows[1:]:
   if len(row)!=4 or row[2]!='0':raise RuntimeError('Original phase-one Undo row/status missing')
   match=[p for p in phases.values() if p['xid']==row[0] and str(p['branch_id'])==row[1]]
   if len(match)!=1 or bytes.fromhex(row[3])!=bytes.fromhex(match[0]['undo_hex']):raise RuntimeError('Original Undo bytes differ from real SDK observation')
  return int(rows[0][1]),[int(row[1]) for row in rows[1:]]
 jb,ju=sql_rows('java-held');bb,bu=sql_rows('both-held');final=(directory/'final.tsv').read_text().splitlines()
 if len(final)!=2 or final[0].split('\t')[0]!='1':raise RuntimeError('Independent final SQL absent')
 result=dict(java_xid=phases['java']['xid'],go_xid=phases['go']['xid'],java_branch=phases['java']['branch_id'],go_branch=phases['go']['branch_id'],java_resource=registered['java']['resource'],go_resource=registered['go']['resource'],java_lock=registered['java']['lock_key'],go_lock=registered['go']['lock_key'],java_balance=jb,both_balance=bb,final_balance=int(final[0].split('\t')[1]),java_undo_branches=ju,both_undo_branches=bu,final_undo_rows=int(final[1]),tm_counts=tm_counts,original_accepted_branches=True)
 for stage,field,status in [('java-held','before',1),('both-held','after',1),('final','final',15)]:
  result[field]={}
  for name,resource,key in [('java-java',result['java_resource'],result['java_lock']),('go-java',result['go_resource'],result['java_lock']),('java-go',result['java_resource'],result['go_lock']),('go-go',result['go_resource'],result['go_lock'])]:
   query=json.loads((directory/f'query-{stage}-{name}.json').read_bytes());frames=query['frames']
   if (query['xid'],query['status'],query['resource'],query['lock_key'])!=(result['go_xid'],status,resource,key) or query['observer_xid'] in [result['java_xid'],result['go_xid']] or query['target_mutations']!=0 or len(frames)!=6:raise RuntimeError('Original cross-query identity differs')
   for row in frames:
    if hashlib.sha256(bytes.fromhex(row['frame_hex'])).hexdigest()!=row['sha256']:raise RuntimeError('Original cross-query frame digest differs')
   requests=[bytes.fromhex(row['frame_hex']) for row in frames if row['direction']=='observer-to-tc'];responses=[bytes.fromhex(row['frame_hex']) for row in frames if row['direction']=='tc-to-observer']
   if [int.from_bytes(frame[16:18],'big') for frame in requests]!=[101,15,21]:raise RuntimeError('Observer must only send read-only TM operations')
   stream=helper.ReplayStream(b''.join(responses));observed=helper.observer.query(stream,result['go_xid'],resource,key,status,query['lockable'])
   if stream.data or bytes(stream.sent)!=b''.join(requests) or observed!=query:raise RuntimeError('Cross-query does not reproduce original frames')
   result[field][name]=query['lockable']
 for label in ['java-held','both-java','both-go','final-java','final-go']:
  value=json.loads((directory/('console-'+label+'.json')).read_bytes());actor='go' if label.endswith('go') else 'java';phase=phases[actor];done=label.startswith('final')
  if str(value.get('code'))!='200' or value.get('total')!=(0 if done else 1) or len(value.get('data',[]))!=(0 if done else 1):raise RuntimeError('Actual console session count differs')
  if not done:
   session=value['data'][0];items=session.get('branchSessionVOs') or []
   if session['xid']!=phase['xid'] or session['status']!=1 or len(items)!=1:raise RuntimeError('Both original transactions must remain Begin while observing')
   branch=items[0]
   if int(branch['branchId'])!=phase['branch_id'] or branch['resourceId']!=registered[actor]['resource'] or branch['branchType']!='AT':raise RuntimeError('Actual console original branch binding differs')
 verified=verify_namespace(result);verified.update(expected_defect_reproduced=True,shared_global_mutex_certified=False,original_branches=branches,wire_frame_counts=wire_counts,native_query_count=12,native_frame_count=72,scope='Actual original default Java/Go namespace divergence on one physical MySQL row; diagnostic only, not safe shared locking or Undo interchange.');return verified
