"""Bind real mixed-mode branches, persisted effects, operator boundaries and lost frames."""
import hashlib,importlib.util,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('commit_actual_wire',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)
spec=importlib.util.spec_from_file_location('file_commit_journal_helpers',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py');file_helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(file_helpers)
ACTION='owned-real-commit-action'

def snapshot(path,phase):
 path=Path(path);name=path.stem;rows=[line.split('\t') for line in path.read_text(encoding='utf-8').splitlines()]
 names=['business','undo','commit_account','reservation'];groups={key:[row[1:] for row in rows if row[0]==key] for key in names};widths=dict(business=2,undo=4,commit_account=3,reservation=4)
 if not rows or len(rows)!=sum(map(len,groups.values())) or any(len(row)!=widths[key] for key,values in groups.items() for row in values) or groups['business']!=[['1','900']] or len(groups['undo'])>1 or len(groups['commit_account'])!=1 or len(groups['reservation'])!=1:raise RuntimeError('Independent mixed business snapshot missing or malformed')
 undo=groups['undo'][0] if groups['undo'] else None
 if undo and (undo[0]!=phase['xid'] or int(undo[1])!=phase['branch_id']):raise RuntimeError('Original AT Undo identity differs')
 account=groups['commit_account'][0];reservation=groups['reservation'][0]
 if account[0]!='1' or reservation[0]!=phase['xid'] or int(reservation[1])!=phase['tcc_branch_id']:raise RuntimeError('Original persistent TCC business identity differs')
 body=json.loads((path.parent/('console-'+name.replace('_','-')+'.json')).read_bytes());data=body.get('data',[]);final=name=='final';before=name in ['timeout','restart']
 if str(body.get('code'))!='200' or body.get('total')!=(0 if final else 1) or len(data)!=(0 if final else 1):raise RuntimeError('Actual RAFT console session count differs')
 branches=[]
 if not final:
  session=data[0];branches=session.get('branchSessionVOs') or [];expected={phase['branch_id'],phase['tcc_branch_id']} if before else {phase['tcc_branch_id']}
  if session.get('xid')!=phase['xid'] or session.get('status')!=16 or {int(branch['branchId']) for branch in branches}!=expected or len(branches)!=len(expected):raise RuntimeError('Actual RAFT console original16 branches differ')
  for branch in branches:
   at=int(branch['branchId'])==phase['branch_id']
   if branch.get('xid')!=phase['xid'] or branch.get('branchType')!=('AT' if at else 'TCC') or branch.get('resourceId')!=(phase['resource_id'] if at else ACTION) or branch.get('status') is None:raise RuntimeError('Original RAFT branch resource/type/status differs')
 native=json.loads((path.parent/('native-'+name+'.json')).read_bytes());frames=native['frames']
 if frames!=json.loads((path.parent/('native-'+name+'-frames.json')).read_bytes()) or len(frames)!=6 or any(hashlib.sha256(bytes.fromhex(frame['frame_hex'])).hexdigest()!=frame['sha256'] for frame in frames):raise RuntimeError('Actual native trace/hash differs')
 requests=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='observer-to-tc'];responses=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='tc-to-observer']
 if len(requests)!=3 or len(responses)!=3 or [int.from_bytes(frame[16:18],'big') for frame in requests]!=[101,15,21]:raise RuntimeError('Native observer must send only read-only TM queries')
 stream=file_helpers.ReplayStream(b''.join(responses));observed=file_helpers.observer.query(stream,phase['xid'],phase['resource_id'],'ACCOUNTS:1;',15 if final else 16,True)
 if stream.data or bytes(stream.sent)!=b''.join(requests) or observed!=native:raise RuntimeError('Actual native frames do not reproduce RAFT commit query')
 return dict(xid=phase['xid'],balance=900,status=None if final else 16,at_branch=phase['branch_id'] if before else None,tcc_branch=None if final else phase['tcc_branch_id'],locks=0,undo_hex=undo[3] if undo else None,undo_status=int(undo[2]) if undo else None,reserved=int(account[1]),confirmed=int(account[2]),effect_count=int(reservation[3]),tcc_phase=reservation[2],tcc_xid=reservation[0],tcc_effect_branch=int(reservation[1]),stored=groups,tc_branches=branches)

