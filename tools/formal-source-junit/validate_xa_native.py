"""Validate original Java bytes, native two-phase facts, restart and effective source controls."""
import argparse,copy,datetime,json,shutil,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha,safe_path
from validate_structured_logs import catalog,sources

OPERATIONS={'java_prefix','omit_prepare','wrong_commit_direction','ignore_format','ignore_qualifier','unknown_as_not_sent','omit_constraints'}
BUSINESS=[b'1\t11',b'1\t10\n7\t70',b'1\t10',b'1\t10',b'1\t10',b'1\t10\n22\t220',b'1\t10',b'1\t10\n10\t100']

def original_java(folder):
    commands=json.loads((folder/'commands.json').read_bytes());need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Original Java oracle not executed')
    result=json.loads((folder/'result.json').read_bytes());need(result['Java_pin']=='cba73e180fd8896a99df6415b859d275122feaa7' and result['vectors']==8,'Java pin/count changed')
    for name in ['XAXid','XABranchXid','XAXidBuilder']:
        original=subprocess.check_output(['git','show',result['Java_pin']+':rm-datasource/src/main/java/org/apache/seata/rm/datasource/xa/'+name+'.java'],cwd=ROOT.parent/'seata-java-phase09-oracle');need((folder/(name+'.java')).read_bytes()==original,'Oracle rewrote original Java class')
    need((folder/'XidOracle.java').read_bytes()==(ROOT/'tools/xa/java/XidOracle.java').read_bytes(),'Executed Java wrapper drift')
    fixture=safe_path(ROOT,result['fixture']);need(sha(fixture)==result['fixture_sha256'] and fixture.read_bytes()==(folder/'oracle.stdout').read_bytes(),'Original Java bytes do not match Rust fixture')
    need(len(fixture.read_bytes().splitlines())==8,'Java vectors incomplete')

def native_rust(folder):
    sources(folder);need(json.loads((folder/'commands.json').read_bytes())[0]['exit_code']==0,'XA identity targets failed')
    output=(folder/'tests.stdout').read_bytes();need(b'test result: ok. 4 passed; 0 failed;' in output and b'test result: ok. 1 passed; 0 failed;' in output and b'original_java_vectors_match_every_component ... ok' in output,'Final original-byte targets not executed')

def packet(raw):
    need(type(raw) is list and len(raw)>=5 and all(type(v) is int and 0<=v<=255 for v in raw),'Invalid original native packet')
    value=bytes(raw);need(int.from_bytes(value[:3],'little')==len(value)-4,'Original packet length differs');return value

def loss(proof):
    need(proof['automatic_replay'] is False and len(proof['cases'])==3,'XA response-loss matrix missing')
    for index,row in enumerate(proof['cases']):
        verb=['PREPARE','COMMIT','ROLLBACK'][index];branch=201+index;need(row['operation']==verb and type(row['branch_id']) is int and row['branch_id']==branch,'XA original operation/identity changed')
        request=packet(row['request']);response=packet(row['held_original_ok']);expected=("XA "+verb+" X'"+b'xa:native:ok-loss'.hex().upper()+"',X'"+('-'+str(branch)).encode().hex().upper()+"',9752").encode()
        need(request[3]==0 and request[4:]==b'\x03'+expected and response[3]==1 and response[4]==0,'Held OK not bound to original text-protocol XA command')
        need(row['result']=='Unknown' and row['prepared_observed'] is (index==0),'XA unknown replaced by guessed terminal')
        need(row['business_value']==(220 if index==1 else None),'Independent response-loss business fact differs')
        need(type(row['actual_commands']) is int and row['actual_commands']==1,'XA control automatically replayed')

