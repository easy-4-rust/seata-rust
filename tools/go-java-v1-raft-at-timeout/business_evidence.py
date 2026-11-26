"""Derive recovery claims from raw, tagged SQL snapshots and immutable actual wire frames."""
import hashlib,importlib.util,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('timeout_wire_decoder',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)
spec=importlib.util.spec_from_file_location('file_timeout_readonly_observer',ROOT/'tools/java-raft-reload-candidate/native_observer.py');observer=importlib.util.module_from_spec(spec);spec.loader.exec_module(observer)

def snapshot(path,phase):
 path=Path(path);name=path.stem;rows=[line.split('\t') for line in path.read_text(encoding='utf-8').splitlines()]
 groups={key:[row[1:] for row in rows if row[0]==key] for key in ['business','undo']}
 if not rows or len(rows)!=sum(len(v) for v in groups.values()) or groups['business'] not in [[['1','900']],[['1','1000']]] or len(groups['undo'])>1:raise RuntimeError('Missing or unexpected independent business/Undo SQL')
 undo=groups['undo'][0] if groups['undo'] else None
 if undo and (len(undo)!=4 or undo[0]!=phase['xid'] or int(undo[1])!=phase['branch_id']):raise RuntimeError('Original SQL Undo identity differs')
 console=json.loads((path.parent/('console-'+name.replace('_','-')+'.json')).read_bytes());data=console.get('data',[]);final=name=='final'
 if str(console.get('code'))!='200' or console.get('total')!=(0 if final else 1) or len(data)!=(0 if final else 1):raise RuntimeError('Actual RAFT console session count differs')
 branch_status=None
 if not final:
  session=data[0];branches=session.get('branchSessionVOs') or []
  if session.get('xid')!=phase['xid'] or session.get('status')!=17 or len(branches)!=1:raise RuntimeError('Actual RAFT original session/branch differs')
  branch=branches[0];branch_status=branch.get('status')
  if str(branch.get('branchId'))!=str(phase['branch_id']) or branch.get('resourceId')!=phase['resource_id'] or branch_status is None:raise RuntimeError('Actual RAFT branch identity/resource/status differs')
 native=json.loads((path.parent/('native-'+name+'.json')).read_bytes());frames=native['frames']
 if frames!=json.loads((path.parent/('native-'+name+'-frames.json')).read_bytes()):raise RuntimeError('Native raw trace differs from query proof')
 if len(frames)!=6 or any(hashlib.sha256(bytes.fromhex(frame['frame_hex'])).hexdigest()!=frame['sha256'] for frame in frames):raise RuntimeError('Native observer original frame hash differs')
 requests=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='observer-to-tc'];responses=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='tc-to-observer']
 if len(requests)!=3 or len(responses)!=3 or [int.from_bytes(f[16:18],'big') for f in requests]!=[101,15,21]:raise RuntimeError('Observer must use only read-only TM queries')
 stream=ReplayStream(b''.join(responses));observed=observer.query(stream,phase['xid'],phase['resource_id'],'ACCOUNTS:1;',15 if final else 17,final)
 if stream.data or bytes(stream.sent)!=b''.join(requests) or observed!=native:raise RuntimeError('Actual native frames do not reproduce original query proof')
 return dict(balance=int(groups['business'][0][1]),global_status=None if final else 17,branches=0 if final else 1,locks=0 if final else 1,undo_status=int(undo[2]) if undo else None,undo_hex=undo[3] if undo else None,xid=phase['xid'],branch_id=phase['branch_id'],branch_status=branch_status,source='business SQL plus actual RAFT console/native',stored=groups)

class ReplayStream:
 def __init__(self,data):self.data=data;self.sent=bytearray()
 def recv(self,size):part=self.data[:min(size,3)];self.data=self.data[len(part):];return part
 def sendall(self,data):self.sent.extend(data)

