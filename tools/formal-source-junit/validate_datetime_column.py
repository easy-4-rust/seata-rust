"""Independent UTC DATETIME JDBC/Undo/primary-key and original TC evidence checks."""
import argparse,copy,json
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_at_concurrent import frames,correlated,journal,string
from xa_managed_java_faults import guard
import datetime_java_oracle

def load(path):return json.loads(path.read_bytes())

def java_text(text):
    if text is None:return None
    date,clock=text.split(' ');seconds,_,fraction=clock.partition('.');nano=int(fraction.ljust(9,'0')) if fraction else 0
    result=date+'T'+seconds[:5]
    if seconds[6:]!='00' or nano:result+=':'+seconds[6:]
    if nano:result+='.'+(f'{nano//1_000_000:03}' if nano%1_000_000==0 else f'{nano//1000:06}')
    return result

def case(folder,version,mode,direction,changes=None):
    changes=changes or {};prefix='-'.join([version,mode,direction]);root=folder/prefix;commit=direction=='commit';profile=3 if mode=='boolean' else 4
    proof=changes.get('proof',load(root/'observations/result.json'));xid=proof['xid'];branch=proof['branch_id']
    need(proof['status']=='AT_DATETIME_COLUMN_VERIFIED' and proof['direction']==direction and proof['profile']==profile and proof['journal']=='Completed' and proof['business_statements']==6 and branch>0,'DATETIME proof identity/scope differs')
    need(proof['observed_end_status'] in (['Committed'] if commit else ['Rollbacked','RollbackRetrying']),'DATETIME end outcome not authoritative')
    java=changes.get('java',load(root/'java-oracle/observations.json'));need(java['undo_timezone']=='UTC' and java['database']=='8.4.13' and java['jackson']=='2.18.3' and java['driver'].startswith('mysql-connector-java-8.0.27 ') and java['profile']==('default' if mode=='boolean' else 'numeric_tinyint'),'DATETIME JDBC/UTC profile differs')
    need(len(java['cases'])==2,'DATETIME source tables missing')
    for table in ['undo_datetime','undo_datetime_keys']:
        original=load(root/('java-oracle/'+table+'.json'));native=changes.get(table,load(root/('observations/rust-java-'+table+'.json')));need(original==native,'DATETIME Rust format differs from original Java: '+table)
    before=changes.get('before',load(root/'observations/before-business.json'));after=changes.get('after',load(root/'observations/after-business.json'));before_keys=changes.get('before-keys',load(root/'observations/before-keys.json'));after_keys=changes.get('after-keys',load(root/'observations/after-keys.json'))
    need(len(before)==9 and [r[0] for r in before]==[1,2,3,4,5,6,7,8,20001] and all(len(r)==8 for r in before),'DATETIME calendar/precision sample set differs')
    need(before[0][1]=='1000-01-01 00:00:00.000001' and before[3][1]=='1969-12-31 23:59:59.999999' and before[7][1]=='9999-12-31 23:59:59.999999' and before[2][1:] == [None]*7,'DATETIME boundaries or SQL NULL changed')
    primary=[j for j in java['cases'] if j['table']=='undo_datetime'];need(len(primary)==1,'Missing original DATETIME table');primary=primary[0];need(primary['rows']==9 and primary['original_roundtrip'] and [c['native_type'] for c in primary['columns'][1:]]==['datetime(6)','datetime','datetime(1)','datetime(2)','datetime(3)','datetime(4)','datetime(5)'],'DATETIME original fractional precision differs')
    for actual,row in zip(primary['values'],before):
        need(actual[0]['exact_value']==str(row[0]) and len(actual)==8,'DATETIME row order differs')
        for field,text in zip(actual[1:],row[1:]):need(field['jdbc_type']==93 and field['runtime_class']==(None if text is None else 'java.time.LocalDateTime') and field['exact_value']==java_text(text),'DATETIME value not original JDBC calendar')
    need(before_keys==[['1970-01-01 00:00:00.000001',7],['2038-01-19 03:14:07.123456',9]],'DATETIME original primary keys differ')
    expected=copy.deepcopy(before);expected_keys=copy.deepcopy(before_keys)
    if commit:
        expected=[r for r in expected if r[0]!=20001];expected[0][1]='2038-01-19 03:14:07.123456';expected.append([10001,'2000-02-29 12:34:56.000001',None,None,None,None,None,None]);expected.sort()
        expected_keys=[['1000-01-01 00:00:00.000001',10],['1970-01-01 00:00:00.000001',8]]
    need(after==expected and after_keys==expected_keys,'DATETIME physical compensation differs')
    def fact(name):return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    hx=lambda v:'NULL' if v is None else v.encode().hex().upper()
    need(fact('independent-business')=='\n'.join('\t'.join([str(r[0]),*[hx(v) for v in r[1:]]]) for r in expected).encode(),'Independent DATETIME business differs')
    need(fact('independent-keys')=='\n'.join(hx(k)+'\t'+str(v) for k,v in expected_keys).encode(),'Independent DATETIME keys differ')
    need(fact('independent-tc')==b'0\t0\t0','DATETIME TC resources leaked')
    snapshot=changes.get('snapshot',load(root/'observations/before-end.json'));need(snapshot==dict(xid=xid,branch_id=branch,profile=profile,wrong_profile_rejected=True,counts=[profile,profile,0,1,6,1]),'DATETIME wrong configuration/Undo/SHA/locks not verified')
    negative=changes.get('negative',load(root/'observations/negative-checks.json'));need(negative==dict(invalid_date_mysql_code=1292,poisoned=True,zero_date_rejected_not_null=True,legacy_profile_rejected=True),'DATETIME invalid/zero date or wrong profile accepted')
    rm=changes.get('rm',frames(root/'RM-frames.jsonl')[0]);tm=changes.get('tm',frames(root/'TM-frames.jsonl')[0]);need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'DATETIME frame altered')
    registers=[r for r in rm if r['kind']==11 and r['direction']=='c2s'];reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];begins=[r for r in tm if r['kind']==1 and r['direction']=='c2s'];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s'];need(len(registers)==len(reports)==len(begins)==len(ends)==1,'DATETIME operation replayed or omitted')
    begin=correlated(tm,begins[0],2)['body'];need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'DATETIME original Begin differs')
    body=registers[0]['body'];actual,pos=string(body,2);need(actual==xid and body[pos]==0,'DATETIME not original AT branch');resource,pos=string(body,pos+1);locks,pos=string(body,pos,4);app,pos=string(body,pos,4)
    expected_locks=['undo_datetime:1','undo_datetime:10001','undo_datetime:20001',*['undo_datetime_keys:'+java_text(k) for k in ['1000-01-01 00:00:00.000001','1970-01-01 00:00:00.000001','2038-01-19 03:14:07.123456']]]
    need(pos==len(body) and app is None and sorted(locks.split(';'))==sorted(expected_locks),'DATETIME original primary key strings differ')
    receipt=correlated(rm,registers[0],12)['body'];need(receipt[2:4]==b'\x01\x00' and int.from_bytes(receipt[4:12],'big',signed=True)==branch,'DATETIME receipt differs')
    body=reports[0]['body'];actual,pos=string(body,2);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==2 and correlated(rm,reports[0],14)['body']==b'\x00\x0e\x01\x00','DATETIME report failed/changed')
    need(ends[0]['kind']==(7 if commit else 9) and string(ends[0]['body'],2)[0]==xid,'DATETIME end reversed');reply=correlated(tm,ends[0],8 if commit else 10)['body'];need(reply[2:]==b'\x01\x00'+bytes([{'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[proof['observed_end_status']]]),'DATETIME outcome not original response')
    callbacks=[r for r in rm if r['kind'] in [3,5] and r['direction']=='s2c'];need(0<len(callbacks)<=64,'DATETIME callback missing');succeeded=False
    for row in callbacks:
        body=row['body'];actual,pos=string(body,2);need(actual==xid and row['kind']==(3 if commit else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==0,'DATETIME callback switched identity');r,pos=string(body,pos+9);a,pos=string(body,pos,4);need(r==resource and a is None and pos==len(body),'DATETIME callback resource/app changed')
        response=correlated(rm,row,4 if commit else 6)['body'];actual,pos=string(response,4);need(response[2:4]==b'\x01\x00' and actual==xid and int.from_bytes(response[pos:pos+8],'big',signed=True)==branch and len(response)==pos+9 and response[-1] in ([5,6] if commit else [8,9]),'DATETIME branch response differs');succeeded|=response[-1]==(5 if commit else 8)
    need(succeeded,'DATETIME original branch never succeeded');journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),[dict(xid=xid,branch_id=branch,commit=commit)],resource)
    columns=fact('independent-state').decode().split('\t');need(len(columns)==9 and columns[:7]==[xid.encode().hex().upper(),str(branch),*(['1','0','1','1'] if commit else ['2','1','0','0']),'1'] and len(columns[7])==64 and columns[8]==str(profile),'DATETIME durable terminal/profile/witness differs')
    need(fact('independent-undo')==(b'' if commit else (xid.encode().hex().upper()+'\t'+str(branch)+'\t1').encode()),'DATETIME Undo/fence differs')
    need((('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in (root/'tc-stdout.log').read_bytes(),'Original TC DATETIME success missing')
    classes=(root/'oracle-class-load.log').read_bytes()
    for name in ['org.apache.seata.rm.datasource.sql.struct.TableRecords','org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser','org.apache.seata.rm.datasource.sql.struct.cache.DateTimeColumnOracle']:need(any((name+' source: file:/').encode() in line and b'/json-oracle/classes/' in line for line in classes.splitlines()),'DATETIME original fresh Java class origin differs')
    command=load(root/'client-command.json');need(command['exit_code']==0 and command['argv'][1:]==['original_java_tc_datetime_column','--ignored','--exact','--nocapture'] and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes() and load(root/'process-cleanup.json')['stopped'],'DATETIME native case/cleanup incomplete')

def verify(folder,digest):
    catalog(folder,digest);sources(folder);datetime_java_oracle.verify(folder);r=load(folder/'result.json');need(r['status']=='ORIGINAL_JAVA_TC_DATETIME_COLUMN_VERIFIED' and r['Java_pin']==guard.reg.PIN and r['Java_jar_sha256']==guard.reg.JAR_SHA and r['whole_plan_complete'] is False and r['actual_global_transactions']==len(r['Rust_versions'])*len(r['profiles'])*len(r['directions']),'DATETIME scope or pin differs')
    need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'],'DATETIME command/cleanup failed')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'DATETIME auth overlay replaced coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'DATETIME auth source drift')
    initial=load(folder/'inspect.stdout')[0];final=load(folder/'inspect-stopped.stdout')[0];need(initial['Id']==final['Id']==(folder/'create.stdout').read_text().strip() and initial['Image']==final['Image']==guard.IMAGE and initial['Mounts']==final['Mounts'] and not final['State']['Running'] and final['State']['ExitCode']==0,'DATETIME owned database identity/stop differs')
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
                for table in ['undo_datetime','undo_datetime_keys']:p=load(root/('observations/rust-java-'+table+'.json'));p['branchId']=1;variants.append({table:p})
                for target,kind in [('rm',11),('rm',13),('rm',3 if d=='commit' else 5),('tm',1),('tm',7 if d=='commit' else 9)]:rows=frames(root/('RM-frames.jsonl' if target=='rm' else 'TM-frames.jsonl'))[0];variants.append({target:[row for row in rows if row['kind']!=kind]})
                for variant in variants:
                    try:case(folder,v,m,d,variant)
                    except ValueError:count+=1
                    else:raise RuntimeError('DATETIME weakening accepted')
    return count

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256);print('DATETIME original UTC/JDBC/Undo/TC verified; weakenings rejected:',weakenings(a.folder) if a.weakenings else 0)
