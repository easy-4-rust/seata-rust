"""Independent UTC TIMESTAMP JDBC/Undo/primary-key and original TC evidence checks."""
import argparse,copy,json
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_at_concurrent import frames,correlated,journal,string
from xa_managed_java_faults import guard
import timestamp_java_oracle

def load(path):return json.loads(path.read_bytes())

def java_text(text):
    if text is None:return None
    date,clock=text.split(' ');seconds,_,fraction=clock.partition('.')
    return date+' '+seconds+'.'+(fraction.rstrip('0') or '0')


def case(folder,version,mode,direction,changes=None):
    changes=changes or {};prefix='-'.join([version,mode,direction]);root=folder/prefix;commit=direction=='commit';profile=3 if mode=='boolean' else 4
    proof=changes.get('proof',load(root/'observations/result.json'));xid=proof['xid'];branch=proof['branch_id']
    need(proof['status']=='AT_TIMESTAMP_COLUMN_VERIFIED' and proof['direction']==direction and proof['profile']==profile and proof['journal']=='Completed' and proof['business_statements']==6 and branch>0,'TIMESTAMP proof identity/scope differs')
    need(proof['observed_end_status'] in (['Committed'] if commit else ['Rollbacked','RollbackRetrying']),'TIMESTAMP end outcome not authoritative')
    java=changes.get('java',load(root/'java-oracle/observations.json'));need(java['undo_timezone']=='UTC' and java['jvm_timezone']=='UTC' and java['database']=='8.4.13' and java['jackson']=='2.18.3' and java['driver'].startswith('mysql-connector-java-8.0.27 ') and java['profile']==('default' if mode=='boolean' else 'numeric_tinyint'),'TIMESTAMP JDBC/UTC profile differs')
    need(len(java['cases'])==2,'TIMESTAMP source tables missing')
    for table in ['undo_timestamp','undo_timestamp_keys']:
        original=load(root/('java-oracle/'+table+'.json'));native=changes.get(table,load(root/('observations/rust-java-'+table+'.json')));need(original==native,'TIMESTAMP Rust format differs from original Java: '+table)
    before=changes.get('before',load(root/'observations/before-business.json'));after=changes.get('after',load(root/'observations/after-business.json'));before_keys=changes.get('before-keys',load(root/'observations/before-keys.json'));after_keys=changes.get('after-keys',load(root/'observations/after-keys.json'))
    need(len(before)==9 and [r[0] for r in before]==[1,2,3,4,5,6,7,8,20001] and all(len(r)==8 for r in before),'TIMESTAMP calendar/precision sample set differs')
    need(before[0][1]=='1970-01-01 00:00:01.000001' and before[3][1]=='1999-12-31 23:59:59.999999' and before[7][1]=='2038-01-19 03:14:07.999999' and before[2][1:] == [None]*7,'TIMESTAMP boundaries or SQL NULL changed')
    primary=[j for j in java['cases'] if j['table']=='undo_timestamp'];need(len(primary)==1,'Missing original TIMESTAMP table');primary=primary[0];need(primary['rows']==9 and primary['original_roundtrip'] and [c['native_type'] for c in primary['columns'][1:]]==['timestamp(6)','timestamp','timestamp(1)','timestamp(2)','timestamp(3)','timestamp(4)','timestamp(5)'],'TIMESTAMP original fractional precision differs')
    for actual,row in zip(primary['values'],before):
        need(actual[0]['exact_value']==str(row[0]) and len(actual)==8,'TIMESTAMP row order differs')
        for field,text in zip(actual[1:],row[1:]):need(field['jdbc_type']==93 and field['runtime_class']==(None if text is None else 'java.sql.Timestamp') and field['exact_value']==java_text(text),'TIMESTAMP value not original JDBC calendar')
    need(before_keys==[['1970-01-01 00:00:01.000001',7],['2038-01-19 03:14:07.123456',9]],'TIMESTAMP original primary keys differ')
    expected=copy.deepcopy(before);expected_keys=copy.deepcopy(before_keys)
    if commit:
        expected=[r for r in expected if r[0]!=20001];expected[0][1]='2038-01-19 03:14:07.123456';expected.append([10001,'2000-02-29 12:34:56.000001',None,None,None,None,None,None]);expected.sort()
        expected_keys=[['1970-01-01 00:00:01.000001',8],['1970-01-01 00:00:01.000002',10]]
    need(after==expected and after_keys==expected_keys,'TIMESTAMP physical compensation differs')
    def fact(name):return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    hx=lambda v:'NULL' if v is None else v.encode().hex().upper()
    need(fact('independent-business')=='\n'.join('\t'.join([str(r[0]),*[hx(v) for v in r[1:]]]) for r in expected).encode(),'Independent TIMESTAMP business differs')
    need(fact('independent-keys')=='\n'.join(hx(k)+'\t'+str(v) for k,v in expected_keys).encode(),'Independent TIMESTAMP keys differ')
    need(fact('independent-tc')==b'0\t0\t0','TIMESTAMP TC resources leaked')
    snapshot=changes.get('snapshot',load(root/'observations/before-end.json'));need(snapshot==dict(xid=xid,branch_id=branch,profile=profile,wrong_profile_rejected=True,counts=[profile,profile,0,1,6,1]),'TIMESTAMP wrong configuration/Undo/SHA/locks not verified')
    negative=changes.get('negative',load(root/'observations/negative-checks.json'));need(negative==dict(invalid_date_mysql_code=1292,poisoned=True,zero_date_rejected_not_null=True,legacy_profile_rejected=True,outside_range_rejected=4),'TIMESTAMP invalid/zero date or wrong profile accepted')
    rm=changes.get('rm',frames(root/'RM-frames.jsonl')[0]);tm=changes.get('tm',frames(root/'TM-frames.jsonl')[0]);need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'TIMESTAMP frame altered')
    registers=[r for r in rm if r['kind']==11 and r['direction']=='c2s'];reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];begins=[r for r in tm if r['kind']==1 and r['direction']=='c2s'];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s'];need(len(registers)==len(reports)==len(begins)==len(ends)==1,'TIMESTAMP operation replayed or omitted')
    begin=correlated(tm,begins[0],2)['body'];need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'TIMESTAMP original Begin differs')
    body=registers[0]['body'];actual,pos=string(body,2);need(actual==xid and body[pos]==0,'TIMESTAMP not original AT branch');resource,pos=string(body,pos+1);locks,pos=string(body,pos,4);app,pos=string(body,pos,4)
    expected_locks=['undo_timestamp:1','undo_timestamp:10001','undo_timestamp:20001',*['undo_timestamp_keys:'+java_text(k) for k in ['1970-01-01 00:00:01.000001','1970-01-01 00:00:01.000002','2038-01-19 03:14:07.123456']]]
    need(pos==len(body) and app is None and sorted(locks.split(';'))==sorted(expected_locks),'TIMESTAMP original primary key strings differ')
    receipt=correlated(rm,registers[0],12)['body'];need(receipt[2:4]==b'\x01\x00' and int.from_bytes(receipt[4:12],'big',signed=True)==branch,'TIMESTAMP receipt differs')
    body=reports[0]['body'];actual,pos=string(body,2);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==2 and correlated(rm,reports[0],14)['body']==b'\x00\x0e\x01\x00','TIMESTAMP report failed/changed')
    need(ends[0]['kind']==(7 if commit else 9) and string(ends[0]['body'],2)[0]==xid,'TIMESTAMP end reversed');reply=correlated(tm,ends[0],8 if commit else 10)['body'];need(reply[2:]==b'\x01\x00'+bytes([{'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[proof['observed_end_status']]]),'TIMESTAMP outcome not original response')
    callbacks=[r for r in rm if r['kind'] in [3,5] and r['direction']=='s2c'];need(0<len(callbacks)<=64,'TIMESTAMP callback missing');succeeded=False
    for row in callbacks:
        body=row['body'];actual,pos=string(body,2);need(actual==xid and row['kind']==(3 if commit else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==0,'TIMESTAMP callback switched identity');r,pos=string(body,pos+9);a,pos=string(body,pos,4);need(r==resource and a is None and pos==len(body),'TIMESTAMP callback resource/app changed')
        response=correlated(rm,row,4 if commit else 6)['body'];actual,pos=string(response,4);need(response[2:4]==b'\x01\x00' and actual==xid and int.from_bytes(response[pos:pos+8],'big',signed=True)==branch and len(response)==pos+9 and response[-1] in ([5,6] if commit else [8,9]),'TIMESTAMP branch response differs');succeeded|=response[-1]==(5 if commit else 8)
    need(succeeded,'TIMESTAMP original branch never succeeded');journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),[dict(xid=xid,branch_id=branch,commit=commit)],resource)
    columns=fact('independent-state').decode().split('\t');need(len(columns)==9 and columns[:7]==[xid.encode().hex().upper(),str(branch),*(['1','0','1','1'] if commit else ['2','1','0','0']),'1'] and len(columns[7])==64 and columns[8]==str(profile),'TIMESTAMP durable terminal/profile/witness differs')
    need(fact('independent-undo')==(b'' if commit else (xid.encode().hex().upper()+'\t'+str(branch)+'\t1').encode()),'TIMESTAMP Undo/fence differs')
    need((('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in (root/'tc-stdout.log').read_bytes(),'Original TC TIMESTAMP success missing')
    classes=(root/'oracle-class-load.log').read_bytes()
    for name in ['org.apache.seata.rm.datasource.sql.struct.TableRecords','org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser','org.apache.seata.rm.datasource.sql.struct.cache.TimestampColumnOracle']:need(any((name+' source: file:/').encode() in line and b'/timestamp-oracle/classes/' in line for line in classes.splitlines()),'TIMESTAMP original fresh Java class origin differs')
    need(b'test result: ok. 1 passed;' in (folder/(prefix+'-session-guard.stdout')).read_bytes(),'Actual TIMESTAMP UTC/non-UTC session checks did not run')
    command=load(root/'client-command.json');need(command['exit_code']==0 and command['argv'][1:]==['original_java_tc_timestamp_restart','--ignored','--exact','--nocapture'] and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes() and load(root/'process-cleanup.json')['stopped'],'TIMESTAMP native case/cleanup incomplete')

    process_case(folder,version,mode,direction,changes)

def process_case(folder,version,mode,direction,changes=None):
    changes=changes or {};root=folder/'-'.join([version,mode,direction]);out=root/'observations'
    p=changes.get('process',load(out/'process-proof.json'));os=changes.get('os',load(root/'os-processes.json'));command=load(root/'client-command.json')
    xid=p['xid'];branch=p['branch_id'];profile=3 if mode=='boolean' else 4
    need(type(p['parent_pid']) is int and p['parent_pid']>0 and p['parent_pid']==command['pid']==os['parent_pid'],'Actual native supervisor differs')
    need(len({p['parent_pid'],p['producer_pid'],p['worker_pid']})==3 and p['producer_forced_termination'] is True and p['producer_exit_success'] is False and p['worker_exit_success'] is True and p['same_branch'] is True,'Fresh process or forced death absent')
    need(os['native_parent_exit_code']==0 and os['all_children_stopped'] is True and os['terminal']['producer']==p['producer_exit_code']!=0 and os['terminal']['worker']==0,'Actual OS process exit differs')
    need(set(os['children'])=={'producer','worker'},'Actual child observation incomplete')
    need(os['children']['worker']['producer_terminated_before_worker_observation'] is True,'Producer was not actually stopped before replacement observation')
    for role in ['producer','worker']:
        row=os['children'][role];actual=row['actual'];claim=row['claimed'];target='timestamp_restart_'+role
        need(actual['ProcessId']==claim['pid']==p[role+'_pid'] and actual['ParentProcessId']==claim['parent_pid']==p['parent_pid'],'Actual child parent/PID differs')
        need(claim['test']==target and target in actual['CommandLine'] and '--ignored' in actual['CommandLine'] and Path(actual['ExecutablePath']).resolve()==Path(command['path']).resolve() and row['executable_sha256']==command['sha256']==sha(Path(command['path'])),'Actual child executable differs')
    ready=changes.get('ready',load(out/'worker-ready.json'));finished=changes.get('finished',load(out/'worker-result.json'));produced=load(out/'producer-ready.json');before=load(out/'before-end.json')
    for item,pid in [(produced,p['producer_pid']),(ready,p['worker_pid']),(finished,p['worker_pid'])]:need(item['pid']==pid and item['xid']==xid==before['xid'] and item['branch_id']==branch==before['branch_id'] and item['profile']==profile,'Process switched persisted identity or profile')
    need(ready['attempt_loaded'] is True and ready['witness_observed'] is True and ready['registered'] is True and finished['journal']=='Completed' and finished['fresh_process'] is True,'Replacement did not reconstruct persisted branch')
    auth=changes.get('auth',load(root/'producer-exit-authorization.json'));need(set(auth)=={'phase','writer_pid','supervisor_pid','monotonic_ns'} and auth['phase']=='timestamp-producer-kill' and auth['writer_pid']==p['producer_pid'] and auth['supervisor_pid']==p['parent_pid'] and type(auth['monotonic_ns']) is int and auth['monotonic_ns']>0,'Wrong forced-exit authorization')
    rows,summary,events=frames(root/'RM-frames.jsonl');rows=changes.get('rm',rows);registrations=[r for r in rows if r['kind']==103 and r['direction']=='c2s'];branches=[r for r in rows if r['kind']==11 and r['direction']=='c2s'];callbacks=[r for r in rows if r['kind'] in [3,5] and r['direction']=='s2c']
    need(summary['connections']==2 and len(registrations)==2 and len(branches)==1 and callbacks,'Fresh RM registration/TC callback missing')
    first=branches[0]['connection'];second=callbacks[0]['connection'];need(first!=second and {r['connection'] for r in registrations}=={first,second} and all(r['connection']==second for r in callbacks),'Callback reused original process connection')
    for r in registrations:need(correlated(rows,r,104)['body'][2]==1,'Actual resource registration failed')
    resets=[r for r in events if r['event']=='expected_peer_reset'];need(len(resets)<=1,'Unexpected multiple controlled resets')
    for r in resets:need(r['connection']==first and r['direction']=='c2s' and r['error_type'] in ['ConnectionResetError','ConnectionAbortedError'] and r['authorization_sha256']==sha(root/'producer-exit-authorization.json') and 0<=r['monotonic_ns']-auth['monotonic_ns']<=10_000_000_000,'Reset escaped original authorized process/connection')
    kill_gate=load(out/'allow-producer-kill.json');worker_gate=load(out/'allow-worker-use.json')
    need(kill_gate['os_pid']==p['producer_pid'] and worker_gate['os_pid']==p['worker_pid'] and kill_gate['supervisor_pid']==worker_gate['supervisor_pid']==p['parent_pid'],'Wrong observed process gate')
    tm=frames(root/'TM-frames.jsonl')[0];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s'];registered=next(r for r in registrations if r['connection']==second)
    need(len(ends)==1 and auth['monotonic_ns']<=kill_gate['monotonic_ns']<registered['monotonic_ns']<correlated(rows,registered,104)['monotonic_ns']<worker_gate['monotonic_ns']<ends[0]['monotonic_ns'],'End/replacement/forced-exit sequence differs')
    need(b'test result: ok. 1 passed;' in (out/'worker.stdout').read_bytes(),'Replacement worker test not executed')


def process_weakenings(folder):
    result=load(folder/'result.json');count=0
    for version in result['Rust_versions']:
        for mode in result['profiles']:
            for direction in result['directions']:
                root=folder/'-'.join([version,mode,direction]);out=root/'observations';variants=[]
                for key,value in [('producer_forced_termination',False),('producer_exit_success',True),('worker_pid',load(out/'process-proof.json')['producer_pid']),('worker_exit_success',False)]:
                    p=load(out/'process-proof.json');p[key]=value;variants.append(dict(process=p))
                p=load(root/'os-processes.json');p['children']['worker']['actual']['ParentProcessId']=1;variants.append(dict(os=p))
                p=load(root/'os-processes.json');p['terminal']['producer']=0;variants.append(dict(os=p))
                p=load(root/'os-processes.json');p['children']['worker']['executable_sha256']='0'*64;variants.append(dict(os=p))
                for key,value in [('profile',1),('attempt_loaded',False),('witness_observed',False)]:p=load(out/'worker-ready.json');p[key]=value;variants.append(dict(ready=p))
                p=load(out/'worker-result.json');p['branch_id']=1;variants.append(dict(finished=p))
                p=load(root/'producer-exit-authorization.json');p['phase']='joint-kill';variants.append(dict(auth=p))
                p=frames(root/'RM-frames.jsonl')[0];first=next(r['connection'] for r in p if r['kind']==11)
                for r in p:
                    if r['kind'] in [3,5] and r['direction']=='s2c':r['connection']=first
                variants.append(dict(rm=p))
                for v in variants:
                    try:process_case(folder,version,mode,direction,v)
                    except ValueError:count+=1
                    else:raise RuntimeError('Restart process proof weakening accepted')
    return count

def verify(folder,digest):
    catalog(folder,digest);sources(folder);timestamp_java_oracle.verify(folder);r=load(folder/'result.json');need(r['status']=='ORIGINAL_JAVA_TC_TIMESTAMP_COLUMN_VERIFIED' and r['Java_pin']==guard.reg.PIN and r['Java_jar_sha256']==guard.reg.JAR_SHA and r['whole_plan_complete'] is False and r['actual_global_transactions']==len(r['Rust_versions'])*len(r['profiles'])*len(r['directions']),'TIMESTAMP scope or pin differs')
    need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'],'TIMESTAMP command/cleanup failed')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'TIMESTAMP auth overlay replaced coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'TIMESTAMP auth source drift')
    initial=load(folder/'inspect.stdout')[0];final=load(folder/'inspect-stopped.stdout')[0];need(initial['Id']==final['Id']==(folder/'create.stdout').read_text().strip() and initial['Image']==final['Image']==guard.IMAGE and initial['Mounts']==final['Mounts'] and not final['State']['Running'] and final['State']['ExitCode']==0,'TIMESTAMP owned database identity/stop differs')
    for v in r['Rust_versions']:
        for m in r['profiles']:
            for d in r['directions']:case(folder,v,m,d)

def weakenings(folder):
    r=load(folder/'result.json');count=0
    for v in r['Rust_versions']:
        for m in r['profiles']:
            for d in r['directions']:
                root=folder/'-'.join([v,m,d]);variants=[]
                for key in ['independent-business','independent-keys','independent-tc','independent-state','independent-undo','journal']:variants.append({key:b'changed'})
                for key,value in [('profile',1),('journal','Pending'),('business_statements',0)]:p=load(root/'observations/result.json');p[key]=value;variants.append(dict(proof=p))
                p=load(root/'observations/before-end.json');p['counts'][4]=5;variants.append(dict(snapshot=p))
                p=load(root/'observations/negative-checks.json');p['zero_date_rejected_not_null']=False;variants.append(dict(negative=p))
                p=load(root/'java-oracle/observations.json');p['undo_timezone']='Asia/Shanghai';variants.append(dict(java=p))
                p=load(root/'observations/before-business.json');p[2][1]='1970-01-01 00:00:00.000000';variants.append(dict(before=p))
                for table in ['undo_timestamp','undo_timestamp_keys']:p=load(root/('observations/rust-java-'+table+'.json'));p['branchId']=1;variants.append({table:p})
                for target,kind in [('rm',11),('rm',13),('rm',3 if d=='commit' else 5),('tm',1),('tm',7 if d=='commit' else 9)]:rows=frames(root/('RM-frames.jsonl' if target=='rm' else 'TM-frames.jsonl'))[0];variants.append({target:[row for row in rows if row['kind']!=kind]})
                for variant in variants:
                    try:case(folder,v,m,d,variant)
                    except ValueError:count+=1
                    else:raise RuntimeError('TIMESTAMP weakening accepted')
    return count

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256);print('TIMESTAMP original UTC/JDBC/Undo/TC verified; weakenings rejected:',weakenings(a.folder)+process_weakenings(a.folder) if a.weakenings else 0)
