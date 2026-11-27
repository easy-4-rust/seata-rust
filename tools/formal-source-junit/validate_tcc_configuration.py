"""Independently bind configuration transitions to sealed sources, raw SQL and context digests."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
from inventory_rebuilt import need
from validate_structured_logs import catalog, sources

NAMES=['configuration-static-try','configuration-static-callback','configuration-static-compat','configuration-permission']

def load(p):return json.loads(p.read_bytes())
def exact(value,expected):return type(value) is type(expected) and value==expected

def case(folder,version,index,alteration=None):
    prefix=version+'-case-'+str(index)
    proof=load(folder/'observations'/version/('case-'+str(index))/'proof'/(NAMES[index]+'.json'))
    facts={name:(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b'') for name in ['business','fence','undo','business-dml','fence-protocol','full-fence','context']}
    if alteration:
        proof=alteration.get('proof',proof);facts.update(alteration.get('facts',{}))
    need(b'test result: ok. 1 passed;' in (folder/(prefix+'.stdout')).read_bytes(),'Actual configuration test missing')
    need(type(proof['rust_process_id']) is int and proof['rust_process_id']>0 and facts['undo']==b'0','Native execution or unchanged Undo missing')
    if index==0:
        expected={'accepted':False,'fence_rows':0,'business':[],'original_receipt_retained':True}
    elif index==1:
        expected={'stage':'completed','static_result':'Failed','static_branch_status':9,'static_fence_rows':0,'static_journal':'None','journal':'Completed','status':4,'protocol':2,'late_try_rejected':True,'business':[]}
    elif index==3:
        expected={'denied_code':1142,'before_grant_journal':'None','before_grant_fence_rows':0,'after_grant_correct_mode':'Completed','status':4,'protocol':2,'max_connections':1,'business':[]}
    else:
        expected={}
        need(len(proof['cases'])==3,'Static compatibility cases missing')
        for row,mode in zip(proof['cases'],['no_optional_table','other_dynamic_branch','static_origin0'],strict=True):
            for key,value in {'mode':mode,'status':2,'protocol':1,'journal':'Completed','max_connections':1,'business':[[1,14]]}.items():
                need(exact(row[key],value),'Static compatibility changed: '+key)
    for key,value in expected.items():need(exact(proof[key],value),'Configuration outcome differs: '+key)
    dml=[]
    for line in facts['business-dml'].splitlines():
        kind,sql=line.split(b'\t',1);need(kind in [b'Execute',b'Query'],'Unknown DML kind');dml.append(sql)
    insert=b'INSERT INTO tcc_managed(id,v) VALUES(1,7)';update=b'UPDATE tcc_managed SET v=14 WHERE id=1'
    need(dml==([insert,update,insert,insert,update,insert,update] if index==2 else [insert]),'Unexpected business replay/DML sequence')
    need(facts['business']==(b'1\t14' if index==2 else b''),'Independent business differs')
    final=proof['cases'][-1] if index==2 else proof
    context=[]
    for line in facts['context'].splitlines():
        fields=line.decode().split('\t');need(len(fields)==9,'Context field count differs')
        x,branch,revision,resource,original,app,server,protocol,checksum=fields
        x,resource,original,app,server,checksum=map(bytes.fromhex,[x,resource,original,app,server,checksum])
        revision=int(revision);branch=int(branch)
        digest=hashlib.sha256(b'seata-rs-tcc-context-v1')
        for value in [x,resource,original,app,server]:digest.update(len(value).to_bytes(8,'big'));digest.update(value)
        digest.update(branch.to_bytes(8,'big',signed=True));digest.update(revision.to_bytes(2,'big'))
        need(digest.digest()==checksum and protocol=='1','Original context SHA or protocol changed')
        need(x.decode()==final['xid'] and str(branch)==final['branch_id'] and resource==b'tcc-original-action' and len(server)==36,'Context identity differs')
        context.append((revision,original,app,server))
    need([r[0] for r in context]==([0] if index==2 else [0,1]),'Durable version sequence differs')
    need(context[0][1]==context[0][2],'Registration bytes changed')
    if index in [1,3]:
        need(context[0][1].decode()==proof['original_application_data'] and context[1][2].decode()==proof['reported_application_data'] and context[1][1]==context[0][1] and context[1][3]==context[0][3],'Report/canonical origin mismatch')
        need(context[0][1]!=context[1][2],'Latest report did not actually change')
    if index==0:
        need(not facts['fence'] and not facts['fence-protocol'] and not facts['full-fence'],'Static Try wrote Fence')
    else:
        status,protocol=(2,1) if index==2 else (4,2)
        need(facts['fence-protocol']==str(protocol).encode(),'Fence protocol downgraded')
        need(facts['fence']==(final['xid'].encode().hex().upper()+'\t'+final['branch_id']+'\t'+str(status)).encode(),'Independent Fence differs')
        row=facts['full-fence'].decode().split('\t');need(len(row)==7,'Full Fence shape differs')
        need(bytes.fromhex(row[0]).decode()==final['xid'] and row[1]==final['branch_id'] and bytes.fromhex(row[2])==b'tcc-original-action' and bytes.fromhex(row[3])==context[0][1] and bytes.fromhex(row[4])==context[0][3] and row[5:]==[str(protocol),str(status)],'Original Fence identity/terminal changed')
    return proof,facts

def verify(folder,digest,weaken=False):
    catalog(folder,digest);sources(folder)
    result=load(folder/'result.json')
    need(result['status']=='TCC_CONFIGURATION_TRANSITION_CASES_EXECUTED' and result['Rust_versions']==['1.99','1.85'] and result['selected_case'] is None and result['expected_failure'] is None and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False,'Configuration scope differs')
    need(load(folder/'cleanup.json')['owned_container_removed'] is True,'Owned fixture remains')
    commands=load(folder/'commands.json');need(all(row['exit_code']==(1 if row['label']=='confirm-removed' else 0) for row in commands),'Native command failure')
    rejected=0
    for version in ['1.99','1.85']:
        for index in range(4):
            proof,facts=case(folder,version,index)
            if not weaken:continue
            alternatives=[{'facts':{key:value}} for key,value in [('business',b'99\t99'),('undo',b'1'),('business-dml',b''),('context',b'')]]
            if index!=0:alternatives += [{'facts':{key:b''}} for key in ['fence','fence-protocol','full-fence']]
            keys={0:[('accepted',True),('original_receipt_retained',False)],1:[('static_journal','Pending'),('static_branch_status',8),('late_try_rejected',False)],2:[],3:[('denied_code',1146),('before_grant_journal','Pending'),('after_grant_correct_mode','Pending')]}
            for key,value in keys[index]:
                changed=copy.deepcopy(proof);changed[key]=value;alternatives.append({'proof':changed})
            if index==2:
                changed=copy.deepcopy(proof);changed['cases'][0]['max_connections']=2;alternatives.append({'proof':changed})
            for change in alternatives:
                try:case(folder,version,index,change)
                except ValueError:rejected+=1
                else:raise RuntimeError('Accepted configuration evidence weakening')
    return rejected

def verify_mutations(folder,digest):
    from inventory_rebuilt import ROOT, sha
    catalog(folder,digest);sources(folder)
    result=load(folder/'result.json')
    need(result['status']=='CONFIGURATION_FAULTY_PRODUCTION_SOURCES_REJECTED' and result['variants_per_version']==3 and result['actual_behavioral_failures']==6 and result['live_source_mutated'] is result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False,'Configuration mutation scope changed')
    tests={
        'static_try_origin_bypassed':('configuration_cases::static_try_cannot_replay_original_receipt_after_unconfirmed_dynamic_report',b'static Try must reject a branch with durable dynamic origin'),
        'static_prejournal_origin_bypassed':('configuration_cases::static_callback_cannot_poison_manifest_only_origin_before_correct_configuration_recovers',b'assertion failed: state.is_none()'),
        'permission_denied_treated_missing':('configuration_cases::static_origin_access_denial_is_not_missing_table_and_correct_mode_recovers',b'called `Result::unwrap_err()` on an `Ok` value'),
    }
    rows=load(folder/'records.json');need(len(rows)==6 and {(r['Rust'],r['mutation']) for r in rows}=={(v,m) for v in ['1.99','1.85'] for m in tests},'Required counterexample absent')
    cleanup=load(folder/'cleanup.json');need(cleanup['owned_container_removed'] is True and Path(cleanup['isolated_source']).drive.upper()=='D:','Mutation fixture cleanup/source differs')
    targets=[Path(p).resolve() for p in cleanup['isolated_targets']]
    need(len(targets)==len(set(targets))==8 and all(p.parent.parent==Path('E:/codex-builds').resolve() and p.parent.name==Path(cleanup['isolated_source']).parent.name for p in targets),'Mutation build targets escape owned E directory')
    commands=load(folder/'commands.json');artifacts=load(folder/'artifacts.json')
    need(all(r['exit_code']==(1 if r['label']=='confirm-removed' else 101 if r['label'] in {a['Rust']+'-'+a['mutation'] for a in rows} else 0) for r in commands),'Mutation build/setup failure')
    for version in ['1.99','1.85']:
        for index in range(3):need(b'test result: ok. 1 passed;' in (folder/(version+'-baseline-'+str(index)+'.stdout')).read_bytes(),'Unmodified isolated baseline missing')
    for row in rows:
        prefix=row['Rust']+'-'+row['mutation'];test,signature=tests[row['mutation']]
        need(row['test']==test and row['exit_code']==101 and sha(folder/(prefix+'.rs'))==row['modified_source_sha256'],'Modified source/test binding differs')
        replacements={
            'static_try_origin_bypassed':('            MySqlTccContextStore::assert_no_dynamic_origin(tx.control()?, &self.source, id, true)\n                .await?;','            let _ = id;'),
            'static_prejournal_origin_bypassed':('                            false,\n                        )\n                        .await?;','                            false,\n                        )\n                        .await.ok();'),
            'permission_denied_treated_missing':('e.number() == 1146','e.number() == 1146 || e.number() == 1142'),
        }
        original=(ROOT/row['path']).read_bytes().decode();before,after=replacements[row['mutation']]
        need(original.count(before)==1 and (folder/(prefix+'.rs')).read_bytes()==original.replace(before,after).encode(),'Unrelated source rewrite counted as selected counterexample')
        need(b'test result: FAILED. 0 passed; 1 failed;' in (folder/(prefix+'.stdout')).read_bytes() and signature in (folder/(prefix+'.stderr')).read_bytes(),'Required behavioral failure absent')
        command=[c for c in commands if c['label']==prefix];copy_rows=[a for a in artifacts if a['label']==prefix+'-build']
        need(len(command)==len(copy_rows)==1 and test in command[0]['argv'] and command[0]['exit_code']==101,'Exact faulty native execution absent')
        artifact=Path(command[0]['argv'][0]).resolve();copy_row=copy_rows[0]
        need(Path(copy_row['executed_path']).resolve()==artifact and copy_row['sha256']==row['artifact_sha256'] and any(p.name=='variant-target-'+prefix and artifact.is_relative_to(p) and Path(copy_row['built_path']).resolve().is_relative_to(p) for p in targets),'Executed artifact not isolated/bound')
    return len(rows)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');p.add_argument('--mutations',action='store_true');a=p.parse_args()
    if a.mutations:print('Verified actual faulty configuration production sources:',verify_mutations(a.folder,a.sha256))
    else:print('Verified eight native configuration functions; rejected evidence weakenings:',verify(a.folder,a.sha256,a.weakenings))
