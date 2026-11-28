"""Verify native JSON images, original JDBC/Undo and real TC branch completion."""
import argparse,copy,json,subprocess
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha,save
from validate_structured_logs import catalog,sources
from validate_at_concurrent import frames,correlated,journal,string
from xa_managed_java_faults import guard
import json_java_oracle


def load(path):return json.loads(path.read_bytes())


def case(folder,version,direction,changes=None):
    changes=changes or {};prefix=version+'-'+direction;root=folder/prefix;commit=direction=='commit'
    proof=changes.get('proof',load(root/'observations/result.json'));txs=proof['transactions']
    need(proof['status']=='AT_JSON_COLUMN_VERIFIED' and proof['direction']==direction and proof['concurrency']==proof['rounds']==len(txs)==1 and proof['tm_closed'] is True and proof['tc_remaining']==[0,0,0],'JSON original transaction incomplete')
    t=txs[0];xid=t['xid'];branch=t['branch_id'];need(t['id']==1 and t['commit'] is commit and branch>0,'JSON branch/direction differs')
    java=changes.get('java',load(root/'java-oracle/observations.json'));need(java['database']=='8.4.13' and java['jackson']=='2.18.3' and java['driver'].startswith('mysql-connector-java-8.0.27 ') and java['default_content_roundtrip'] is True,'Wrong original JSON JDBC profile')
    need(len(java['cases'])==1 and java['cases'][0]['rows']==9 and java['cases'][0]['original_roundtrip'] is True,'Missing original JSON records')
    j=java['cases'][0];column=j['columns'][1];need(column['name']=='v' and column['jdbc_type']==-1 and column['type_name']=='JSON' and column['native_type']=='json','JSON confused with TEXT/JDBC type')
    original=load(root/'java-oracle/undo_json.json');rust=changes.get('rust',load(root/'observations/rust-java-json-format.json'));need(original==rust,'Rust JSON format differs from original Java')
    before=changes.get('before',load(root/'observations/before-business.json'));after=changes.get('after',load(root/'observations/after-business.json'))
    checks=changes.get('negative',load(root/'observations/negative-checks.json'))
    need(checks==dict(invalid_json_mysql_code=3140,subsequent_execute_poisoned=True,oversized_json_rejected=True,invalid_row_count=0),'JSON invalid-input/budget checks incomplete')
    if not commit:
        dirty=changes.get('dirty',load(root/'observations/dirty-check.json'))
        need(dirty==dict(xid=xid,branch_id=branch,resolution='ManualInterventionRequired',original_undo_preserved=True,dirty_value='"dirty"',restored_after='"updated"',direct_resource_probe=True),'JSON dirty image accepted or original Undo not preserved')
    need(len(before)==9 and [v[0] for v in before]==[1,2,3,4,5,6,7,8,20001],'JSON sample set differs')
    need(before[1]==[2,'null'] and before[2]==[3,None] and '9007199254740993' in before[0][1] and '18446744073709551615' in before[0][1] and before[7][1]=='18446744073709551615','SQL NULL/JSON null/uint64 was lost')
    for actual,(key,value) in zip(j['values'],before):
        need(actual[0]['exact_value']==str(key) and actual[1]['name']=='v' and actual[1]['jdbc_type']==-1 and actual[1]['runtime_class']==(None if value is None else 'java.lang.String') and actual[1]['exact_value']==value,'Native image not original JDBC value')
    expected=copy.deepcopy(before)
    if commit:expected=[row for row in expected if row[0]!=20001];expected[0]=[1,'"updated"'];expected.append([10001,'{"inserted": 18446744073709551615}']);expected.sort()
    need(after==expected,'JSON physical compensation differs')
    def fact(name):return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    physical='\n'.join(str(k)+'\t'+('NULL' if v is None else v.encode().hex().upper()) for k,v in expected).encode()
    need(fact('independent-business')==physical and fact('independent-tc')==b'0\t0\t0','Independent JSON business/TC cleanup differs')
    rm=changes.get('rm',frames(root/'RM-frames.jsonl')[0]);tm=changes.get('tm',frames(root/'TM-frames.jsonl')[0])
    need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'JSON frame altered')
    reg=[r for r in rm if r['kind']==11 and r['direction']=='c2s'];reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];begins=[r for r in tm if r['kind']==1 and r['direction']=='c2s'];ends=[r for r in tm if r['kind'] in [7,9] and r['direction']=='c2s']
    need(len(reg)==len(reports)==len(begins)==len(ends)==1,'JSON global/branch operation replayed')
    begin=correlated(tm,begins[0],2)['body'];need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'JSON original Begin differs')
    body=reg[0]['body'];actual,pos=string(body,2);need(actual==xid and body[pos]==0,'JSON branch not original AT');resource,pos=string(body,pos+1);lock,pos=string(body,pos,4);app,pos=string(body,pos,4)
    need(pos==len(body) and app is None and sorted(lock.split(';'))==sorted(['undo_json:1','undo_json:10001','undo_json:20001']),'JSON original locks differ')
    ack=correlated(rm,reg[0],12)['body'];need(ack[2:4]==b'\x01\x00' and int.from_bytes(ack[4:12],'big',signed=True)==branch,'JSON original registration receipt differs')
    body=reports[0]['body'];actual,pos=string(body,2);need(actual==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==2 and correlated(rm,reports[0],14)['body']==b'\x00\x0e\x01\x00','JSON report not original PhaseOneDone')
    need(ends[0]['kind']==(7 if commit else 9) and string(ends[0]['body'],2)[0]==xid,'JSON direction changed')
    response=correlated(tm,ends[0],8 if commit else 10)['body'];need(response[2:]==b'\x01\x00'+bytes([{'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[t['observed_end_status']]]),'JSON end outcome differs')
    callbacks=[r for r in rm if r['kind'] in [3,5] and r['direction']=='s2c'];need(0<len(callbacks)<=64,'Missing original JSON callback');success=False
    for row in callbacks:
        body=row['body'];actual,pos=string(body,2);need(actual==xid and row['kind']==(3 if commit else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==0,'JSON callback switched identity');r,pos=string(body,pos+9);a,pos=string(body,pos,4);need(r==resource and a is None and pos==len(body),'JSON callback switched resource')
        response=correlated(rm,row,4 if commit else 6)['body'];actual,pos=string(response,4);need(response[2:4]==b'\x01\x00' and actual==xid and int.from_bytes(response[pos:pos+8],'big',signed=True)==branch and len(response)==pos+9,'JSON terminal reply differs');need(response[-1] in ([5,6] if commit else [8,9]),'JSON terminal status differs');success|=response[-1]==(5 if commit else 8)
    need(success,'JSON original branch never succeeded');journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),txs,resource)
    columns=fact('independent-state').decode().split('\t');need(columns[:7]==[xid.encode().hex().upper(),str(branch),*(['1','0','1','1'] if commit else ['2','1','0','0']),'1'] and len(columns)==8 and len(columns[7])==64,'JSON physical terminal/witness/checksum differs')
    need(fact('independent-undo')==(b'' if commit else (xid.encode().hex().upper()+'\t'+str(branch)+'\t1').encode()),'JSON Normal Undo cleanup/fence differs')
    logs=(root/'tc-stdout.log').read_bytes();need((('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in logs,'Original TC JSON success absent')
    classes=(root/'oracle-class-load.log').read_bytes()
    for name in ['org.apache.seata.rm.datasource.sql.struct.TableRecords','org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser','org.apache.seata.rm.datasource.sql.struct.cache.JsonColumnOracle']:
        need(any((name+' source: file:/').encode() in line and b'/json-oracle/classes/' in line for line in classes.splitlines()),'Original JSON class not freshly compiled origin')
    need(load(root/'client-command.json')['exit_code']==0 and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes() and load(root/'process-cleanup.json')['stopped'],'JSON native case/cleanup incomplete')
    need(load(root/'client-command.json')['argv'][1:]==['original_java_tc_json_column','--ignored','--exact','--nocapture'],'Wrong JSON native function')
    return dict(Rust=version,direction=direction,xid=xid,branch_id=branch)


def verify(folder,digest):
    catalog(folder,digest);sources(folder);json_java_oracle.verify(folder);r=load(folder/'result.json')
    need(r['status']=='ORIGINAL_JAVA_TC_JSON_COLUMN_VERIFIED' and r['Java_pin']==guard.reg.PIN and r['Java_jar_sha256']==guard.reg.JAR_SHA and r['whole_plan_complete'] is False,'JSON scope/pin differs')
    need(r['actual_global_transactions']==len(r['Rust_versions'])*len(r['directions']),'JSON transaction count differs')
    need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'],'JSON command failed/container remains')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'JSON fixture replaced original coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'JSON auth fixture source drift')
    initial=load(folder/'inspect.stdout')[0];final=load(folder/'inspect-stopped.stdout')[0];created=(folder/'create.stdout').read_text().strip();need(initial['Id']==final['Id']==created and initial['Image']==final['Image']==guard.IMAGE and initial['State']['Running'] and not final['State']['Running'] and final['State']['ExitCode']==0 and initial['Mounts']==final['Mounts'],'JSON owned database identity/mount/stop differs')
    for v in r['Rust_versions']:
        for d in r['directions']:case(folder,v,d)


def weakenings(folder):
    r=load(folder/'result.json');count=0
    for v in r['Rust_versions']:
        for d in r['directions']:
            root=folder/(v+'-'+d);variants=[]
            for key in ['independent-business','independent-tc','independent-state','independent-undo','journal']:variants.append({key:b'changed'})
            before=load(root/'observations/before-business.json');before[1][1]=None;variants.append(dict(before=before))
            java=load(root/'java-oracle/observations.json');java['cases'][0]['columns'][1]['jdbc_type']=12;variants.append(dict(java=java))
            rust=load(root/'observations/rust-java-json-format.json');rust['branchId']=1;variants.append(dict(rust=rust))
            after=load(root/'observations/after-business.json');after[0][1]='changed';variants.append(dict(after=after))
            changed=load(root/'observations/negative-checks.json');changed['oversized_json_rejected']=False;variants.append(dict(negative=changed))
            if d=='rollback':
                changed=load(root/'observations/dirty-check.json');changed['resolution']='Applied';variants.append(dict(dirty=changed))
            for key,kind in [('rm',11),('rm',13),('rm',3 if d=='commit' else 5),('tm',1),('tm',7 if d=='commit' else 9)]:
                rows=frames(root/('RM-frames.jsonl' if key=='rm' else 'TM-frames.jsonl'))[0];variants.append({key:[row for row in rows if row['kind']!=kind]})
            for variant in variants:
                try:case(folder,v,d,variant)
                except ValueError:count+=1
                else:raise RuntimeError('JSON evidence weakening accepted')
    return count


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256);print('JSON original-JDBC/Undo/TC proof verified; weakenings rejected:',weakenings(a.folder) if a.weakenings else 0)