def derive(directory):
 directory=Path(directory);phase=json.loads((directory/'phase-one.json').read_bytes());snapshots={n:snapshot(directory/(n+'.tsv'),phase) for n in ['timeout','restart','lost','second_restart','final']};runtime=json.loads((directory/'runtime.json').read_bytes())
 log=(directory/'initial/tc-stdout.log').read_bytes()
 if ('The Global session '+phase['xid']+' has changed the status to RollbackRetryTimeout, need to be handled it manually.').encode() not in log:raise RuntimeError('Natural TC scheduler timeout transition missing')
 stages=[];registrations=[];errors=[];withheld=lost=begins=rollbacks=0;original_branch=False;correlations=True;resources=[];callback_resources=[]
 for name in ['initial','operator_loss','operator_replay']:
  stage=directory/name;events=json.loads((stage/'proxy-events.json').read_bytes());state=json.loads((stage/'proxy-result.json').read_bytes());errors.extend(state['errors'])
  if [event['sequence'] for event in events]!=list(range(1,len(events)+1)):raise RuntimeError('Ambiguous actual event ordering')
  if any(event['event'] not in ['connected','wire','operator-command-start','rollback-request-withheld','successful-rollback-response-lost'] for event in events):raise RuntimeError('Unexpected proxy control event')
  if state['faults'] or state['fragmented_directions']!=['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Unexpected proxy mutations or untested fragmentation')
  rows=[(e,wire.decode(e)) for e in events if e['event']=='wire'];regs={};requests={};accepted=[];callbacks=[];rpc_requests={};rpc_replies=set();accepted_tm=[]
  process=json.loads((stage/'tc-process.json').read_bytes());config_args=[arg for arg in process['command'] if arg.startswith('-Dspring.config.location=file:')]
  if len(config_args)!=1 or Path(config_args[0].split('file:',1)[1]).drive.upper() not in ['D:','E:']:raise RuntimeError('Actual TC command does not bind an owned RAFT configuration')
  tc_log=(stage/'tc-stdout.log').read_bytes()
  index=['initial','operator_loss','operator_replay'].index(name)
  if process['pid']!=runtime['tc_pids'][index] or (' with PID '+str(process['pid'])+' (').encode() not in tc_log:raise RuntimeError('Actual TC startup PID differs from fresh-process proof')
  if b'use session store mode: raft' not in tc_log or b'use lock store mode: raft' not in tc_log:raise RuntimeError('Actual active TC session/lock backend is not RAFT')
  commands=[event for event in events if event['event']=='operator-command-start']
  if name!='initial' and (len(commands)!=1 or commands[0].get('xid')!=phase['xid']):raise RuntimeError('Actual operator boundary missing or ambiguous')
  for event,value in rows:
   code=value['code'];key=(value['connection'],value['id']);to_tc=event['direction']=='sdk-to-tc'
   expected={1:(True,{0}),2:(False,{1}),5:(False,{0}),6:(True,{1}),9:(True,{0}),10:(False,{1}),11:(True,{0}),12:(False,{1}),13:(True,{0}),14:(False,{1}),101:(True,{0,2}),102:(False,{1}),103:(True,{0}),104:(False,{1})}
   if event['direction'] not in ['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Unknown actual wire direction')
   if code is None:
    if (event['direction'],value['kind']) not in [('sdk-to-tc',3),('tc-to-sdk',4)]:raise RuntimeError('Actual heartbeat envelope differs')
   elif code not in expected or to_tc!=expected[code][0] or value['kind'] not in expected[code][1]:raise RuntimeError('Actual RAFT recovery protocol direction/envelope differs')
   request_codes={2:1,10:9,12:11,14:13,102:101,104:103}
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
   if code in [2,9]:
    head=int.from_bytes(value['frame'][7:9],'big');reader=wire.Reader(value['frame'][head+2:])
    if code==2 and (reader.integer(1),reader.integer(1))!=(1,0):raise RuntimeError('Actual Begin response not successful')
    if reader.string()!=phase['xid']:raise RuntimeError('Original TM Begin/Rollback XID differs from actual business transaction')
    reader.string();reader.end()
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
  if not accepted_tm:raise RuntimeError('Actual full SDK TM identification missing')
  registrations.append(bool(accepted));stages.append(dict(stage=name,connections=state['connections'],callbacks=len(callbacks),registrations=len(accepted),tm_registrations=len(accepted_tm)))
 if len(set(resources))!=1:raise RuntimeError('Original resource identity missing or ambiguous')
 resource=resources[0]
 if any(candidate!=resource for candidate in callback_resources):raise RuntimeError('Callback changed the original registered resource')
 if resources!=[phase['resource_id']]:raise RuntimeError('Actual original RAFT resource differs from phase-one proof')
 if len({snapshots[name]['branch_status'] for name in ['timeout','restart','lost','second_restart']})!=1:raise RuntimeError('Retained RAFT branch status changed across lost acknowledgements')
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
