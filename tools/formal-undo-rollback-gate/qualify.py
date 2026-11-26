"""Selected native Undo rollback qualification; exact live source/frames and scoped inherited baseline."""
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
PREVIOUS=ROOT/'docs/phase-2/evidence/undo-fence-gate/run_20261116_043811_c001f6c1'
sys.path.insert(0,str(ROOT/'tools/formal-undo-rollback-loss'))
from validate import validate,negative_controls
TARGET='uncertain_rollback_commit_keeps_original_identity_and_recovers_without_business_replay'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(value,message):
    if not value:raise ValueError(message)
def roles(rust,base,loss):
    need(base['verified'] is True and base['Rust']==rust and base['selected'] is None and base['fault'] is None and base['status']=='SELECTED_MYSQL_UNDO_ROLLBACK_OBSERVATIONS_VERIFIED' and any('test result: ok. 13 passed; 0 failed; 0 ignored;' in s for s in base['actual_test_summary']),'Native rollback role/version/count differs')
    need(len(loss)==3 and {r['fault'] for r in loss}=={'drop_ok','block_commit','delay_ok'} and all(r['verified'] is True and r['Rust']==rust and r['selected']==TARGET and r['status']=='SELECTED_MYSQL_UNDO_ROLLBACK_OBSERVATIONS_VERIFIED' and any('test result: ok. 1 passed; 0 failed; 0 ignored;' in s for s in r['actual_test_summary']) for r in loss),'Rollback fault role/version/set differs')
