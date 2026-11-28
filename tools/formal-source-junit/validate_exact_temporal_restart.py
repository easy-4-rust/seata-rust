"""Independent UTC EXACT_TEMPORAL_RESTART JDBC/Undo/primary-key and original TC evidence checks."""
import argparse,copy,json
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_at_concurrent import frames,correlated,journal,string
from xa_managed_java_faults import guard
import exact_temporal_restart_java_oracle

def load(path):return json.loads(path.read_bytes())

def java_text(text):
    if text is None:return None
    return text


def case(folder,version,mode,direction,changes=None):
    changes=changes or {};prefix='-'.join([version,mode,direction]);root=folder/prefix;commit=direction=='commit';profile=5 if mode=='boolean' else 6
    proof=changes.get('proof',load(root/'observations/result.json'));xid=proof['xid'];branch=proof['branch_id']
    need(proof['status']=='AT_EXACT_TEMPORAL_RESTART_COLUMN_VERIFIED' and proof['direction']==direction and proof['profile']==profile and proof['journal']=='Completed' and proof['business_statements']==6 and branch>0,'EXACT_TEMPORAL_RESTART proof identity/scope differs')
    need(proof['observed_end_status'] in (['Committed'] if commit else ['Rollbacked','RollbackRetrying']),'EXACT_TEMPORAL_RESTART end outcome not authoritative')
    java=changes.get('java',load(root/'java-oracle/observations.json'));need(java['undo_timezone']=='UTC' and java['jvm_timezone']=='UTC' and java['database']=='8.4.13' and java['jackson']=='2.18.3' and java['driver'].startswith('mysql-connector-java-8.0.27 ') and java['profile']==('default' if mode=='boolean' else 'numeric_tinyint'),'EXACT_TEMPORAL_RESTART JDBC/UTC profile differs')
    need(len(java['cases'])==2,'EXACT_TEMPORAL_RESTART source tables missing')
    for table in ['undo_exact_temporal_restart','undo_exact_temporal_restart_keys']:
        original=load(root/('java-oracle/'+table+'.json'));native=changes.get(table,load(root/('observations/rust-java-'+table+'.json')));need(original==native,'EXACT_TEMPORAL_RESTART Rust format differs from original Java: '+table)
    before=changes.get('before',load(root/'observations/before-business.json'));after=changes.get('after',load(root/'observations/after-business.json'));before_keys=changes.get('before-keys',load(root/'observations/before-keys.json'));after_keys=changes.get('after-keys',load(root/'observations/after-keys.json'))
    need(len(before)==13 and [r[0] for r in before]==list(range(1,13))+[20001] and all(len(r)==8 for r in before),'EXACT_TEMPORAL_RESTART calendar/precision sample set differs')
    dates=['1000-01-01','1500-02-28','1582-10-04','1582-10-05','1582-10-10','1582-10-14','1582-10-15','1582-10-29','1969-12-31','1970-01-01','2000-02-29','9999-12-31',None]
    need([r[1] for r in before]==dates and before[0][2:]==['1000-01-01','1500-02-28','1582-10-05','1582-10-10','1582-10-14','9999-12-31'] and all(r[2:]==[None]*6 for r in before[1:]),'DATE complete physical calendar baseline differs')
    primary=[j for j in java['cases'] if j['table']=='undo_exact_temporal_restart'];need(len(primary)==1,'Missing original EXACT_TEMPORAL_RESTART table');primary=primary[0];need(primary['rows']==13 and primary['original_roundtrip'] and [c['native_type'] for c in primary['columns'][1:]]==['date']*7,'EXACT_TEMPORAL_RESTART original fractional precision differs')
    for actual,row in zip(primary['values'],before):
        need(actual[0]['exact_value']==str(row[0]) and len(actual)==8,'EXACT_TEMPORAL_RESTART row order differs')
        for field,text in zip(actual[1:],row[1:]):need(field['jdbc_type']==91 and field['runtime_class']==(None if text is None else 'java.lang.String') and field['exact_value']==java_text(text),'EXACT_TEMPORAL_RESTART value not original JDBC calendar')
    need(before_keys==[['-838:59:59.000000',6],['-00:00:00.000001',7],['838:59:59.000000',9]],'EXACT_TEMPORAL_RESTART original primary keys differ')
    expected=copy.deepcopy(before);expected_keys=copy.deepcopy(before_keys)
    if commit:
        expected=[r for r in expected if r[0]!=20001];expected[0][1]='1582-10-14';expected.append([10001,'1000-01-01',None,None,None,None,None,None]);expected.sort()
        expected_keys=[['-838:59:59.000000',6],['-25:01:02.000001',10],['-00:00:00.000001',8]]
    need(after==expected and after_keys==expected_keys,'EXACT_TEMPORAL_RESTART physical compensation differs')
    def fact(name):return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    hx=lambda v:'NULL' if v is None else v.encode().hex().upper()
    need(fact('independent-business')=='\n'.join('\t'.join([str(r[0]),*[hx(v) for v in r[1:]]]) for r in expected).encode(),'Independent EXACT_TEMPORAL_RESTART business differs')
    need(fact('independent-keys')=='\n'.join(hx(k)+'\t'+str(v) for k,v in expected_keys).encode(),'Independent EXACT_TEMPORAL_RESTART keys differ')
    need(fact('independent-tc')==b'0\t0\t0','EXACT_TEMPORAL_RESTART TC resources leaked')
    snapshot=changes.get('snapshot',load(root/'observations/before-end.json'));need(snapshot==dict(xid=xid,branch_id=branch,profile=profile,wrong_profile_rejected=True,counts=[profile,profile,0,1,6,1]),'EXACT_TEMPORAL_RESTART wrong configuration/Undo/SHA/locks not verified')
    negative=changes.get('negative',load(root/'observations/negative-checks.json'));need(negative==dict(invalid_date_mysql_code=1292,poisoned=True,zero_date_rejected_not_null=True,legacy_profile_rejected=True,outside_range_rejected=4),'EXACT_TEMPORAL_RESTART invalid/zero date or wrong profile accepted')
    rm=changes.get('rm',frames(root/'RM-frames.jsonl')[0]);tm=changes.get('tm',frames(root/'TM-frames.jsonl')[0]);need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'EXACT_TEMPORAL_RESTART frame altered')
    registers=[r for r in rm if r['kind']==11 and r['direction']=='c2s'];reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];begins=[r for r in tm if r['kind']==1 and r['direction']=='c2s'];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s'];need(len(registers)==len(reports)==len(begins)==len(ends)==1,'EXACT_TEMPORAL_RESTART operation replayed or omitted')
    begin=correlated(tm,begins[0],2)['body'];need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'EXACT_TEMPORAL_RESTART original Begin differs')
    body=registers[0]['body'];actual,pos=string(body,2);need(actual==xid and body[pos]==0,'EXACT_TEMPORAL_RESTART not original AT branch');resource,pos=string(body,pos+1);locks,pos=string(body,pos,4);app,pos=string(body,pos,4)
    expected_locks=['undo_exact_temporal_restart:1','undo_exact_temporal_restart:10001','undo_exact_temporal_restart:20001',*['undo_exact_temporal_restart_keys:'+java_text(k) for k in ['-00:00:00.000001','-25:01:02.000001','838:59:59.000000']]]
    need(pos==len(body) and app is None and sorted(locks.split(';'))==sorted(expected_locks),'EXACT_TEMPORAL_RESTART original primary key strings differ')
    receipt=correlated(rm,registers[0],12)['body'];need(receipt[2:4]==b'\x01\x00' and int.from_bytes(receipt[4:12],'big',signed=True)==branch,'EXACT_TEMPORAL_RESTART receipt differs')
    body=reports[0]['body'];actual,pos=string(body,2);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==2 and correlated(rm,reports[0],14)['body']==b'\x00\x0e\x01\x00','EXACT_TEMPORAL_RESTART report failed/changed')
    need(ends[0]['kind']==(7 if commit else 9) and string(ends[0]['body'],2)[0]==xid,'EXACT_TEMPORAL_RESTART end reversed');reply=correlated(tm,ends[0],8 if commit else 10)['body'];need(reply[2:]==b'\x01\x00'+bytes([{'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[proof['observed_end_status']]]),'EXACT_TEMPORAL_RESTART outcome not original response')
    callbacks=[r for r in rm if r['kind'] in [3,5] and r['direction']=='s2c'];need(1<len(callbacks)<=64,'Corrupt and restored callbacks absent');succeeded=[];failed=[]
    for row in callbacks:
        body=row['body'];actual,pos=string(body,2);need(actual==xid and row['kind']==(3 if commit else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==0,'Callback switched identity');r,pos=string(body,pos+9);a,pos=string(body,pos,4);need(r==resource and a is None and pos==len(body),'Callback resource/app changed')
        response=correlated(rm,row,4 if commit else 6);body=response['body']
        if body[2]==1:
            need(body[3]==0,'Successful callback exception changed');pos=4
        else:
            need(body[2]==0,'Callback result code changed');message,pos=string(body,3);need(message=='resource action not confirmed' and body[pos]==0,'Checksum callback error changed');pos+=1
        actual,pos=string(body,pos);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and len(body)==pos+9,'Callback branch identity changed')
        if body[-1]==(5 if commit else 8):need(body[2]==1,'Failed overall result claimed successful branch');succeeded.append(response)
        else:need(body[-1]==(6 if commit else 9),'Callback branch status changed');failed.append(response)
    need(failed and succeeded and max(r['index'] for r in failed)<min(r['index'] for r in succeeded),'Corrupt branch did not fail before successful original retry')
    tamper_case(root,xid,branch,profile,changes,failed,succeeded)
    journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),[dict(xid=xid,branch_id=branch,commit=commit)],resource)
    columns=fact('independent-state').decode().split('\t');need(len(columns)==9 and columns[:7]==[xid.encode().hex().upper(),str(branch),*(['1','0','1','1'] if commit else ['2','1','0','0']),'1'] and len(columns[7])==64 and columns[8]==str(profile),'EXACT_TEMPORAL_RESTART durable terminal/profile/witness differs')
    need(fact('independent-undo')==(b'' if commit else (xid.encode().hex().upper()+'\t'+str(branch)+'\t1').encode()),'EXACT_TEMPORAL_RESTART Undo/fence differs')
    need((('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in (root/'tc-stdout.log').read_bytes(),'Original TC EXACT_TEMPORAL_RESTART success missing')
    classes=(root/'oracle-class-load.log').read_bytes()
    for name in ['org.apache.seata.rm.datasource.sql.struct.TableRecords','org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser','org.apache.seata.rm.datasource.sql.struct.cache.ExactTemporalRestartOracle']:need(any((name+' source: file:/').encode() in line and b'/exact-temporal-oracle/classes/' in line for line in classes.splitlines()),'EXACT_TEMPORAL_RESTART original fresh Java class origin differs')
    command=load(root/'client-command.json');need(command['exit_code']==0 and command['argv'][1:]==['original_java_tc_exact_temporal_restart','--ignored','--exact','--nocapture'] and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes() and load(root/'process-cleanup.json')['stopped'],'EXACT_TEMPORAL_RESTART native case/cleanup incomplete')

    process_case(folder,version,mode,direction,changes)

def tamper_case(root,xid,branch,profile,changes,failed,succeeded):
    import hashlib
    out=root/'observations'
    original=changes.get('original-undo',(out/'original-undo.bin').read_bytes());changed=changes.get('changed-undo',(out/'changed-undo.bin').read_bytes())
    need(original!=changed and len(original)==len(changed),'Equal-length Undo edit absent')
    position=original.find(b'1582-10-14');need(position>=0 and changed==original[:position]+b'1582-10-15'+original[position+10:],'Undo edit was not the specified legal date')
    json.loads(original);json.loads(changed)
    before=changes.get('tamper',load(out/'tamper-prepared.json'));restored=changes.get('restored',load(out/'undo-restored.json'));rejected=changes.get('rejected',load(out/'recovery-checksum-rejected.json'));gate=changes.get('restore-gate',load(out/'allow-original-undo-restore.json'))
    for r in [before,restored,rejected,gate]:need(r['xid']==xid and r['branch_id']==branch,'Tamper proof switched branch')
    need(before['equal_length'] and before['legal_date_edit'] and before['original_sha256']==hashlib.sha256(original).hexdigest() and before['changed_sha256']==hashlib.sha256(changed).hexdigest(),'Tamper digest differs')
    need(rejected['profile']==profile and rejected['observation_rejected'] is True and rejected['recovery_rejected'] is True,'Fresh-process recovery accepted changed Undo')
    need(restored['original_bytes_restored'] is True and restored['unchanged_while_rejected'] is True and restored['terminal_while_rejected']==[0,0] and restored['sha256']==before['original_sha256'],'Changed branch applied or original bytes not restored')
    matches=[r for r in failed if r['index']==gate['failed_response_index']];need(len(matches)==1,'Restore gate has no actual failed callback');reply=matches[0]
    need(gate['failed_response_sha256']==hashlib.sha256(bytes.fromhex(reply['frame_hex'])).hexdigest() and gate['connection']==reply['connection'] and reply['monotonic_ns']<gate['monotonic_ns']<min(r['monotonic_ns'] for r in succeeded) and restored['authorization_frame_index']==reply['index'],'Original bytes restored without causal failed callback gate')
    worker=load(out/'worker-ready.json');need(gate['worker_pid']==rejected['pid']==worker['pid'],'Rejection or restore gate belongs to another process')
    events=changes.get('checksum-events',load(out/'worker-checksum-events.json'));checks=[r for r in events if r.get('target')=='seata_db_mysql::undo' and r.get('message')=='mysql_undo_checksum_rejected']
    need(len(checks)>=3 and all(r.get('xid')==xid and r.get('branch_id')==str(branch) and r.get('outcome')=='unknown' for r in checks),'Corrupt recovery/callback checksum events absent or unrelated')


def process_case(folder,version,mode,direction,changes=None):
    changes=changes or {};root=folder/'-'.join([version,mode,direction]);out=root/'observations'
    p=changes.get('process',load(out/'process-proof.json'));os=changes.get('os',load(root/'os-processes.json'));command=load(root/'client-command.json')
    xid=p['xid'];branch=p['branch_id'];profile=5 if mode=='boolean' else 6
    need(type(p['parent_pid']) is int and p['parent_pid']>0 and p['parent_pid']==command['pid']==os['parent_pid'],'Actual native supervisor differs')
    need(len({p['parent_pid'],p['producer_pid'],p['worker_pid']})==3 and p['producer_forced_termination'] is True and p['producer_exit_success'] is False and p['worker_exit_success'] is True and p['same_branch'] is True,'Fresh process or forced death absent')
    need(os['native_parent_exit_code']==0 and os['all_children_stopped'] is True and os['terminal']['producer']==p['producer_exit_code']!=0 and os['terminal']['worker']==0,'Actual OS process exit differs')
    need(set(os['children'])=={'producer','worker'},'Actual child observation incomplete')
    need(os['children']['worker']['producer_terminated_before_worker_observation'] is True,'Producer was not actually stopped before replacement observation')
    for role in ['producer','worker']:
        row=os['children'][role];actual=row['actual'];claim=row['claimed'];target='exact_temporal_restart_'+role
        need(actual['ProcessId']==claim['pid']==p[role+'_pid'] and actual['ParentProcessId']==claim['parent_pid']==p['parent_pid'],'Actual child parent/PID differs')
        need(claim['test']==target and target in actual['CommandLine'] and '--ignored' in actual['CommandLine'] and Path(actual['ExecutablePath']).resolve()==Path(command['path']).resolve() and row['executable_sha256']==command['sha256']==sha(Path(command['path'])),'Actual child executable differs')
    ready=changes.get('ready',load(out/'worker-ready.json'));finished=changes.get('finished',load(out/'worker-result.json'));produced=load(out/'producer-ready.json');before=load(out/'before-end.json')
    for item,pid in [(produced,p['producer_pid']),(ready,p['worker_pid']),(finished,p['worker_pid'])]:need(item['pid']==pid and item['xid']==xid==before['xid'] and item['branch_id']==branch==before['branch_id'] and item['profile']==profile,'Process switched persisted identity or profile')
    need(ready['attempt_loaded'] is True and ready['witness_observed'] is False and ready['corrupt_undo_rejected'] is True and ready['registered'] is True and finished['journal']=='Completed' and finished['fresh_process'] is True,'Replacement did not reconstruct persisted branch')
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
                for key,value in [('profile',1),('attempt_loaded',False),('witness_observed',True)]:p=load(out/'worker-ready.json');p[key]=value;variants.append(dict(ready=p))
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
    catalog(folder,digest);sources(folder);exact_temporal_restart_java_oracle.verify(folder);r=load(folder/'result.json');need(r['status']=='ORIGINAL_JAVA_TC_EXACT_TEMPORAL_RESTART_COLUMN_VERIFIED' and r['Java_pin']==guard.reg.PIN and r['Java_jar_sha256']==guard.reg.JAR_SHA and r['whole_plan_complete'] is False and r['actual_global_transactions']==len(r['Rust_versions'])*len(r['profiles'])*len(r['directions']),'EXACT_TEMPORAL_RESTART scope or pin differs')
    need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'],'EXACT_TEMPORAL_RESTART command/cleanup failed')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'EXACT_TEMPORAL_RESTART auth overlay replaced coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'EXACT_TEMPORAL_RESTART auth source drift')
    initial=load(folder/'inspect.stdout')[0];final=load(folder/'inspect-stopped.stdout')[0];need(initial['Id']==final['Id']==(folder/'create.stdout').read_text().strip() and initial['Image']==final['Image']==guard.IMAGE and initial['Mounts']==final['Mounts'] and not final['State']['Running'] and final['State']['ExitCode']==0,'EXACT_TEMPORAL_RESTART owned database identity/stop differs')
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
                for table in ['undo_exact_temporal_restart','undo_exact_temporal_restart_keys']:p=load(root/('observations/rust-java-'+table+'.json'));p['branchId']=1;variants.append({table:p})
                for target,kind in [('rm',11),('rm',13),('rm',3 if d=='commit' else 5),('tm',1),('tm',7 if d=='commit' else 9)]:rows=frames(root/('RM-frames.jsonl' if target=='rm' else 'TM-frames.jsonl'))[0];variants.append({target:[row for row in rows if row['kind']!=kind]})
                for variant in variants:
                    try:case(folder,v,m,d,variant)
                    except ValueError:count+=1
                    else:raise RuntimeError('EXACT_TEMPORAL_RESTART weakening accepted')
    return count

def tamper_weakenings(folder):
    r=load(folder/'result.json');count=0
    for version in r['Rust_versions']:
        for mode in r['profiles']:
            for direction in r['directions']:
                out=folder/'-'.join([version,mode,direction])/'observations';variants=[]
                variants.append({'changed-undo':(out/'original-undo.bin').read_bytes()})
                variants.append({'changed-undo':(out/'changed-undo.bin').read_bytes().replace(b'1582-10-15',b'1582-10-20',1)})
                p=load(out/'recovery-checksum-rejected.json');p['recovery_rejected']=False;variants.append(dict(rejected=p))
                p=load(out/'allow-original-undo-restore.json');p['failed_response_index']=-1;variants.append({'restore-gate':p})
                p=load(out/'allow-original-undo-restore.json');p['monotonic_ns']=2**63-1;variants.append({'restore-gate':p})
                p=load(out/'undo-restored.json');p['original_bytes_restored']=False;variants.append(dict(restored=p))
                variants.append({'checksum-events':[]})
                for variant in variants:
                    try:case(folder,version,mode,direction,variant)
                    except ValueError:count+=1
                    else:raise RuntimeError('Corruption/recovery proof weakening accepted')
    return count


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256);print('EXACT_TEMPORAL_RESTART original UTC/JDBC/Undo/TC verified; weakenings rejected:',weakenings(a.folder)+process_weakenings(a.folder)+tamper_weakenings(a.folder) if a.weakenings else 0)
