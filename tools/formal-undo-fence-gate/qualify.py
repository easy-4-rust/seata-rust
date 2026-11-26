"""Explicit original-identity fence qualification; current real cases plus scoped inherited regression authority."""
import datetime
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
BASE=ROOT/'docs/phase-2/evidence/lock-key-gate/run_20261116_011215_dc9906b8'
STORE=ROOT/'docs/phase-2/evidence/undo-store-gate/run_20261116_032443_ec53c018'
sys.path.insert(0,str(ROOT/'tools/formal-undo-fence'))
from validate_wire import validate,negative_controls
SNAPSHOT='mysql::mysql_undo_log_manager::tests::older_control_snapshot_reads_the_later_committed_guard'
READY='connection_proxy::tests::ready_undo_publication_after_deadline_keeps_poison_and_rolls_back_original_transaction'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(value,message):
    if not value:raise ValueError(message)
def role(rust,fence,loss,store,ready):
    need(fence['verified'] is True and fence['Rust']==rust and fence['selected'] is None and fence['fault'] is None and fence['status']=='SELECTED_MYSQL_UNDO_FENCE_OBSERVATIONS_VERIFIED' and any('test result: ok. 9 passed; 0 failed; 0 ignored;' in s for s in fence['actual_test_summary']),'Fence role/toolchain/count differs')
    need(len(loss)==3 and {p['fault'] for p in loss}=={'drop_ok','block_commit','delay_ok'} and all(p['verified'] is True and p['Rust']==rust and p['selected']=='unknown_control_commit_requires_original_identity_observation' and p['status']=='SELECTED_MYSQL_UNDO_FENCE_OBSERVATIONS_VERIFIED' and any('test result: ok. 1 passed; 0 failed; 0 ignored;' in s for s in p['actual_test_summary']) for p in loss),'Fault role/toolchain/set differs')
    need(store['verified'] is True and store['Rust']==rust and store['selected'] is None and store['status']=='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED' and any('test result: ok. 10 passed; 0 failed;' in s for s in store['actual_test_summary']),'Store regression role differs')
    need(ready['verified'] is True and ready['Rust']==rust and ready['selected']==READY and ready['status']=='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED' and any('test result: ok. 1 passed; 0 failed;' in s for s in ready['actual_test_summary']),'Ready regression role differs')