def prepared_bytes(raw):
    rows=raw.strip().splitlines();need(len(rows)==2,'Original prepared set incomplete');found=set()
    for row in rows:
        columns=row.split(b'\t');need(len(columns)==4 and int(columns[0])==9752,'Original XA format differs')
        global_id=b'xa:native:restart';payload=bytes.fromhex(columns[3].decode().removeprefix('0x'));need(int(columns[1])==len(global_id) and payload[:len(global_id)]==global_id,'Original global bytes differ')
        branch=payload[len(global_id):];need(int(columns[2])==len(branch) and branch in [b'-101',b'-102'],'Original branch bytes differ');found.add(branch)
    need(found=={b'-101',b'-102'},'Duplicate or wrong prepared branch')

def mysql(folder):
    sources(folder);commands=json.loads((folder/'commands.json').read_bytes());need(all(c['exit_code']==0 for c in commands if c['label']!='confirm-removed'),'XA runtime command failed')
    need(next(c for c in commands if c['label']=='confirm-removed')['exit_code']!=0 and json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Owned XA container remains')
    state=json.loads((folder/'inspect.stdout').read_bytes())[0];need(state['Id']==(folder/'create.stdout').read_text().strip() and state['Image']=='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902' and state['HostConfig']['PortBindings']['3306/tcp'][0]['HostIp']=='127.0.0.1','Owned runtime identity/bind drift')
    rejected={}
    for version in ['1.99','1.85']:
        for index,business in enumerate(BUSINESS):
            need(b'test result: ok. 1 passed;' in (folder/(version+'-case-'+str(index)+'.stdout')).read_bytes(),'Native XA case not executed')
            need((folder/(version+'-case-'+str(index)+'-independent-business.stdout')).read_bytes().strip()==business,'Independent per-case business state differs')
            prepared=(folder/(version+'-case-'+str(index)+'-independent-prepared.stdout')).read_bytes()
            if index==6:prepared_bytes(prepared)
            else:need(not prepared.strip(),'Unexpected remaining prepared branch')
        before=(folder/(version+'-prepared-before-restart.stdout')).read_bytes();after=(folder/(version+'-prepared-after-restart.stdout')).read_bytes();prepared_bytes(before);prepared_bytes(after);need(sorted(before.splitlines())==sorted(after.splitlines()),'Prepared bytes changed on MySQL restart')
        for stage in ['before','after']:need((folder/(version+'-invisible-'+stage+'-restart.stdout')).read_bytes().strip()==b'1\t10','Prepared rows exposed before explicit resolution')
        obs=folder/'observations'/version;pre=json.loads((obs/'case-6/before-restart.json').read_bytes());post=json.loads((obs/'case-7/after-restart.json').read_bytes())
        need(pre['server_uuid']==post['server_uuid'] and type(pre['server_uuid']) is str and len(pre['server_uuid'])==36,'Restart did not preserve physical instance')
        need(type(pre['rust_process_id']) is int and type(post['rust_process_id']) is int and pre['rust_process_id']!=post['rust_process_id'],'XA recovery did not use a new native process')
        need(post['committed_value']==100 and post['rollback_row'] is None and post['prepared_remaining']==0,'New process resolution facts differ')
        proof=json.loads((obs/'case-5/original-xa-ok-loss.json').read_bytes());loss(proof);rejected[version]=[]
        cancellation=json.loads((obs/'case-2/poison-and-cancel.json').read_bytes())
        def cancel(value):
            need(type(value['actual_lock_waits_before_cancel']) is int and value['actual_lock_waits_before_cancel']>0,'Actual MySQL row wait missing before cancellation')
            need(type(value['blocker_connection_id']) is int and value['blocker_connection_id']>0 and value['cancelled_lock_wait_poisoned'] is True and value['partial_insert_absent'] is True,'Cancellation original-session facts missing')
        cancel(cancellation)
        for field,value in [('actual_lock_waits_before_cancel',0),('actual_lock_waits_before_cancel',True),('blocker_connection_id',0)]:
            changed=copy.deepcopy(cancellation);changed[field]=value
            try:cancel(changed)
            except (ValueError,AssertionError,KeyError,TypeError):rejected[version].append('false-cancellation-'+field+'-'+str(value))
            else:raise ValueError('Weakened cancellation evidence accepted')
        def reject(name,value):
            try:loss(value)
            except (ValueError,AssertionError,KeyError,TypeError):rejected[version].append(name)
            else:raise ValueError('Weakened XA evidence accepted: '+name)
        for field,value in [('result','Success'),('prepared_observed',False),('branch_id',999),('actual_commands',2),('actual_commands',True),('held_original_ok',[0,0,0,1,0]),('request',[0,0,0,0,3])]:
            changed=copy.deepcopy(proof);changed['cases'][0][field]=value;reject('wrong-'+field,value=changed)
        changed=copy.deepcopy(proof);changed['cases'][1]['business_value']=None;reject('commit-business-missing',changed)
        changed=copy.deepcopy(proof);changed['cases']=changed['cases'][:-1];reject('missing-loss-case',changed)
    return rejected

def mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes());need(len(rows)==14 and {(r['Rust'],r['operation']) for r in rows}=={(v,k) for v in ['1.99','1.85'] for k in OPERATIONS},'Effective XA source controls incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256']);commands=json.loads((child/'commands.json').read_bytes())
        names=['tests.stdout','tests.stderr'] if row['operation']=='java_prefix' else [row['Rust']+'-live.stdout',row['Rust']+'-live.stderr']
        need(row['marker'].encode() in b''.join((child/name).read_bytes() for name in names),'XA control failed for unrelated reason')
        if row['operation']=='java_prefix':need(len(commands)==1 and commands[0]['exit_code']==101,'Java byte control did not fail real Rust target')
        else:need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101 and b'test result: FAILED.' in (child/names[0]).read_bytes(),'Missing actual native failure')
    for row in json.loads((folder/'restored.json').read_bytes()):need(row['exact_original_restored'] is True and sha(safe_path(ROOT,row['path']))==row['sha256'],'XA production source not restored')