def derive(directory):
 directory=Path(directory);phase=json.loads((directory/'phase-one.json').read_bytes());snapshots={name:snapshot(directory/(name+'.tsv'),phase) for name in ['timeout','restart','lost','second_restart','final']};runtime=json.loads((directory/'runtime.json').read_bytes())
 if sorted((directory/'business-tables.txt').read_text().splitlines())!=['accounts','commit_account','commit_reservation','undo_log']:raise RuntimeError('TC state must not live in the actual business database')
 ids={0:phase['branch_id'],1:phase['tcc_branch_id']};branch_types={value:kind for kind,value in ids.items()};resources={};branch_reports={};stage_report=[];registrations=[];errors=[];begins=commits=rollbacks=withheld=lost=0;all_callbacks=[];correlations=True
 if ('The Global session '+phase['xid']+' has changed the status to CommitRetryTimeout, need to be handled it manually.').encode() not in (directory/'initial/tc-stdout.log').read_bytes():raise RuntimeError('Natural16 scheduler transition missing')
 for name in ['initial','operator_loss','operator_replay']:
  stage=directory/name;events=json.loads((stage/'proxy-events.json').read_bytes());state=json.loads((stage/'proxy-result.json').read_bytes());errors+=state['errors']
  if state['faults'] or state['fragmented_directions']!=['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Unexpected mixed proxy mutation')
  if [e['sequence'] for e in events]!=list(range(1,len(events)+1)) or any(e['event'] not in ['connected','wire','operator-command-start','commit-request-withheld','successful-tcc-response-lost'] for e in events):raise RuntimeError('Ambiguous actual proxy ordering/control')
  rows=[(event,wire.decode(event)) for event in events if event['event']=='wire'];regs={};accepted=[];branches={};callbacks=[];replies=[];rpc_requests={};rpc_replies=set();accepted_tm=[];commands=[e for e in events if e['event']=='operator-command-start']
  process=json.loads((stage/'tc-process.json').read_bytes());config_args=[arg for arg in process['command'] if arg.startswith('-Dspring.config.location=file:')]
  if len(config_args)!=1 or Path(config_args[0].split('file:',1)[1]).drive.upper() not in ['D:','E:']:raise RuntimeError('Actual TC command does not bind the owned RAFT journal path')
  if b'  store:\n    mode: raft\n    session:\n      mode: raft\n    lock:\n      mode: raft\n' not in (stage/'application.yml').read_bytes():raise RuntimeError('Actual TC configuration must explicitly use RAFT session/lock storage')
  tc_log=(stage/'tc-stdout.log').read_bytes()
  index=['initial','operator_loss','operator_replay'].index(name)
  if process['pid']!=runtime['tc_pids'][index] or (' with PID '+str(process['pid'])+' (').encode() not in tc_log:raise RuntimeError('Actual TC startup PID differs from fresh-process proof')
  if b'use session store mode: raft' not in tc_log or b'use lock store mode: raft' not in tc_log:raise RuntimeError('Actual active TC session/lock backend is not RAFT')
  if name!='initial' and (len(commands)!=1 or commands[0]['xid']!=phase['xid']):raise RuntimeError('Operator boundary missing')
  for event,value in rows:
   code=value['code'];key=(value['connection'],value['id']);to_tc=event['direction']=='sdk-to-tc'
   expected={1:(True,{0}),2:(False,{1}),3:(False,{0}),4:(True,{1}),7:(True,{0}),8:(False,{1}),11:(True,{0}),12:(False,{1}),13:(True,{0}),14:(False,{1}),101:(True,{0,2}),102:(False,{1}),103:(True,{0}),104:(False,{1})}
   if event['direction'] not in ['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Unknown actual wire direction')
   if code is None:
    if (event['direction'],value['kind']) not in [('sdk-to-tc',3),('tc-to-sdk',4)]:raise RuntimeError('Actual heartbeat envelope differs')
   elif code not in expected or to_tc!=expected[code][0] or value['kind'] not in expected[code][1]:raise RuntimeError('Actual RAFT recovery protocol direction/envelope differs')
   request_codes={2:1,8:7,12:11,14:13,102:101,104:103}
   if to_tc and code in request_codes.values():
    rpc_key=(code,*key)
    if rpc_key in rpc_requests:raise RuntimeError('Ambiguous original SDK RPC request identity')
    rpc_requests[rpc_key]=value
   if code in request_codes:
    rpc_key=(request_codes[code],*key)
    if rpc_key not in rpc_requests or rpc_key in rpc_replies:raise RuntimeError('Unmatched or duplicate actual SDK RPC response')
    rpc_replies.add(rpc_key)
   if code==102:
    if not value['identified']:raise RuntimeError('Actual full SDK TM registration rejected')
    accepted_tm.append(value)
   if code in [2,7]:
    head=int.from_bytes(value['frame'][7:9],'big');reader=wire.Reader(value['frame'][head+2:])
    if code==2 and (reader.integer(1),reader.integer(1))!=(1,0):raise RuntimeError('Actual Begin response not successful')
    if reader.string()!=phase['xid']:raise RuntimeError('Original TM Begin/Commit XID differs from actual business transaction')
    reader.string();reader.end()
   if code==13:
    head=int.from_bytes(value['frame'][7:9],'big');reader=wire.Reader(value['frame'][head+2:]);report=dict(xid=reader.string(),branch_id=reader.integer(8,True),status=reader.integer(1));resource=reader.string();application_data=reader.string(4);kind=reader.integer(1);reader.end()
    if name!='initial' or kind not in ids or kind in branch_reports or report['xid']!=phase['xid'] or report['branch_id']!=ids[kind] or report['status']!={0:2,1:1}[kind] or resource not in ['',phase['resource_id'] if kind==0 else ACTION]:raise RuntimeError('Original real AT/TCC phase-one report differs')
    report.update(branch_type=kind,resource=resource if resource else None,application_data=application_data if application_data else None,connection=value['connection'],request_id=value['id'],accepted=False);branch_reports[kind]=report
   if code==14:
    head=int.from_bytes(value['frame'][7:9],'big');reader=wire.Reader(value['frame'][head+2:])
    if (reader.integer(1),reader.integer(1))!=(1,0):raise RuntimeError('Actual original branch report rejected')
    reader.end()
    reports=[report for report in branch_reports.values() if (report['connection'],report['request_id'])==key]
    if len(reports)!=1 or reports[0]['accepted']:raise RuntimeError('Successful branch report acknowledgement lacks its unique original report')
    reports[0]['accepted']=True;reports[0]['acknowledgement_sequence']=event['sequence']
   if to_tc and code==1:begins+=1
   if to_tc and code==7:commits+=1
   if to_tc and code==9:rollbacks+=1
   if to_tc and code==103:regs[key]=value
   if not to_tc and code==104:
    if key not in regs or not value['identified']:raise RuntimeError('Rejected/unmatched real RM registration')
    accepted.append(dict(resource=regs[key]['resource'],connection=value['connection'],sequence=value['sequence']))
   if to_tc and code==11:branches[key]=value
   if not to_tc and code==12:
    if key not in branches or value['result']!=1:raise RuntimeError('Actual branch registration rejected/unmatched')
    branch=branches[key];kind=branch['branch_type']
    if kind not in ids or value['branch_id']!=ids[kind] or branch['xid']!=phase['xid'] or branch['lock_key']!=('ACCOUNTS:1;' if kind==0 else '') or kind in resources:raise RuntimeError('Original mixed branch/lock mapping differs')
    resources[kind]=branch['resource']
   if not to_tc and code==3:
    kind=value['branch_type']
    if kind not in ids or (value['xid'],value['branch_id'])!=(phase['xid'],ids[kind]):raise RuntimeError('Commit callback original branch/type differs')
    if name!='initial' and event['sequence']<=commands[0]['sequence']:raise RuntimeError('Retry16 replayed before operator command')
    if not any(reg['connection']==value['connection'] and reg['resource']==value['resource'] and reg['sequence']<value['sequence'] for reg in accepted):raise RuntimeError('Callback precedes actual resource identification')
    head=int.from_bytes(value['frame'][7:9],'big');reader=wire.Reader(value['frame'][head+2:]);reader.string();reader.integer(8);reader.integer(1);reader.string();application_data=reader.string(4);reader.end()
    if kind not in branch_reports or branch_reports[kind]['accepted'] is not True:raise RuntimeError('Actual commit callback precedes accepted original phase-one report')
    if (application_data if application_data else None)!=branch_reports[kind]['application_data']:raise RuntimeError('Actual commit callback changed original branch action context')
    callbacks.append(value);all_callbacks.append(value)
   if to_tc and code==4:
    matched=[request for request in callbacks if (request['connection'],request['id'])==key]
    if len(matched)!=1 or (value['xid'],value['branch_id'],value['result'],value['status'])!=(phase['xid'],matched[0]['branch_id'],1,5):raise RuntimeError('Actual successful Commit response correlation differs')
    replies.append(value)
  drops={kind:[e for e in events if e['event']==kind] for kind in ['commit-request-withheld','successful-tcc-response-lost']}
  if name=='initial':
   if not callbacks or replies or len(drops['commit-request-withheld'])!=len(callbacks) or drops['successful-tcc-response-lost']:raise RuntimeError('Initial real Commit requests not fully withheld')
   if {v['branch_type'] for v in callbacks}!={0,1}:raise RuntimeError('Both actual AT/TCC commit paths must be blocked')
  elif name=='operator_loss':
   if len(callbacks)!=2 or {v['branch_type'] for v in callbacks}!={0,1} or len(replies)!=2 or drops['commit-request-withheld'] or len(drops['successful-tcc-response-lost'])!=1:raise RuntimeError('First operator must lose only one TCC success and complete real AT')
   if {reply['branch_id'] for reply in replies}!=set(ids.values()):raise RuntimeError('First operator requires one AT and one TCC success response')
  elif len(callbacks)!=1 or callbacks[0]['branch_type']!=1 or len(replies)!=1 or any(drops.values()):raise RuntimeError('Final original TCC replay must be delivered successfully')
  for event in drops['commit-request-withheld']+drops['successful-tcc-response-lost']:
   value=wire.decode(event);code=3 if event['event']=='commit-request-withheld' else 4;actual=[r for _,r in rows if (r['frame'],r['connection'],r['id'],r['code'])==(value['frame'],value['connection'],value['id'],code)]
   if len(actual)!=1 or event['sequence']<=actual[0]['sequence']:raise RuntimeError('Dropped frame is not actual preceding original wire')
   if code==4 and value['branch_id']!=ids[1]:raise RuntimeError('Only actual TCC response may be lost')
   withheld+=code==3;lost+=code==4
  for request in callbacks:
   if name!='initial' and sum((reply['connection'],reply['id'],reply['branch_id'])==(request['connection'],request['id'],request['branch_id']) for reply in replies)!=1:raise RuntimeError('Each operator callback requires one original success response')
   if name=='initial' and sum(e['frame_hex']==request['frame'].hex() and (e['connection'],e['id'])==(request['connection'],request['id']) for e in drops['commit-request-withheld'])!=1:raise RuntimeError('Each initial Commit request must be withheld once')
  if not accepted_tm:raise RuntimeError('Actual full SDK TM identification missing')
  registrations.append({r['resource'] for r in accepted}.issuperset(set(resources.values())) and len(resources)==2);stage_report.append(dict(stage=name,registrations=len(accepted),callbacks=len(callbacks),success_responses=len(replies),connections=state['connections']))
 if set(resources)!={0,1} or resources[1]!=ACTION or resources[0]==resources[1]:raise RuntimeError('Two distinct real resources missing')
 for callback in all_callbacks:
  if callback['resource']!=resources[callback['branch_type']]:raise RuntimeError('Callback changed original resource')
 if resources.get(0)!=phase['resource_id']:raise RuntimeError('Original actual AT resource differs from phase-one proof')
 if set(branch_reports)!={0,1} or not all(report['accepted'] for report in branch_reports.values()):raise RuntimeError('Original accepted AT and TCC phase-one reports missing')
 for value in snapshots.values():
  for branch in value['tc_branches']:
   kind=branch_types[int(branch['branchId'])]
   if branch['resourceId']!=resources[kind]:raise RuntimeError('Retained RAFT branch original resource differs')
   if branch['status']!=branch_reports[kind]['status'] or 'applicationData' not in branch or branch['applicationData']!=branch_reports[kind]['application_data']:raise RuntimeError('Retained RAFT branch original status/action context differs')
 tcc_statuses={branch['status'] for value in snapshots.values() for branch in value['tc_branches'] if int(branch['branchId'])==ids[1]}
 if len(tcc_statuses)!=1:raise RuntimeError('Retained original TCC branch status changed across RAFT recovery')
 runtime=json.loads((directory/'runtime.json').read_bytes());operators=[json.loads((directory/name/'operator.json').read_bytes()) for name in ['operator_loss','operator_replay']];results=[(o['http_status'],str(json.loads(o['body']).get('code'))) for o in operators]
 for index,name in enumerate(['initial','operator_loss','operator_replay']):
  raw=(directory/name/'sdk-stdout.log').read_bytes();pid=str(runtime['sdk_pids'][index]).encode();mode=b'prepare' if index==0 else b'serve'
  if b'REAL_COMMIT_PROCESS '+mode+b' '+pid not in raw or b'REAL_COMMIT_SERVING '+pid not in raw:raise RuntimeError('Actual complete SDK process identity differs')
  if index>0 and ('REAL_TCC_FINISHED '+phase['xid']+' '+str(ids[1])+' committed 1 '+str(runtime['sdk_pids'][index])).encode() not in raw:raise RuntimeError('Real TCC business confirmation proof missing')
 if phase['process_id']!=runtime['sdk_pids'][0]:raise RuntimeError('Original phase-one process differs')
 loss_log=(directory/'operator_loss/tc-stdout.log').read_bytes();target=("request:BranchCommitRequest{xid='"+phase['xid']+"', branchId="+str(ids[1])+", branchType=TCC").encode()
 if not any(b'wait response error:' in line and target in line for line in loss_log.splitlines()) or ('Committing global transaction is NOT done, xid = '+phase['xid']).encode() not in loss_log or ('Commit branch transaction successfully, xid = '+phase['xid']+' branchId = '+str(ids[0])).encode() not in loss_log:raise RuntimeError('First operator lacks exact original TCC response timeout and real AT completion logs')
 return dict(original_branch_reports=[branch_reports[kind] for kind in [0,1]],phase=phase,snapshots=snapshots,sdk_pids=runtime['sdk_pids'],tc_pids=runtime['tc_pids'],database_starts=runtime['database_starts'],withheld=withheld,lost=lost,tm_begins=begins,tm_commits=commits,tm_rollbacks=rollbacks,stages_registered=registrations,callback_correlations=correlations,original_branches=len(resources)==2,operator_results=results,proxy_errors=errors,stages=stage_report)
