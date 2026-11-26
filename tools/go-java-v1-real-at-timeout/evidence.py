"""Derive recovery claims from raw, tagged SQL snapshots and immutable actual wire frames."""
import importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('timeout_wire_decoder',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)

def snapshot(path,phase):
 rows=[line.split('\t') for line in Path(path).read_text(encoding='utf-8').splitlines()]
 groups={name:[row[1:] for row in rows if row[0]==name] for name in ['business','undo','global','branch','lock']}
 if not rows or len(rows)!=sum(len(v) for v in groups.values()) or groups['business'] not in [[['1','900']],[['1','1000']]]:raise RuntimeError('Missing or unexpected independent business snapshot')
 xid=phase['xid'];branch=phase['branch_id']
 for name in ['global','branch','lock','undo']:
  if len(groups[name])>1:raise RuntimeError('Unexpected owned database row count')
  if groups[name] and groups[name][0][0]!=xid:raise RuntimeError('Stored XID differs: '+name)
 for name in ['branch','lock','undo']:
  if groups[name] and int(groups[name][0][1])!=branch:raise RuntimeError('Stored branch differs: '+name)
 return dict(balance=int(groups['business'][0][1]),global_status=int(groups['global'][0][1]) if groups['global'] else None,branches=len(groups['branch']),locks=len(groups['lock']),undo_status=int(groups['undo'][0][2]) if groups['undo'] else None,undo_hex=groups['undo'][0][3] if groups['undo'] else None,xid=xid,branch_id=branch,stored=groups)

