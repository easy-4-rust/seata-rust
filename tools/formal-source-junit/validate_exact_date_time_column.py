"""Independent UTC EXACT_DATE_TIME JDBC/Undo/primary-key and original TC evidence checks."""
import argparse,copy,json
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_at_concurrent import frames,correlated,journal,string
from xa_managed_java_faults import guard
import exact_date_time_java_oracle

def load(path):return json.loads(path.read_bytes())

def java_text(text):
    if text is None:return None
    return text


def case(folder,version,mode,direction,changes=None):
    changes=changes or {};prefix='-'.join([version,mode,direction]);root=folder/prefix;commit=direction=='commit';profile=5 if mode=='boolean' else 6
    proof=changes.get('proof',load(root/'observations/result.json'));xid=proof['xid'];branch=proof['branch_id']
    need(proof['status']=='AT_EXACT_DATE_TIME_COLUMN_VERIFIED' and proof['direction']==direction and proof['profile']==profile and proof['journal']=='Completed' and proof['business_statements']==6 and branch>0,'EXACT_DATE_TIME proof identity/scope differs')
    need(proof['observed_end_status'] in (['Committed'] if commit else ['Rollbacked','RollbackRetrying']),'EXACT_DATE_TIME end outcome not authoritative')
    java=changes.get('java',load(root/'java-oracle/observations.json'));need(java['undo_timezone']=='UTC' and java['jvm_timezone']=='UTC' and java['database']=='8.4.13' and java['jackson']=='2.18.3' and java['driver'].startswith('mysql-connector-java-8.0.27 ') and java['profile']==('default' if mode=='boolean' else 'numeric_tinyint'),'EXACT_DATE_TIME JDBC/UTC profile differs')
    need(len(java['cases'])==2,'EXACT_DATE_TIME source tables missing')
    for table in ['undo_exact_date_time','undo_exact_date_time_keys']:
        original=load(root/('java-oracle/'+table+'.json'));native=changes.get(table,load(root/('observations/rust-java-'+table+'.json')));need(original==native,'EXACT_DATE_TIME Rust format differs from original Java: '+table)
    before=changes.get('before',load(root/'observations/before-business.json'));after=changes.get('after',load(root/'observations/after-business.json'));before_keys=changes.get('before-keys',load(root/'observations/before-keys.json'));after_keys=changes.get('after-keys',load(root/'observations/after-keys.json'))
    need(len(before)==9 and [r[0] for r in before]==[1,2,3,4,5,6,7,8,20001] and all(len(r)==8 for r in before),'EXACT_DATE_TIME calendar/precision sample set differs')
    need(before[0][1]=='-00:00:00.000001' and before[1][1]=='-838:59:59.000000' and before[3][1]=='838:59:59.000000' and before[7][1]=='23:59:59.999999' and before[2][1:] == [None]*7,'EXACT_DATE_TIME boundaries or SQL NULL changed')
    primary=[j for j in java['cases'] if j['table']=='undo_exact_date_time'];need(len(primary)==1,'Missing original EXACT_DATE_TIME table');primary=primary[0];need(primary['rows']==9 and primary['original_roundtrip'] and [c['native_type'] for c in primary['columns'][1:]]==['time(6)','time','time(1)','time(2)','time(3)','time(4)','time(5)'],'EXACT_DATE_TIME original fractional precision differs')
    for actual,row in zip(primary['values'],before):
        need(actual[0]['exact_value']==str(row[0]) and len(actual)==8,'EXACT_DATE_TIME row order differs')
        for field,text in zip(actual[1:],row[1:]):need(field['jdbc_type']==92 and field['runtime_class']==(None if text is None else 'java.lang.String') and field['exact_value']==java_text(text),'EXACT_DATE_TIME value not original JDBC calendar')
    need(before_keys==[['1582-10-05',7],['9999-12-31',9]],'EXACT_DATE_TIME original primary keys differ')
    expected=copy.deepcopy(before);expected_keys=copy.deepcopy(before_keys)
    if commit:
        expected=[r for r in expected if r[0]!=20001];expected[0][1]='-838:59:58.123456';expected.append([10001,'-25:01:02.000001',None,None,None,None,None,None]);expected.sort()
        expected_keys=[['1582-10-05',8],['1582-10-14',10]]
    need(after==expected and after_keys==expected_keys,'EXACT_DATE_TIME physical compensation differs')
    def fact(name):return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    hx=lambda v:'NULL' if v is None else v.encode().hex().upper()
    need(fact('independent-business')=='\n'.join('\t'.join([str(r[0]),*[hx(v) for v in r[1:]]]) for r in expected).encode(),'Independent EXACT_DATE_TIME business differs')
    need(fact('independent-keys')=='\n'.join(hx(k)+'\t'+str(v) for k,v in expected_keys).encode(),'Independent EXACT_DATE_TIME keys differ')
    need(fact('independent-tc')==b'0\t0\t0','EXACT_DATE_TIME TC resources leaked')
    snapshot=changes.get('snapshot',load(root/'observations/before-end.json'));need(snapshot==dict(xid=xid,branch_id=branch,profile=profile,wrong_profile_rejected=True,counts=[profile,profile,0,1,6,1]),'EXACT_DATE_TIME wrong configuration/Undo/SHA/locks not verified')
    negative=changes.get('negative',load(root/'observations/negative-checks.json'));need(negative==dict(time_overflow_mysql_code=1292,poisoned=True,zero_date_rejected_not_null=True,old_profile_rejected=True,outside_range_rejected=4),'EXACT_DATE_TIME invalid/zero date or wrong profile accepted')
    rm=changes.get('rm',frames(root/'RM-frames.jsonl')[0]);tm=changes.get('tm',frames(root/'TM-frames.jsonl')[0]);need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'EXACT_DATE_TIME frame altered')
    registers=[r for r in rm if r['kind']==11 and r['direction']=='c2s'];reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];begins=[r for r in tm if r['kind']==1 and r['direction']=='c2s'];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s'];need(len(registers)==len(reports)==len(begins)==len(ends)==1,'EXACT_DATE_TIME operation replayed or omitted')
    begin=correlated(tm,begins[0],2)['body'];need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'EXACT_DATE_TIME original Begin differs')
    body=registers[0]['body'];actual,pos=string(body,2);need(actual==xid and body[pos]==0,'EXACT_DATE_TIME not original AT branch');resource,pos=string(body,pos+1);locks,pos=string(body,pos,4);app,pos=string(body,pos,4)
    expected_locks=['undo_exact_date_time:1','undo_exact_date_time:10001','undo_exact_date_time:20001',*['undo_exact_date_time_keys:'+java_text(k) for k in ['1582-10-05','1582-10-14','9999-12-31']]]
    need(pos==len(body) and app is None and sorted(locks.split(';'))==sorted(expected_locks),'EXACT_DATE_TIME original primary key strings differ')
    receipt=correlated(rm,registers[0],12)['body'];need(receipt[2:4]==b'\x01\x00' and int.from_bytes(receipt[4:12],'big',signed=True)==branch,'EXACT_DATE_TIME receipt differs')
    body=reports[0]['body'];actual,pos=string(body,2);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==2 and correlated(rm,reports[0],14)['body']==b'\x00\x0e\x01\x00','EXACT_DATE_TIME report failed/changed')
    need(ends[0]['kind']==(7 if commit else 9) and string(ends[0]['body'],2)[0]==xid,'EXACT_DATE_TIME end reversed');reply=correlated(tm,ends[0],8 if commit else 10)['body'];need(reply[2:]==b'\x01\x00'+bytes([{'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[proof['observed_end_status']]]),'EXACT_DATE_TIME outcome not original response')
    callbacks=[r for r in rm if r['kind'] in [3,5] and r['direction']=='s2c'];need(0<len(callbacks)<=64,'EXACT_DATE_TIME callback missing');succeeded=False
    for row in callbacks:
        body=row['body'];actual,pos=string(body,2);need(actual==xid and row['kind']==(3 if commit else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==0,'EXACT_DATE_TIME callback switched identity');r,pos=string(body,pos+9);a,pos=string(body,pos,4);need(r==resource and a is None and pos==len(body),'EXACT_DATE_TIME callback resource/app changed')
        response=correlated(rm,row,4 if commit else 6)['body'];actual,pos=string(response,4);need(response[2:4]==b'\x01\x00' and actual==xid and int.from_bytes(response[pos:pos+8],'big',signed=True)==branch and len(response)==pos+9 and response[-1] in ([5,6] if commit else [8,9]),'EXACT_DATE_TIME branch response differs');succeeded|=response[-1]==(5 if commit else 8)
    need(succeeded,'EXACT_DATE_TIME original branch never succeeded');journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),[dict(xid=xid,branch_id=branch,commit=commit)],resource)
    columns=fact('independent-state').decode().split('\t');need(len(columns)==9 and columns[:7]==[xid.encode().hex().upper(),str(branch),*(['1','0','1','1'] if commit else ['2','1','0','0']),'1'] and len(columns[7])==64 and columns[8]==str(profile),'EXACT_DATE_TIME durable terminal/profile/witness differs')
    need(fact('independent-undo')==(b'' if commit else (xid.encode().hex().upper()+'\t'+str(branch)+'\t1').encode()),'EXACT_DATE_TIME Undo/fence differs')
    need((('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in (root/'tc-stdout.log').read_bytes(),'Original TC EXACT_DATE_TIME success missing')
    classes=(root/'oracle-class-load.log').read_bytes()
    for name in ['org.apache.seata.rm.datasource.sql.struct.TableRecords','org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser','org.apache.seata.rm.datasource.sql.struct.cache.ExactDateTimeColumnOracle']:need(any((name+' source: file:/').encode() in line and b'/exact-temporal-oracle/classes/' in line for line in classes.splitlines()),'EXACT_DATE_TIME original fresh Java class origin differs')
    command=load(root/'client-command.json');need(command['exit_code']==0 and command['argv'][1:]==['original_java_tc_exact_date_time_column','--ignored','--exact','--nocapture'] and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes() and load(root/'process-cleanup.json')['stopped'],'EXACT_DATE_TIME native case/cleanup incomplete')

def verify(folder,digest):
    catalog(folder,digest);sources(folder);exact_date_time_java_oracle.verify(folder);r=load(folder/'result.json');need(r['status']=='ORIGINAL_JAVA_TC_EXACT_DATE_TIME_COLUMN_VERIFIED' and r['Java_pin']==guard.reg.PIN and r['Java_jar_sha256']==guard.reg.JAR_SHA and r['whole_plan_complete'] is False and r['actual_global_transactions']==len(r['Rust_versions'])*len(r['profiles'])*len(r['directions']),'EXACT_DATE_TIME scope or pin differs')
    need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'],'EXACT_DATE_TIME command/cleanup failed')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'EXACT_DATE_TIME auth overlay replaced coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'EXACT_DATE_TIME auth source drift')
    initial=load(folder/'inspect.stdout')[0];final=load(folder/'inspect-stopped.stdout')[0];need(initial['Id']==final['Id']==(folder/'create.stdout').read_text().strip() and initial['Image']==final['Image']==guard.IMAGE and initial['Mounts']==final['Mounts'] and not final['State']['Running'] and final['State']['ExitCode']==0,'EXACT_DATE_TIME owned database identity/stop differs')
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
                for table in ['undo_exact_date_time','undo_exact_date_time_keys']:p=load(root/('observations/rust-java-'+table+'.json'));p['branchId']=1;variants.append({table:p})
                for target,kind in [('rm',11),('rm',13),('rm',3 if d=='commit' else 5),('tm',1),('tm',7 if d=='commit' else 9)]:rows=frames(root/('RM-frames.jsonl' if target=='rm' else 'TM-frames.jsonl'))[0];variants.append({target:[row for row in rows if row['kind']!=kind]})
                for variant in variants:
                    try:case(folder,v,m,d,variant)
                    except ValueError:count+=1
                    else:raise RuntimeError('EXACT_DATE_TIME weakening accepted')
    return count

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256);print('EXACT_DATE_TIME original UTC/JDBC/Undo/TC verified; weakenings rejected:',weakenings(a.folder) if a.weakenings else 0)
