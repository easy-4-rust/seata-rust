"""Actual shared-lock rejection, local rollback and distinct successful transaction evidence."""
from pathlib import Path
import hashlib,importlib.util,json
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
wire=module('positive_shared_lock_wire',HERE/'wire.py');replay=module('positive_shared_lock_query',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py')
def derive(directory):
 directory=Path(directory).resolve();case=json.loads((directory/'case.json').read_bytes());runtime=json.loads((directory/'runtime.json').read_bytes());java=json.loads((directory/'java/phase-one.json').read_bytes());go=json.loads((directory/'go/phase-one.json').read_bytes());blocked=json.loads((directory/'go/blocked.json').read_bytes());phase={'java':java,'go':go};action=case['go_end_action'];java_action='rollback' if action=='commit' else 'commit'
 if action not in ['commit','rollback'] or case['java_end_action']!=java_action or len({java['xid'],go['xid'],blocked['xid']})!=3:raise RuntimeError('Three distinct original transactions and explicit end actions required')
 if len({runtime['tc_pid'],runtime['java_pid'],runtime['go_pid']})!=3 or runtime['java_exit_code'] or runtime['go_exit_code']:raise RuntimeError('Actual original process identities differ')
 for name,p in phase.items():
  if p['process_id']!=runtime[name+'_pid'] or p['branch_id']<=0 or not bytes.fromhex(p['undo_hex']):raise RuntimeError('Actual SDK phase-one identity/Undo absent')
 if blocked['process_id']!=runtime['go_pid'] or blocked['balance']!=900 or blocked['undo_rows']!=0 or not blocked['error']:raise RuntimeError('Rejected Go SQL must leave no local effect or Undo')
 begin=json.loads((directory/'go/begun.json').read_bytes());duplicate=json.loads((directory/'go/duplicate-open.json').read_bytes())
 if begin['xid']!=blocked['xid'] or begin['process_id']!=runtime['go_pid'] or duplicate!={'rejected':True,'process_id':runtime['go_pid']}:raise RuntimeError('Actual Go original Begin/duplicate-Open control differs')
 requests_by_actor={};counts={};wire_counts={};registration={};callbacks_proof=[];reports_proof=[];rejected_proof=[]
 for actor in ['java','go']:
  events=json.loads((directory/actor/'proxy-events.json').read_bytes());state=json.loads((directory/actor/'proxy-result.json').read_bytes());pid=json.loads((directory/actor/'process.json').read_bytes())['pid']
  if state['errors'] or state['field_translation'] or state['fault_injection'] or pid!=runtime[actor+'_pid'] or [r['sequence'] for r in events]!=list(range(1,len(events)+1)):raise RuntimeError('Original transparent recorder/process differs')
  requests={};seen=set();rm=[];accepted=[];callbacks={};reply_count=0;begins=[];ends=[];reports=[];acknowledged=[];end_acks={};identifications=0;version=2 if actor=='java' else 1
  for event in events:
   if event['event']=='connected':continue
   if event['event']!='wire':raise RuntimeError('Unexpected control event in transparent recorder')
   value=wire.decode(event);code=value['code'];key=(value['connection'],value['id']);to_tc=event['direction']=='sdk-to-tc'
   if value['version']!=version:raise RuntimeError('Original Java V2/Go V1 wire version changed')
   if code is None:
    if (to_tc,value['kind']) not in [(True,3),(False,4)]:raise RuntimeError('Actual heartbeat direction/envelope differs')
    continue
   reqs={1,7,9,11,13,101,103};resp={2:1,8:7,10:9,12:11,14:13,102:101,104:103}
   if code in reqs:
    if not to_tc or value['kind'] not in [0,2] or (code,*key) in requests:raise RuntimeError('Original request identity/direction/envelope differs')
    requests[(code,*key)]=value
   elif code in resp:
    target=(resp[code],*key)
    if to_tc or value['kind']!=1 or target not in requests or target in seen:raise RuntimeError('Unmatched/duplicate original response')
    seen.add(target)
   elif code in [3,5]:
    if to_tc or value['kind'] not in [0,2] or key in callbacks:raise RuntimeError('Original phase-two request envelope/identity differs')
    callbacks[key]=value
   elif code in [4,6]:
    if not to_tc or value['kind']!=1 or key not in callbacks:raise RuntimeError('Unmatched actual phase-two reply')
    original=callbacks[key];expected=4 if original['code']==3 else 6;status=5 if expected==4 else 8;p=phase[actor]
    if (code,value['result'],value['error'],value['xid'],value['branch_id'],value['status'])!=(expected,1,0,p['xid'],p['branch_id'],status):raise RuntimeError('Original successful RM callback response differs')
    reply_count+=1
   else:raise RuntimeError('Unexpected actor message family')
   if code in [102,104]:
    if not value['identified'] or value.get('result',1)!=1 or not value['server_version']:raise RuntimeError('Original TM/RM identification rejected')
    if code==102:identifications+=1
    else:rm.append(dict(resource=requests[(103,*key)]['resource'],connection=value['connection'],sequence=value['sequence']))
   if actor=='go' and code==1 and len(begins)==1 and blocked['xid'] not in end_acks:raise RuntimeError('New Go Begin precedes acknowledged termination of rejected transaction')
   if code==2:
    request=requests[(1,*key)];expected_name='shared-lock-java-held' if actor=='java' else ('shared-lock-go-rejected' if len(begins)==0 else 'shared-lock-go-new-'+action);expected_xid=java['xid'] if actor=='java' else (blocked['xid'] if len(begins)==0 else go['xid'])
    if request['name']!=expected_name or (value['result'],value['error'],value['xid'])!=(1,0,expected_xid):raise RuntimeError('Original Begin name/XID differs from actual SQL transaction')
    begins.append(expected_xid)
   if code in [7,9]:
    expected_xid=java['xid'] if actor=='java' else (blocked['xid'] if len(ends)==0 else go['xid']);expected_action=java_action if actor=='java' else ('rollback' if len(ends)==0 else action)
    if value['xid']!=expected_xid or code!=(7 if expected_action=='commit' else 9):raise RuntimeError('Original end targets wrong transaction or action')
    ends.append(expected_action)
   if code in [8,10]:
    if value['result']!=1 or value['error']!=0 or value['status'] not in ([8,9] if code==8 else [10,11]):raise RuntimeError('Original global end not accepted')
    end_acks[requests[(7 if code==8 else 9,*key)]['xid']]=value['sequence']
   if code==12:
    request=requests[(11,*key)];is_rejected=actor=='go' and request['xid']==blocked['xid']
    if request['branch_type']!=0 or request['resource']!=case['shared_resource'] or request['lock_key']!=case['java_lock_key']:raise RuntimeError('Actual shared AT resource/key must equal original Java bytes')
    if is_rejected:
     if (value['result'],value['error'],value['branch_id'])!=(0,2,0):raise RuntimeError('Conflicting AT registration must be explicit original LockKeyConflict rejection')
     rejected_proof.append(dict(xid=request['xid'],error_code=value['error'],branch_id=value['branch_id'],resource=request['resource'],lock_key=request['lock_key']))
    else:
     p=phase[actor]
     if (request['xid'],value['result'],value['error'],value['branch_id'])!=(p['xid'],1,0,p['branch_id']):raise RuntimeError('Original successful AT branch differs')
     accepted.append(request);registration[actor]=request
   if code==13:
    if actor!='go' or (value['xid'],value['branch_id'],value['status'],value['branch_type'])!=(go['xid'],go['branch_id'],2,0):raise RuntimeError('Original accepted phase-one report targets wrong branch')
    reports.append(value)
   if code==14:
    if (value['result'],value['error'])!=(1,0):raise RuntimeError('Original phase-one report rejected')
    acknowledged.append(dict(request=requests[(13,*key)],sequence=value['sequence']))
  if requests.keys()!=seen or identifications!=1 or len(accepted)!=1 or len(callbacks)!=1 or reply_count!=1:raise RuntimeError('Incomplete actual TM/resource/branch/callback chain')
  if begins!=([java['xid']] if actor=='java' else [blocked['xid'],go['xid']]) or ends!=([java_action] if actor=='java' else ['rollback',action]):raise RuntimeError('Original transaction lifecycle count differs')
  callback=next(iter(callbacks.values()));p=phase[actor];expected_code=3 if (java_action if actor=='java' else action)=='commit' else 5
  if (callback['code'],callback['xid'],callback['branch_id'],callback['resource'],callback['branch_type'])!=(expected_code,p['xid'],p['branch_id'],case['shared_resource'],0) or not any(row['resource']==case['shared_resource'] and row['connection']==callback['connection'] and row['sequence']<callback['sequence'] for row in rm):raise RuntimeError('Original phase-two dispatch lacks actual authenticated RM resource')
  if actor=='go':
   if len(reports)!=1 or len(acknowledged)!=1 or not reports[0]['sequence']<acknowledged[0]['sequence']<callback['sequence']:raise RuntimeError('One acknowledged original phase-one report before actual callback required')
   authoritative=reports[0]['application_data'];reports_proof.append(dict(xid=go['xid'],branch_id=go['branch_id'],status=2))
  else:authoritative=registration[actor]['application_data']
  if callback['application_data']!=authoritative:raise RuntimeError('Actual callback application data differs from acknowledged branch state')
  counts[actor]=dict(begins=len(begins),commits=ends.count('commit'),rollbacks=ends.count('rollback'));wire_counts[actor]=sum(row['event']=='wire' for row in events);callbacks_proof.append(dict(actor=actor,xid=p['xid'],branch_id=p['branch_id'],type=expected_code));requests_by_actor[actor]=len(requests)
 if len(rejected_proof)!=1:raise RuntimeError('One actual rejected conflict branch required')
 def sql_snapshot(name,want,p):
  rows=[line.split('\t') for line in (directory/(name+'.tsv')).read_text().splitlines()]
  if rows[0]!=['1',str(want)] or len(rows)!=2 or len(rows[1])!=4 or rows[1][:3]!=[p['xid'],str(p['branch_id']),'0'] or bytes.fromhex(rows[1][3])!=bytes.fromhex(p['undo_hex']):raise RuntimeError('Independent SQL business/Undo differs: '+name)
 sql_snapshot('java-held',900,java);sql_snapshot('blocked',900,java);sql_snapshot('go-held',900 if action=='commit' else 800,go)
 if (directory/'initial.tsv').read_bytes()!=b'1\t1000\n0\n' or (directory/'java-ended.tsv').read_bytes()!=('1\t'+str(1000 if action=='commit' else 900)+'\n0\n').encode() or (directory/'final.tsv').read_bytes()!=b'1\t900\n0\n':raise RuntimeError('Independent initial/released/final SQL differs')
 for name,p,empty in [('java-held',java,False),('blocked-java',java,False),('blocked-go',blocked,True),('java-ended',java,True),('go-held',go,False),('final-go',go,True)]:
  value=json.loads((directory/('console-'+name+'.json')).read_bytes())
  if str(value.get('code'))!='200' or value.get('total')!=(0 if empty else 1) or len(value.get('data',[]))!=(0 if empty else 1):raise RuntimeError('Actual console lifecycle differs')
  if not empty:
   session=value['data'][0];branches=session.get('branchSessionVOs') or []
   if session['xid']!=p['xid'] or session['status']!=1 or len(branches)!=1 or int(branches[0]['branchId'])!=p['branch_id'] or branches[0]['resourceId']!=case['shared_resource'] or branches[0]['branchType']!='AT':raise RuntimeError('Actual console original successful branch differs')
 for name,target,status,lockable in [('java-held',blocked['xid'],1,False),('go-held',go['xid'],1,False),('final',go['xid'],15,True)]:
  query=json.loads((directory/('query-'+name+'.json')).read_bytes());frames=query['frames'];requests=[bytes.fromhex(r['frame_hex']) for r in frames if r['direction']=='observer-to-tc'];responses=[bytes.fromhex(r['frame_hex']) for r in frames if r['direction']=='tc-to-observer']
  if len(frames)!=6 or any(hashlib.sha256(bytes.fromhex(row['frame_hex'])).hexdigest()!=row['sha256'] for row in frames) or [int.from_bytes(frame[16:18],'big') for frame in requests]!=[101,15,21]:raise RuntimeError('Actual read-only lock-query frames differ')
  stream=replay.ReplayStream(b''.join(responses));actual=replay.observer.query(stream,target,case['shared_resource'],case['java_lock_key'],status,lockable)
  if stream.data or bytes(stream.sent)!=b''.join(requests) or actual!=query or query['observer_xid'] in [java['xid'],blocked['xid'],go['xid']]:raise RuntimeError('Actual lock query does not reproduce original bytes/independent identity')
 origins=class_origins(directory)
 return dict(conflict_rejection_certified=True,go_end_action=action,java_end_action=java_action,java_xid=java['xid'],rejected_go_xid=blocked['xid'],new_go_xid=go['xid'],shared_resource=case['shared_resource'],shared_lock_key=case['java_lock_key'],rejected_original_branch=rejected_proof[0],original_callbacks=callbacks_proof,original_reports=reports_proof,tm_counts=counts,wire_frame_counts=wire_counts,native_queries=3,native_frames=18,actual_class_origins=origins,final_balance=900,final_undo_rows=0,duplicate_open_rejected=True,old_resource_migration_certified=False,undo_interchange_certified=False)

def class_origins(directory):
 build=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3');jar=ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar';parent=ROOT/'docs/phase-0/evidence/local-java-preface-candidate/run_20261112_221159_a16eafb1';driver=Path('E:/Java/.m2/repository/com/mysql/mysql-connector-j/8.0.33/mysql-connector-j-8.0.33.jar');origins=[]
 expected_java={'SharedLockJavaActor':directory/'compiled','org.apache.seata.rm.datasource.DataSourceProxy':build/'rm-datasource/target/classes','org.apache.seata.rm.datasource.ConnectionProxy':build/'rm-datasource/target/classes','org.apache.seata.rm.datasource.initializer.db.MysqlResourceIdInitializer':build/'rm-datasource/target/classes','org.apache.seata.tm.TMClient':build/'tm/target/classes','org.apache.seata.tm.api.DefaultGlobalTransaction':build/'tm/target/classes','org.apache.seata.rm.RMClient':build/'rm/target/classes','com.mysql.cj.jdbc.MysqlDataSource':driver}
 original=['org.apache.seata.server.storage.file.lock.FileLockManager','org.apache.seata.server.transaction.at.ATCore','org.apache.seata.server.session.SessionHolder'];overlay=['org.apache.seata.core.rpc.netty.ProtocolDetectHandler','org.apache.seata.core.protocol.detector.IncrementalProtocolDetector','org.apache.seata.core.protocol.detector.SeataDetector','org.apache.seata.core.protocol.detector.HttpDetector','org.apache.seata.core.protocol.detector.Http2Detector']
 for actor,expected in [('java',expected_java),('tc',{name:jar for name in original}|{name:parent/'overlay' for name in overlay})]:
  text=(directory/actor/('sdk-stdout.log' if actor=='java' else 'tc-stdout.log')).read_text(encoding='utf-8',errors='replace')
  for name,source in expected.items():
   lines=[line for line in text.splitlines() if ' '+name+' source: ' in line];location='jar:nested:/'+jar.as_posix()+'/!BOOT-INF/classes/!/' if actor=='tc' and name in original else 'file:/'+source.as_posix()+('/' if source.suffix!='.jar' else '')
   if len(lines)!=1 or not lines[0].endswith(' source: '+location):raise RuntimeError('Actual Java/TC class origin differs: '+name)
   origins.append(dict(actor=actor,type=name,source=location,line=lines[0]))
 actual=[dict(path=p.relative_to(directory/'compiled').as_posix(),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted((directory/'compiled').rglob('*.class'))]
 if actual!=json.loads((directory/'compiled-manifest.json').read_bytes()) or len(actual)!=1:raise RuntimeError('Compiled Java actor changed')
 return origins