def run(manifest):
    manifest=manifest.resolve();values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both toolchains required')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/undo-fence-gate'/('run_'+stamp);out.mkdir(parents=True);print('UNDO_FENCE_GATE_RUN '+str(out),flush=True)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def packet(raw,digest=None):
        p=Path(raw);p=p if p.is_absolute() else ROOT/p;need(p.is_relative_to(ROOT/'docs/phase-2/evidence'),'Packet outside repository evidence')
        if digest:need(sha(p/'catalog.json')==digest,'Packet digest differs')
        rows=read(p/'catalog.json')['members'];need(len(rows)==len({r['path'] for r in rows}) and {r['path'] for r in rows}=={q.relative_to(p).as_posix() for q in p.rglob('*') if q.is_file() and q!=p/'catalog.json'},'Packet member set differs')
        for row in rows:need(sha(p/row['path'])==row['sha256'],'Packet member hash differs')
        return dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=sha(p/'catalog.json'),members=len(rows),result=read(p/'result.json'))
    inherited=packet(STORE,'fa6199466c5eb6e81b24907532509f0024b0b23aa43b19a8b19847aeb83b8314')
    packet(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    previous={Path(r['path']):r for r in read(STORE/'source-inputs.json')}
    for path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:need(sha(path)==previous[path]['sha256'],'Dependency edges/versions changed')
    changes={p.relative_to(ROOT).as_posix() for p,r in previous.items() if p.is_relative_to(ROOT/'crates') and '/src/' in p.as_posix() and sha(p)!=r['sha256']}
    need(changes=={'crates/seata_db_mysql/src/data_source_proxy.rs','crates/seata_db_mysql/src/lib.rs','crates/seata_db_mysql/src/mysql/mod.rs','crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs'},'Unexpected existing source change')
    suffixes={'crates/seata_db_mysql/src/data_source_proxy.rs':'    /// 参数为有限连接/TLS配置','crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs':'    /// 参数为原事务/schema/身份与已验证原格式字节'}
    for name,marker in suffixes.items():
        path=ROOT/name;before=(STORE/previous[path]['snapshot']).read_text(encoding='utf-8');after=path.read_text(encoding='utf-8');need(before[before.index(marker):].split('#[cfg(test)]')[0].strip()==after[after.index(marker):].split('#[cfg(test)]')[0].strip(),'Existing selected database method bodies changed')
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-undo-fence-contract.md',ROOT/'tools/formal-undo-format/runtime_binding.py'}
    for folder in [ROOT/'crates',ROOT/'tools/formal-undo-fence',ROOT/'tools/formal-undo-store',HERE]:paths.update(p for p in folder.rglob('*') if p.is_file())
    for path,row in previous.items():
        if '/registry/' in str(path).replace('\\','/'):
            need(sha(path)==row['sha256'],'Qualified dependency source/archive drift');paths.add(path)
    frozen=[]
    for path in sorted(paths):
        name='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(name,path.read_bytes());frozen.append(dict(path=str(path),snapshot=name,sha256=sha(path)))
    save('source-inputs.json',frozen);save('explicit-packets.json',values)
    live={};proof_paths=[];wire_controls=[];cleanup=[];unconsumed=[]
    for rust,profile in values['profiles'].items():
        proofs={key:packet(profile[key]) for key in ['fence','store','ready','snapshot']};loss=[packet(p) for p in profile['loss']]
        role(rust,proofs['fence']['result'],[p['result'] for p in loss],proofs['store']['result'],proofs['ready']['result'])
        snapshot=proofs['snapshot']['result'];need(snapshot['verified'] is True and snapshot['Rust']==rust and snapshot['selected']==SNAPSHOT and snapshot['status']=='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED' and any('test result: ok. 1 passed; 0 failed;' in s for s in snapshot['actual_test_summary']),'Exact current-read snapshot role differs')
        for proof in [*proofs.values(),*loss]:
            p=ROOT/proof['path'];need((p/'rust-version.txt').read_text(encoding='utf-8').startswith('rustc '+rust+'.'),'Actual recorded live compiler version differs');proof_paths.append(str(p));need(read(p/'cleanup.json')['owned_container_removed'] is True,'Owned runtime cleanup incomplete');cleanup.append(read(p/'owned-runtime.json'))
            for row in read(p/'source-inputs.json'):
                path=Path(row['path'])
                # 单独integration target不会编译其他integration测试；保留其原快照，不冒充当前执行输入。
                if path==ROOT/'crates/seata_db_mysql/tests/live_undo_fence.rs' and proof is not proofs['fence']:
                    unconsumed.append(dict(packet=proof['path'],path=str(path),archived_sha256=row['sha256'],reason='other integration target not compiled or executed in this exact role'));continue
                need(sha(path)==row['sha256'],'Consumed current live source drift')
        for key,comparisons in [('fence',2),('store',4)]:
            peer=proofs[key]['result']['Java_peer'];bound=packet(peer['run'],peer['catalog_sha256']);need(bound['result']['comparisons']==comparisons and bound['result']['actual_JDBC']=='8.0.27' and bound['result']['original_newly_compiled_sources']==2,'Actual original Java scope differs');proofs[key+'_Java']=bound
        for proof in loss:
            p=ROOT/proof['path'];wire=read(p/'mysql-wire.json');observation=read(p/'observations/wire-observation.json');decoded=validate(wire,observation)
            need(decoded==read(p/'wire-validation.json'),'Independent raw wire result differs');controls=negative_controls(wire,observation);need(len(controls)==5 and controls==read(p/'wire-negative-controls.json'),'Independent actual wire controls differ');wire_controls.extend(dict(Rust=rust,mode=wire['mode'],**c) for c in controls)
        live[rust]=dict(**proofs,loss=loss)
    need(len(proof_paths)==len(set(proof_paths))==14,'Current toolchain/role packets reused')
    red=packet(values['red'],'76714c032a82361fd92087f79735e7441bc3c667af496b537ddd4040ff8489b6');r=red['result'];need(r['verified'] is True and r['status']=='EXPECTED_MISSING_FENCE_RED' and r['Rust']=='1.99' and r['selected']=='durable_guard_blocks_two_original_late_normal_writers_without_replaying_business' and any('test result: FAILED. 0 passed; 1 failed;' in s for s in r['actual_test_summary']),'Original missing fence RED differs')
    snapshot_red=packet(values['snapshot_red'],'6ca686b6d33b625de5d428f105504a9c07419a7c41e0e011039fb6831fcb591d');r=snapshot_red['result'];need(r['verified'] is True and r['Rust']=='1.99' and r['selected']==SNAPSHOT and r['status']=='EXPECTED_MISSING_STORE_RED' and any('test result: FAILED. 0 passed; 1 failed;' in s for s in r['actual_test_summary']) and 'existing guard must use a current read' in (ROOT/snapshot_red['path']/'live-tests.log').read_text(encoding='utf-8'),'Deterministic older snapshot RED differs')
    controls=[]
    def reject(name,call):
        try:call()
        except ValueError as error:controls.append(dict(control=name,rejection=str(error)))
        else:raise ValueError('Altered actual role accepted: '+name)
    for rust,proofs in live.items():
        f=proofs['fence']['result'];loss=[p['result'] for p in proofs['loss']];s=proofs['store']['result'];ready=proofs['ready']['result']
        reject('wrong_toolchain_'+rust,lambda:role('1.85' if rust=='1.99' else '1.99',f,loss,s,ready))
        reject('duplicate_fault_'+rust,lambda:role(rust,f,[loss[0],loss[0],loss[2]],s,ready))
        changed=copy.deepcopy(f);changed['selected']='some_other_case';reject('partial_fence_as_full_'+rust,lambda:role(rust,changed,loss,s,ready))
    save('packet-role-controls.json',controls);save('independent-wire-controls.json',wire_controls);save('unconsumed-frozen-superset-files.json',unconsumed)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    def command(args,name,extra=None):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,v.stdout);need(v.returncode==0,name+' failed');return v.stdout
    command(['cargo','fmt','--all','--','--check'],'fmt.log');command(['cargo','clippy','--locked','--offline','--workspace','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','check','--locked','--offline','--workspace','--all-targets','--all-features'],'all-features.log')
    command(['cargo','doc','--locked','--offline','-p','seata-db-mysql','--no-deps'],'doc.log',dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-undo-fence-doc'))
    ordinary={}
    for rust in ['1.99','1.85']:
        extra={};cargo='cargo'
        if rust=='1.85':
            tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';extra=dict(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/rust-version.txt',extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--all-targets','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra)
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(int(v) for v in counts);need(passed==331,'Expected325 existing lib/integration +6 existing example tests differs')
        artifacts=[]
        for line in raw.splitlines():
            try:item=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(item,dict):artifacts.append(item)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')]
        compiled={r['package_id'].rsplit('#',1)[1] for r in artifacts if r.get('reason')=='compiler-artifact' and r['package_id'].startswith('registry+')}
        need(len(tested)==58 and len(counts)==len(tested),'Actual harness summaries differ from compiled test artifact count')
        receipts=[]
        for item in tested:
            path=Path(item['executable']);digest=sha(path);snapshot=rust+'/artifacts/'+digest[:24]+'.exe';put(snapshot,path.read_bytes());receipts.append(dict(path=str(path),sha256=digest,snapshot=snapshot))
        save(rust+'/test-artifacts.json',receipts);save(rust+'/compiled-registry-packages.json',sorted(compiled));need(len(compiled)==150,'Actual compiled dependency closure changed')
        expected_compiled={'@'.join(parts) for parts in read(BASE/rust/'compiled-registry.json')};need(compiled==expected_compiled,'Actual compiled dependency set differs from qualified baseline')
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw);need(len(groups)==2,'Actual RM recovery child process groups missing')
        children=[]
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'RM recovery directory escapes D workspace')
            for path in directory.rglob('*'):
                if path.is_file():
                    snapshot=rust+'/rm-restarts/'+phase.decode()+'/'+path.relative_to(directory).as_posix();put(snapshot,path.read_bytes());children.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
        save(rust+'/RM-child-evidence.json',children)
        ordinary[rust]=dict(passed=passed,existing_lib_integration_tests=325,existing_example_tests=6,test_artifacts=len(tested),compiled_registry_packages=len(compiled),actual_RM_children=6)

    # 实际observer停止异常路径；失败后仍必须删除精确自有容器，不能只检查正向清理。
    v=subprocess.run([sys.executable,str(ROOT/'tools/formal-undo-fence/live.py'),'--fault','drop_ok','--cleanup-control'],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=240)
    put('cleanup-control/actual-command.log',v.stdout);need(v.returncode==1 and b'Observer cleanup failed after owned container cleanup' in v.stdout,'Observer stop failure path was not exercised')
    match=re.search(rb'MYSQL_UNDO_FENCE_RUN (.+)',v.stdout);need(match is not None,'Cleanup control run missing');child=Path(match[1].decode().strip());need(child.is_relative_to(ROOT/'docs/phase-2/evidence/mysql-undo-fence'),'Cleanup control escapes evidence root')
    control_cleanup=read(child/'cleanup.json');need(control_cleanup['owned_container_removed'] is True and control_cleanup['observer_error']['message']=='injected observer stop failure after actual stop','Observer exception skipped owned cleanup')
    for name in ['cleanup.json','owned-runtime.json','mysql-wire.json','live-tests.log','observer-cleanup-error.json','remove.log']:
        put('cleanup-control/'+name,(child/name).read_bytes())
    cleanup.append(read(child/'owned-runtime.json'))
    audits=[]
    for runtime in cleanup:
        v=subprocess.run(['docker','inspect',runtime['container_id']],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30)
        need(v.returncode!=0 and b'no such object' in v.stdout.lower(),'Owned container still exists or absence not established');audits.append(dict(container=runtime['container_id'],verified_absent=True))
    save('owned-runtime-final-audit.json',audits)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final candidate source drift')
    save('result.json',dict(status='SELECTED_DURABLE_UNDO_FENCE_VERIFIED_FULL_WP05_OPEN',live=live,ordinary=ordinary,inherited_store_authority=inherited,red=red,snapshot_red=snapshot_red,raw_wire_controls=len(wire_controls),packet_role_controls=len(controls),owned_DB_containers=len(cleanup),observer_failure_cleanup_control=True,fresh_DB_cases_per_toolchain=24,fresh_fence_cases_per_toolchain=13,fresh_store_cases_per_toolchain=11,original_Java_guard_comparisons_per_toolchain=2,original_Java_store_comparisons_per_toolchain=4,inherited_other_DB_cases_per_toolchain=39,inherited_other_DB_cases_fresh=False,registry_archive_candidates=185,actual_compiled_registry_packages=150,existing_source_changes=sorted(changes),public_business_COMMIT_available=False,full_original_undo_migrated=False,SQL_Undo_commit_atomicity_verified=False,TC_registration_gate_passed=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),source_inputs=len(frozen))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