def derive(directory):
 directory=Path(directory);phase=json.loads((directory/'phase-one.json').read_bytes());snapshots={n:snapshot(directory/(n+'.tsv'),phase) for n in ['timeout','restart','lost','second_restart','final']}
 for name in ['timeout','restart','lost','second_restart','final']:
  body=json.loads((directory/('console-'+name.replace('_','-')+'.json')).read_bytes());final=name=='final';data=body.get('data',[])
  if str(body.get('code'))!='200' or body.get('total')!=(0 if final else 1) or len(data)!=(0 if final else 1):raise RuntimeError('Actual console persisted session count differs')
  if not final:
   row=data[0];branches=row.get('branchSessionVOs') or []
   if row.get('xid')!=phase['xid'] or row.get('status')!=17 or len(branches)!=1 or str(branches[0].get('branchId'))!=str(phase['branch_id']):raise RuntimeError('Actual console original timeout/branch differs')
 log=(directory/'initial/tc-stdout.log').read_bytes()
 if ('The Global session '+phase['xid']+' has changed the status to RollbackRetryTimeout, need to be handled it manually.').encode() not in log:raise RuntimeError('Natural TC scheduler timeout transition missing')
 stages=[];registrations=[];errors=[];withheld=lost=begins=rollbacks=0;original_branch=False;correlations=True;resources=[];callback_resources=[]
 for name in ['initial','operator_loss','operator_replay']:
  stage=directory/name;events=json.loads((stage/'proxy-events.json').read_bytes());state=json.loads((stage/'proxy-result.json').read_bytes());errors.extend(state['errors'])
  if [event['sequence'] for event in events]!=list(range(1,len(events)+1)):raise RuntimeError('Ambiguous actual event ordering')
  if any(event['event'] not in ['connected','wire','operator-command-start','rollback-request-withheld','successful-rollback-response-lost'] for event in events):raise RuntimeError('Unexpected proxy control event')
  if state['faults'] or state['fragmented_directions']!=['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Unexpected proxy mutations or untested fragmentation')
  rows=[(e,wire.decode(e)) for e in events if e['event']=='wire'];regs={};requests={};accepted=[];callbacks=[]
  commands=[event for event in events if event['event']=='operator-command-start']
  if name!='initial' and (len(commands)!=1 or commands[0].get('xid')!=phase['xid']):raise RuntimeError('Actual operator boundary missing or ambiguous')
  for event,value in rows:
   code=value['code'];key=(value['connection'],value['id']);to_tc=event['direction']=='sdk-to-tc'
   if to_tc and code==1:begins+=1
   if to_tc and code==9:rollbacks+=1
   if to_tc and code==103:regs[key]=value
   if not to_tc and code==104:
    if key not in regs or not value['identified']:raise RuntimeError('Unmatched/rejected real RM registration')
    accepted.append(dict(resource=regs[key]['resource'],connection=value['connection'],sequence=value['sequence']))
   if to_tc and code==11:requests[key]=value
   if not to_tc and code==12:
    if key not in requests or value['result']!=1:raise RuntimeError('Unmatched real TC branch registration')
    reg=requests[key];resources.append(reg['resource'])
    if (reg['xid'],value['branch_id'],reg['branch_type'],reg['lock_key'])==(phase['xid'],phase['branch_id'],0,'ACCOUNTS:1;'):original_branch=True
   if not to_tc and code==5:
    callback_resources.append(value['resource'])
    if name!='initial' and event['sequence']<=commands[0]['sequence']:raise RuntimeError('Timeout17 replayed before the operator command')
    if (value['xid'],value['branch_id'],value['branch_type'])!=(phase['xid'],phase['branch_id'],0):raise RuntimeError('Actual callback changed original identity')
    if not any(r['resource']==value['resource'] and r['connection']==value['connection'] and r['sequence']<value['sequence'] for r in accepted):raise RuntimeError('Callback precedes accepted real resource registration')
    callbacks.append(value)
   if to_tc and code==6:
    matching=[r for r in callbacks if (r['connection'],r['id'])==key]
    if len(matching)!=1 or (value['xid'],value['branch_id'],value['result'],value['status'])!=(phase['xid'],phase['branch_id'],1,8):correlations=False
  drops={kind:[event for event in events if event['event']==kind] for kind in ['rollback-request-withheld','successful-rollback-response-lost']}
  if name=='initial':
   if len(drops['rollback-request-withheld'])!=len(callbacks) or drops['successful-rollback-response-lost']:raise RuntimeError('Initial actual rollback withholding differs')
  elif name=='operator_loss':
   if drops['rollback-request-withheld'] or len(drops['successful-rollback-response-lost'])!=1:raise RuntimeError('Actual registered callback/drop evidence differs at first operator')
  elif any(drops.values()):raise RuntimeError('Final replay response must be forwarded')
  for event in events:
   if event['event'] in ['rollback-request-withheld','successful-rollback-response-lost']:
    value=wire.decode(event);kind=5 if event['event']=='rollback-request-withheld' else 6
    actual=[r for _,r in rows if (r['frame'],r['connection'],r['id'],r['code'])==(value['frame'],value['connection'],value['id'],kind)]
    if len(actual)!=1 or event['sequence']<=actual[0]['sequence']:raise RuntimeError('Suppressed frame is not the actual preceding observed wire frame')
    withheld+=kind==5;lost+=kind==6
  for request in callbacks:
   if name=='initial' and sum(event['frame_hex']==request['frame'].hex() and event['connection']==request['connection'] and event['id']==request['id'] for event in drops['rollback-request-withheld'])!=1:raise RuntimeError('Not every actual initial request was withheld exactly once')
  if name=='initial' and (not callbacks or any(v['code']==6 for _,v in rows)):raise RuntimeError('Initial rollback was not fully withheld')
  if name!='initial' and (len(callbacks)!=1 or sum(v['code']==6 for _,v in rows)!=1):raise RuntimeError('Need exactly one actual rollback/ack per operator attempt')
  registrations.append(bool(accepted));stages.append(dict(stage=name,connections=state['connections'],callbacks=len(callbacks),registrations=len(accepted)))
 if len(set(resources))!=1:raise RuntimeError('Original resource identity missing or ambiguous')
 resource=resources[0]
 if any(candidate!=resource for candidate in callback_resources):raise RuntimeError('Callback changed the original registered resource')
 for name,row in snapshots.items():
  for stored in row['stored']['branch']:
   if stored[3:]!=[resource,'AT']:raise RuntimeError('Stored resource/type differs after restart')
  for stored in row['stored']['lock']:
   if stored[2:5]!=[resource,'ACCOUNTS','1']:raise RuntimeError('Stored original lock identity differs')
 runtime=json.loads((directory/'runtime.json').read_bytes());operators=[json.loads((directory/name/'operator.json').read_bytes()) for name in ['operator_loss','operator_replay']]
 if len(runtime['tc_pids'])!=3 or len(set(runtime['tc_pids']))!=3:raise RuntimeError('Three separate TC processes are required')
 for index,name in enumerate(['initial','operator_loss','operator_replay']):
  data=(directory/name/'sdk-stdout.log').read_bytes();mode=b'prepare' if index==0 else b'serve';pid=str(runtime['sdk_pids'][index]).encode()
  if b'REAL_TIMEOUT_PROCESS '+mode+b' '+pid not in data or b'REAL_TIMEOUT_SERVING '+pid not in data:raise RuntimeError('Actual SDK process startup marker differs')
 if phase['process_id']!=runtime['sdk_pids'][0]:raise RuntimeError('Phase-one proof did not come from original SDK process')
 loss_log=(directory/'operator_loss/tc-stdout.log').read_bytes()
 if ('change global status fail, xid:'+phase['xid']).encode() not in loss_log or ('Send branch rollback failed, xid = '+phase['xid']+' branchId = '+str(phase['branch_id'])).encode() not in loss_log or b'Caused by: java.util.concurrent.TimeoutException:' not in loss_log:raise RuntimeError('First operator failure is not the original branch response timeout')
 # HTTP alone is not sufficient; require the precise accepted code on the successful replay.
 if operators[1]['http_status']!=200 or str(json.loads(operators[1]['body']).get('code'))!='200':raise RuntimeError('Actual second operator command was not accepted')
 return dict(phase=phase,snapshots=snapshots,processes=runtime['sdk_pids'],database_starts=runtime['database_starts'],request_withheld=withheld,successful_response_lost=lost,registered_stages=registrations,original_branch=original_branch,callback_correlations=correlations,tm_begins=begins,tm_rollbacks=rollbacks,operator_results=[o['http_status'] for o in operators],proxy_errors=errors,stages=stages)