def run(manifest):
    manifest=manifest.resolve();values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both actual toolchains required')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/undo-rollback-gate'/('run_'+stamp);out.mkdir(parents=True);print('UNDO_ROLLBACK_GATE_RUN '+str(out),flush=True)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def packet(raw,digest=None):
        p=Path(raw);p=p if p.is_absolute() else ROOT/p;need(p.is_relative_to(ROOT/'docs/phase-2/evidence'),'Packet outside selected root')
        if digest:need(sha(p/'catalog.json')==digest,'Packet digest differs')
        rows=read(p/'catalog.json')['members'];need(len(rows)==len({r['path'] for r in rows}) and {r['path'] for r in rows}=={q.relative_to(p).as_posix() for q in p.rglob('*') if q.is_file() and q!=p/'catalog.json'},'Packet member set differs')
        for r in rows:need(sha(p/r['path'])==r['sha256'],'Packet member SHA differs')
        return dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=sha(p/'catalog.json'),members=len(rows),result=read(p/'result.json'))
    inherited=packet(PREVIOUS,'e679996e3feca7b1662c910fe6bada812992ad6eeacb87b03d15c52f9de43fdd');packet(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    before={Path(r['path']):r for r in read(PREVIOUS/'source-inputs.json')}
    for path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:need(sha(path)==before[path]['sha256'],'Dependency edges/versions changed')
    changes={p.relative_to(ROOT).as_posix() for p,r in before.items() if p.is_relative_to(ROOT/'crates') and '/src/' in p.as_posix() and sha(p)!=r['sha256']}
    expected={'crates/seata_at/src/undo/parser/jackson_undo_log_parser.rs','crates/seata_at/src/undo/parser/mod.rs','crates/seata_at/src/undo/parser/prepared_format.rs','crates/seata_db_mysql/src/data_source_proxy.rs','crates/seata_db_mysql/src/db_error.rs','crates/seata_db_mysql/src/lib.rs','crates/seata_db_mysql/src/mysql/mod.rs','crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs'}
    need(changes==expected,'Unexpected existing production source changes')
    for name,marker in {'crates/seata_db_mysql/src/data_source_proxy.rs':'    /// 参数为原XID/分支号','crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs':'    /// 参数为独立控制事务/schema/原身份'}.items():
        path=ROOT/name;previous=(PREVIOUS/before[path]['snapshot']).read_text(encoding='utf-8');current=path.read_text(encoding='utf-8');need(previous[previous.index(marker):]==current[current.index(marker):],'Previous fence/write/local transaction method bodies changed')
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-undo-rollback-contract.md',ROOT/'tools/formal-undo-format/runtime_binding.py',ROOT/'tools/formal-undo-fence/validate_wire.py'}
    for directory in [ROOT/'crates',ROOT/'tools/formal-undo-rollback',ROOT/'tools/formal-undo-rollback-loss',ROOT/'tools/formal-undo-rollback-java',HERE]:paths.update(p for p in directory.rglob('*') if p.is_file())
    for path,row in before.items():
        if '/registry/' in str(path).replace('\\','/'):
            need(sha(path)==row['sha256'],'Qualified dependency source/archive drift');paths.add(path)
    frozen=[]
    for path in sorted(paths):
        name='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(name,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=name))
    save('source-inputs.json',frozen);save('explicit-packets.json',values);live={};paths_seen=[];cleanup=[];wire_controls=[]
    for rust,profile in values['profiles'].items():
        native=packet(profile['native']);loss=[packet(p) for p in profile['loss']];roles(rust,native['result'],[p['result'] for p in loss])
        for proof in [native,*loss]:
            p=ROOT/proof['path'];paths_seen.append(str(p));need((p/'rust-version.txt').read_text(encoding='utf-8').startswith('rustc '+rust+'.'),'Actual recorded compiler differs');need(read(p/'cleanup.json')['owned_container_removed'] is True and read(p/'cleanup.json').get('observer_error') is None,'Owned live cleanup failed');cleanup.append(read(p/'owned-runtime.json'))
            for row in read(p/'source-inputs.json'):need(sha(Path(row['path']))==row['sha256'],'Current consumed input source drift')
        peer=native['result']['Java_peer'];java=packet(peer['run'],peer['catalog_sha256']);jr=java['result'];need(jr['status']=='SELECTED_ORIGINAL_UNDO_EXECUTOR_METHODS_MATCH' and jr['comparisons']==24 and jr['native_input_cases']==10 and jr['original_newly_compiled_sources']==7 and jr['actual_JDBC']=='8.0.27' and jr['actual_cleanup_negative_controls']==2 and jr['original_full_undo_invoked'] is False and jr['original_JUnit_executed'] is False,'Actual original source scope differs')
        observed=read(ROOT/java['path']/'observations.json');need(len(observed)==24 and all(r['status']=='MATCH' and r['other_branch_unchanged'] is True and (r['dirty_undo_exactly_retained'] is True if r['dirty_rejected'] else r['root_children_deleted'] is True) for r in observed) and {r['mode'] for r in observed}=={'matching_after','already_before','dirty'},'Actual source observation matrix differs')
        for proof in loss:
            p=ROOT/proof['path'];wire=read(p/'mysql-wire.json');observation=read(p/'observations/rollback-wire-observation.json');need(validate(wire,observation)==read(p/'wire-validation.json'),'Independent original wire validation differs');controls=negative_controls(wire,observation);need(len(controls)==10 and controls==read(p/'wire-negative-controls.json'),'Independent wire negative controls differ');wire_controls.extend(dict(Rust=rust,mode=wire['mode'],**c) for c in controls)
        live[rust]=dict(native=native,loss=loss,Java=java)
    need(len(paths_seen)==len(set(paths_seen))==8,'Live roles reused')
    red=packet(values['red'],'12d861ebfa73c444ac3deca6cd05709cb534f2b964c4d303b29e9d72efb225cc');r=red['result'];need(r['verified'] is True and r['Rust']=='1.99' and r['status']=='EXPECTED_MISSING_ROLLBACK_RED' and r['selected']=='stored_insert_undo_restores_business_and_consumes_original_row_atomically' and any('test result: FAILED. 0 passed; 1 failed;' in s for s in r['actual_test_summary']),'Original actual rollback RED differs')
    role_controls=[]
    def reject(name,call):
        try:call()
        except ValueError as error:role_controls.append(dict(control=name,rejection=str(error)))
        else:raise ValueError('Wrong actual role accepted: '+name)
    for rust,p in live.items():
        base=p['native']['result'];loss=[e['result'] for e in p['loss']]
        reject('wrong_toolchain_'+rust,lambda:roles('1.85' if rust=='1.99' else '1.99',base,loss));reject('duplicate_fault_'+rust,lambda:roles(rust,base,[loss[0],loss[0],loss[2]]));changed=copy.deepcopy(base);changed['selected']='another_single_case';reject('single_as_full_'+rust,lambda:roles(rust,changed,loss))
    save('wire-controls.json',wire_controls);save('role-controls.json',role_controls)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    def command(args,name,extra=None):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,v.stdout);need(v.returncode==0,name+' failed');return v.stdout
    command(['cargo','fmt','--all','--','--check'],'fmt.log');command(['cargo','clippy','--locked','--offline','--workspace','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','check','--locked','--offline','--workspace','--all-targets','--all-features'],'all-features.log')
    command(['cargo','doc','--locked','--offline','-p','seata-db-mysql','--no-deps'],'doc.log',dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-undo-rollback-doc'))
    ordinary={}
    for rust in ['1.99','1.85']:
        extra={};cargo='cargo'
        if rust=='1.85':
            tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';extra=dict(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/rust-version.txt',extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--all-targets','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra)
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(int(v) for v in counts);need(passed==333,'Expected327 lib/integration plus6 existing example tests differs')
        artifacts=[]
        for line in raw.splitlines():
            try:item=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(item,dict):artifacts.append(item)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')]
        compiled={r['package_id'].rsplit('#',1)[1] for r in artifacts if r.get('reason')=='compiler-artifact' and r['package_id'].startswith('registry+')}
        need(len(tested)==60 and len(counts)==len(tested),'Actual harness summaries differ from compiled test artifact count')
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
        ordinary[rust]=dict(passed=passed,existing_lib_integration_tests=327,existing_example_tests=6,test_artifacts=len(tested),compiled_registry_packages=len(compiled),actual_RM_children=6)

    audits=[]
    for runtime in cleanup:
        v=subprocess.run(['docker','inspect',runtime['container_id']],cwd=ROOT,env=env|dict(DOCKER_CONFIG='D:/Temp/seata-docker-config'),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30);need(v.returncode!=0 and b'no such object' in v.stdout.lower(),'Owned container not independently absent');audits.append(dict(container=runtime['container_id'],verified_absent=True))
    save('owned-runtime-final-audit.json',audits)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final source inputs drift')
    save('result.json',dict(status='SELECTED_NORMAL_UNDO_COMPENSATION_VERIFIED_FULL_WP05_WP06_OPEN',live=live,red=red,ordinary=ordinary,inherited_fence_component=inherited,fresh_native_DB_tests_per_toolchain=13,fresh_commit_fault_DB_tests_per_toolchain=3,fresh_DB_tests_per_toolchain=16,original_Java_observations_per_toolchain=24,original_native_input_cases_per_toolchain=10,original_source_files_newly_compiled=7,raw_wire_controls=len(wire_controls),role_controls=len(role_controls),actual_owned_DB_containers=len(cleanup),registry_archive_candidates=185,actual_compiled_registry_packages=150,existing_production_source_changes=sorted(changes),previous_database_tests_inherited_not_fresh=63,source_objects_state='PARTIAL',original_full_manager_or_JUnit_executed=False,historical_lossy_Java_tinyint_logs_reversibility_verified=False,selected_compensation_and_log_delete_atomicity_verified=True,all_old_attempts_permanently_fenced_by_Consumed=False,generic_RM_Applied_NotApplied_mapping_available=False,formal_resource_adapter_available=False,public_business_COMMIT_available=False,SQL_Undo_commit_atomicity_verified=False,TC_registration_gate_passed=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),source_inputs=len(frozen))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
