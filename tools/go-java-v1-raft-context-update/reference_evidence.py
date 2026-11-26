"""Require actual report/replica divergence, failed original callback and unchanged business effect."""
import hashlib,importlib.util,json,os,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
wire=module('context_reference_wire',ROOT/'tools/go-java-v1-real-at/verify.py');mixed=module('context_reference_mixed_snapshots',ROOT/'tools/go-java-v1-raft-context-update/reference_snapshot.py');topology=module('context_reference_topology',ROOT/'tools/go-java-v1-raft-context-update/reference_checks.py')

def application_data(value):
 frame=value['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);reader.string()
 if value['code']==11:reader.integer(1);reader.string();reader.string(4)
 else:reader.integer(8);reader.integer(1);reader.string()
 context=reader.string(4);reader.end();return context if context else None

def derive(directory):
 directory=Path(directory);phase=json.loads((directory/'phase-one.json').read_bytes());runtime=json.loads((directory/'runtime.json').read_bytes());requests={};registered={};reports={};acknowledged=set();callbacks={};counts={1:0,7:0,9:0};rejected=[]
 for stage in ['initial','operator_rejected']:
  events=json.loads((directory/stage/'proxy-events.json').read_bytes());result=json.loads((directory/stage/'proxy-result.json').read_bytes())
  if result['errors'] or result['faults'] or [e['sequence'] for e in events]!=list(range(1,len(events)+1)):raise RuntimeError('Actual diagnostic proxy ordering/errors differ')
  rows=[(event,wire.decode(event)) for event in events if event['event']=='wire'];stage_requests={};stage_report_requests={};stage_callbacks={}
  for event,value in rows:
   code=value['code'];key=(value['connection'],value['id']);to_tc=event['direction']=='sdk-to-tc'
   if to_tc and code in counts:counts[code]+=1
   if to_tc and code==11:stage_requests[key]=value
   if not to_tc and code==12:
    request=stage_requests[key];kind=request['branch_type']
    if stage!='initial' or kind in registered or value['result']!=1:raise RuntimeError('Original branch registration missing/replayed/rejected')
    expected=phase['branch_id' if kind==0 else 'tcc_branch_id']
    if (request['xid'],value['branch_id'])!=(phase['xid'],expected):raise RuntimeError('Original context branch identity differs')
    registered[kind]=application_data(request)
   if to_tc and code==13:
    frame=value['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);xid=reader.string();branch=reader.integer(8,True);status=reader.integer(1);reader.string();context=reader.string(4);kind=reader.integer(1);reader.end()
    if stage!='initial' or kind in reports or xid!=phase['xid'] or branch!=phase['branch_id' if kind==0 else 'tcc_branch_id'] or status!={0:2,1:1}[kind]:raise RuntimeError('Original updated branch report missing or changed')
    reports[kind]=context if context else None;stage_report_requests[key]=kind
   if not to_tc and code==14:
    frame=value['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:])
    if (reader.integer(1),reader.integer(1))!=(1,0) or key not in stage_report_requests:raise RuntimeError('Original updated report was not acknowledged')
    reader.end();acknowledged.add(stage_report_requests[key])
   if not to_tc and code==3:
    kind=value['branch_type']
    if (value['xid'],value['branch_id'])!=(phase['xid'],phase['branch_id' if kind==0 else 'tcc_branch_id']):raise RuntimeError('Original actual callback identity differs')
    callbacks.setdefault(stage,[]).append(value);stage_callbacks[key]=value
   if to_tc and code==4 and stage=='operator_rejected':
    request=stage_callbacks.get(key)
    if request is None or (value['xid'],value['branch_id'])!=(request['xid'],request['branch_id']):raise RuntimeError('Actual rejected callback reply lacks original correlation')
    if request['branch_type']==1:
     if value['result']==1 and value['status']==5:raise RuntimeError('Missing context unexpectedly confirmed business')
     rejected.append(dict(result=value['result'],status=value['status'],id=value['id'],connection=value['connection']))
  if stage=='initial':
   withheld=[e for e in events if e['event']=='commit-request-withheld']
   if len(withheld)!=len(callbacks.get(stage,[])) or not withheld:raise RuntimeError('Natural timeout did not withhold real initial requests')
  elif any(e['event'] in ['commit-request-withheld','successful-tcc-response-lost'] for e in events):raise RuntimeError('Reference rejection must come from original callback, not fault injection')
 if counts!={1:1,7:1,9:0} or set(registered)!={0,1} or set(reports)!={0,1} or acknowledged!={0,1}:raise RuntimeError('Original TM/report execution differs')
 if registered[1]==reports[1] or reports[1] is None:raise RuntimeError('Prepare did not actually change reported context')
 updated=json.loads(reports[1])['actionContext'];initial=json.loads(registered[1])['actionContext'];token=hashlib.sha256((phase['xid']+'/'+str(phase['tcc_branch_id'])+'/10').encode()).hexdigest()
 expected={'prepared-units':'10','prepared-branch':str(phase['tcc_branch_id']),'prepared-token':token}
 if any(key in initial or updated.get(key)!=value for key,value in expected.items()):raise RuntimeError('Original registered/reported new business context is not genuine')
 for name in ['timeout','restart','rejected']:
  row=mixed.snapshot(directory/(name+'.tsv'),phase)
  if (row['reserved'],row['confirmed'],row['effect_count'],row['tcc_phase'])!=(10,0,0,'prepared') or row['balance']!=900 or row['undo_status']!=0 or bytes.fromhex(row['undo_hex'])!=bytes.fromhex(phase['undo_hex']):raise RuntimeError('Missing-context original business consequences differ')
  lines=(directory/('prepared-context-'+name+'.tsv')).read_text().splitlines()
  if lines!=['\t'.join([phase['xid'],str(phase['tcc_branch_id']),'10',token])]:raise RuntimeError('Durable real prepared context row changed')
 original=json.loads((directory/'cluster-timeout.json').read_bytes());elected=json.loads((directory/'cluster-election_one.json').read_bytes());restored=json.loads((directory/'cluster-restored.json').read_bytes());topology.verify_stage(original,phase);topology.verify_stage(elected,phase);topology.verify_stage(restored,phase);topology.verify_transition(original,elected)
 observations=[]
 for name,bundle in [('timeout',original),('election_one',elected),('restored',restored)]:
  for key,node in bundle['nodes'].items():
   branch=next(b for b in node['records'][0]['branches'] if b['branch_id']==phase['tcc_branch_id']);context=branch['application_data'];expected_context=reports[1] if int(key)==original['leader'] else registered[1]
   if context!=expected_context:raise RuntimeError('Reference report/replica divergence does not match original unreplicated update')
   observations.append(dict(stage=name,node=int(key),pid=node['pid'],matches_report=context==reports[1],matches_registration=context==registered[1],context=context))
 for request in callbacks.get('initial',[]):
  if request['branch_type']==1:
   frame=request['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);reader.string();reader.integer(8);reader.integer(1);reader.string();context=reader.string(4);reader.end()
   if context!=reports[1]:raise RuntimeError('Original leader callback did not use actual updated report')
 for request in callbacks.get('operator_rejected',[]):
  if request['branch_type']==1:
   frame=request['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);reader.string();reader.integer(8);reader.integer(1);reader.string();context=reader.string(4);reader.end()
   if context!=registered[1]:raise RuntimeError('Elected original callback did not expose old registered context')
 log=(directory/'operator_rejected/sdk-stdout.log').read_bytes();marker=('REAL_TCC_CONTEXT_REJECTED '+phase['xid']+' '+str(phase['tcc_branch_id'])+' '+str(runtime['sdk_pids'][1])).encode()
 if not rejected or marker not in log or b'REAL_TCC_FINISHED ' in log or b'REAL_TCC_CONTEXT_ACCEPTED ' in log:raise RuntimeError('Actual business rejection proof missing')
 response=json.loads((directory/'operator_rejected/operator.json').read_bytes())
 if str(json.loads(response['body']).get('code'))!='500':raise RuntimeError('Original manual commit unexpectedly completed')
 if len(runtime['all_tc_pids'])!=4 or len(set(runtime['all_tc_pids']+runtime['sdk_pids']))!=6 or len(set(runtime['database_starts']))!=2:raise RuntimeError('Original real restart identities differ')
 return dict(expected_defect_reproduced=True,recovery_certified=False,registered_tcc_context=registered[1],acknowledged_report_tcc_context=reports[1],new_context_values=expected,original_replica_observations=observations,rejected_original_tcc_replies=rejected,tm_begin_commit_rollback=[1,1,0],real_business_confirmed=0,real_business_effect_count=0,scope='Observed original status-only replication loses actual updated Prepare context after election; real callback rejects absent context. Not successful recovery.')