def run(args):
    folders={key:safe_path(ROOT,getattr(args,key)) for key in ['java','rust199','rust185','mysql','mutations','regression199','regression185','default']}
    for key,folder in folders.items():catalog(folder,getattr(args,key+'_sha'))
    original_java(folders['java']);native_rust(folders['rust199']);native_rust(folders['rust185']);rejected=mysql(folders['mysql']);mutations(folders['mutations'])
    for key,version in [('regression199','1.99'),('regression185','1.85')]:
        folder=folders[key];result=json.loads((folder/'result.json').read_bytes());need(result['Rust']==version and result['passed']==785 and result['ignored']==356 and result['strict_xa_mysql_all_targets_clippy'] is True,'Full regression totals differ');need(all(c['exit_code']==0 for c in json.loads((folder/'commands.json').read_bytes())),'Workspace/Clippy command failed')
        for row in result['production_sources']:need(sha(safe_path(ROOT,row['path']))==row['sha256'],'Regression production bytes drift')
    need(len(json.loads((folders['default']/'commands.json').read_bytes()))==12 and all(c['exit_code']==0 for c in json.loads((folders['default']/'commands.json').read_bytes())),'Default/opt-in matrix failed')
    for version in ['1.99','1.85']:
        need(b'seata-xa v' not in (folders['default']/(version+'-default-normal-tree.stdout')).read_bytes() and b'seata-xa v0.1.0' in (folders['default']/(version+'-xa-normal-tree.stdout')).read_bytes(),'XA dependency gate differs')
    out=ROOT/'docs/phase-4/evidence/mysql-xa'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='JAVA_XA_BYTES_AND_NATIVE_MYSQL_TWO_PHASE_RESTART_VALIDATED',authority={k:dict(path=getattr(args,k),catalog_sha256=getattr(args,k+'_sha')) for k in folders},rejected_controls=rejected,source_controls_rejected=14,original_Java_TC_XA_rerun=False,XA_RM_gate_complete=False,AT_WP05_gate_complete=False,whole_plan_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['java','rust199','rust185','mysql','mutations','regression199','regression185','default']:parser.add_argument('--'+key,required=True);parser.add_argument('--'+key+'-sha',required=True)
    run(parser.parse_args())
