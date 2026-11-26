"""Qualify original lock-key format/order and collection; actual TC coverage and atomic Undo remain open."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
import uuid

ROOT=Path(__file__).resolve().parents[2]
BASE=ROOT/'docs/phase-2/evidence/undo-codec-refinement/run_20261115_235850_b36442a0'
JAVA=ROOT.parent/'seata-java-phase09-oracle'
LOCK=ROOT/'docs/phase-2/evidence/lock-key-peer/run_20261116_004741_eaf663fb'
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
MSRV=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/lock-key-gate'/('run_'+stamp)
    work=Path('D:/codex-builds')/('seata_lock_key_gate_'+stamp)
    out.mkdir(parents=True);work.mkdir();print('LOCK_KEY_GATE_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1')
    for name in ['RUSTC','RUSTDOC']:env.pop(name,None)
    def put(name,data):
        path=out/name;path.parent.mkdir(parents=True,exist_ok=True)
        with path.open('xb') as file:file.write(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name,cwd=ROOT,extra=None,expected=0):
        value=subprocess.run([str(x) for x in args],cwd=cwd,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,value.stdout);need(value.returncode==expected,name+' unexpected exit '+str(value.returncode));return value.stdout
    def packet(path):
        members=read(path/'catalog.json')['members']
        for row in members:need(sha(path/row['path'])==row['sha256'],'Archive member drift')
        return dict(path=str(path),catalog_sha256=sha(path/'catalog.json'),members=len(members))
    base_binding=packet(BASE)
    previous={Path(row['path']):row for row in read(BASE/'source-inputs.json')}
    for path,row in previous.items():
        if 'registry' in path.parts:need(sha(path)==row['sha256'],'Registry source/archive drift')
    old=tomllib.loads((BASE/previous[ROOT/'Cargo.lock']['snapshot']).read_text('utf-8'))
    current=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))
    registry=lambda data:[x for x in data['package'] if x.get('source','').startswith('registry+')]
    need(registry(old)==registry(current),'Registry versions/checksums/edges changed')
    active=set(map(tuple,read(BASE/'1.99/compiled-registry-packages.json')))
    paths={ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/global-lock-key-contract.md',ROOT/'docs/phase-2/lock-key-fixture-ledger.json'}
    for folder in ['crates','tools/formal-lock-key','tools/formal-undo-format','tools/formal-mysql-foundation','tests/fixtures/locks','tests/fixtures/undo','tests/fixtures/protocol/v1']:
        paths.update(path for path in (ROOT/folder).rglob('*') if path.is_file())
    paths.update(path for path in (ROOT/'docs/phase-2').glob('*contract.md'))
    paths.update([JAVA/'script/client/at/db/mysql.sql',LOCK/'catalog.json',LOCK/'runtime.json'])
    paths.update(path for path in previous if 'registry' in path.parts)
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(snapshot,path.read_bytes());frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    source_map={Path(row['path']):row for row in frozen};save('source-inputs.json',frozen)
    control_plane=[];database=[]
    for path,row in source_map.items():
        if any(path.is_relative_to(ROOT/'crates'/name/'src') for name in ['seata_protocol','seata_transport','seata_tm','seata_rm','seata_datasource','seata_db_mysql']):
            need(path in previous and previous[path]['sha256']==row['sha256'],'Existing control-plane/database implementation changed')
            (database if path.is_relative_to(ROOT/'crates/seata_db_mysql') else control_plane).append(str(path.relative_to(ROOT)))
    save('scope.json',dict(base=base_binding,unchanged_existing_control_and_datasource_sources=control_plane,unchanged_DB_production=database,
        local_dependency_change='seata-db-mysql dev edge to already locked serde_json1.0.140 only',registry_entries_unchanged=True,
        old38_DB_scenes='inherited, not re-executed; only added lock-key methods consumed by extended native codec test'))
    for family,source in [('primary-order',Path('D:/codex-builds/seata-global-lock-order-red-20261116')),('profile',Path('D:/codex-builds/seata-lock-profile-red-20261116'))]:
        need(b'test result: FAILED. 0 passed; 1 failed;' in (source/'behavior-red.log').read_bytes(),'Missing actual RED')
        for path in source.rglob('*'):
            if path.is_file():put('tdd/'+family+'/'+path.relative_to(source).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(primary_order='Real target returned composite:2_1 rather than original composite:1_2',profile='Mixed profile append accepted; guard now fixes even empty configured context',
        API='Initial missing exports compiler failure only',diagnostics='Wrong test metadata domain/scale, missing oracle JsonUtil/Netty and shadowed test variable are setup failures, not production behavior RED'))
    for package in ['seata-at','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/doc.log',extra=dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-lock-key-doc'))
    command(['cargo','check','--locked','--offline','--workspace','--all-targets','--all-features'],'all-features.log')
    artifacts=[];restarts=[]
    for rust,cargo,extra in [('1.99','cargo',{}),('1.85',MSRV/'cargo.exe',dict(RUSTC=str(MSRV/'rustc.exe'),RUSTDOC=str(MSRV/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('LOCK_WORKSPACE '+rust,flush=True)
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/version.txt',extra=extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--lib','--tests','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra=extra)
        need(sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw)))==325,'Expected316 previous +9 lock tests')
        compiled=set();count=0
        for line in raw.splitlines():
            try:row=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if row.get('reason')=='compiler-artifact' and row.get('package_id','').startswith('registry+'):compiled.add(tuple(row['package_id'].rsplit('#',1)[1].split('@')))
            if row.get('reason')=='compiler-artifact' and row.get('profile',{}).get('test') and row.get('executable'):
                path=Path(row['executable']);snapshot='artifacts/'+rust+'/'+path.name;put(snapshot,path.read_bytes());artifacts.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path),rust=rust));count+=1
        need(count==48 and compiled==active,'Actual48 targets/150 registry closure differ');save(rust+'/compiled-registry.json',sorted(compiled))
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw);need(len(groups)==2,'RM recovery groups missing')
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Unexpected recovery path')
            for path in directory.rglob('*'):
                if path.is_file():put('process-restarts/'+rust+'/'+phase.decode()+'/'+path.relative_to(directory).as_posix(),path.read_bytes())
            restarts.append(dict(rust=rust,phase=phase.decode(),actual_children=3))
    save('test-artifacts.json',artifacts);save('process-restarts.json',restarts)
    def isolated(name):
        directory=work/name;directory.mkdir()
        for part in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/part,directory/part)
        for part in ['crates','tests/fixtures/undo','tests/fixtures/locks','tests/fixtures/protocol/v1','tools/formal-lock-key']:shutil.copytree(ROOT/part,directory/part)
        return directory
    controls=[]
    for name,file,before,after,target,test in [
        ('index-order','exec/base_transactional_executor.rs','JavaPrimaryKeyOrder::indices(&primary_names)?','(0..primary_names.len()).collect::<Vec<_>>()','java_lock_order_is_separate_from_database_index_and_physical_column_order','global_lock_key_order'),
        ('collision-tree','exec/java_primary_key_order.rs','if previous_length >= 8 {','if false {','original_java_order_corpus_covers_resize_tree_collisions_and_split_iteration','global_lock_key_order'),
        ('wrong-tiny-profile','exec/base_transactional_executor.rs','profile == MySqlJdbcProfile::Connector8Default','profile != MySqlJdbcProfile::Connector8Default','default_tiny_boolean_does_not_destroy_integer_two_and_unknown_native_is_rejected','global_lock_keys'),
        ('wrong-image','connection_context.rs','if log.sql_type() == SqlType::Delete','if log.sql_type() != SqlType::Delete','delete_uses_before_insert_and_update_use_after_and_full_chunks_are_deduplicated','global_lock_keys'),
        ('separator-budget','connection_context.rs','let separator = usize::from(!self.lock_keys.is_empty());','let separator = 0;','aggregate_budget_and_log_count_are_atomic_and_include_separators','global_lock_keys'),
        ('output-budget','exec/lock_key_output.rs','size > 1_048_576','false','single_key_budget_includes_table_prefix_and_expanded_signed_binary_bytes','global_lock_keys'),
        ('mixed-profile','connection_context.rs','if self.profile.is_some_and(|current| current != profile) {','if false {','one_transaction_cannot_mix_jdbc_profiles_even_when_the_current_key_bytes_match','global_lock_keys'),
        ('source-log-count','connection_context.rs','if self.undo_count >= 128 {','if self.undo_count > 128 {','aggregate_budget_and_log_count_are_atomic_and_include_separators','global_lock_keys')]:
        print('LOCK_CONTROL '+name,flush=True);directory=isolated(name);path=directory/'crates/seata_at/src'/file;text=path.read_text('utf-8');need(text.count(before)==1,'Control anchor differs '+name);path.write_text(text.replace(before,after,1),encoding='utf-8')
        raw=command(['cargo','test','--locked','--offline','-p','seata-at','--test',test,target,'--','--exact','--nocapture'],'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-lock-key-controls'),expected=101)
        need(b'test result: FAILED. 0 passed; 1 failed;' in raw and b'panicked at crates' in raw and b'error[E' not in raw,'Control did not fail actual target assertion')
        put('controls/'+name+'.rs',path.read_bytes());controls.append(dict(name=name,source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target=target,target_assertion_failed=True))
    save('controls.json',controls)
    children=[]
    for rust in ['1.99','1.85']:
        print('LOCK_REAL_MYSQL '+rust,flush=True)
        raw=command([sys.executable,ROOT/'tools/formal-lock-key/live.py','--rust',rust],'live-'+rust+'.log');match=re.search(rb'MYSQL_LOCK_KEYS_RUN ([^\r\n]+)',raw);need(match,'Native child missing');child=Path(match[1].decode())
        result=read(child/'result.json');need(result['succeeded'] and result['actual_live_tests']==1 and not result['cleanup_errors'],'Native key path did not pass')
        for row in read(child/'source-inputs.json'):
            parent=source_map.get(Path(row['path']));need(parent and parent['sha256']==row['sha256'],'Native source not parent-frozen: '+row['path'])
        need(all(row['logs']==9 and row['actual_prepared_codec_MATCH'] for row in read(child/'lock-peer/result.json')['profiles']),'Missing9 native lock or4 codec logs')
        children.append(packet(child)|dict(rust=rust,actual_cases=1,profiles=2,lock_logs_per_profile=9,codec_logs_per_profile=4))
    save('live-children.json',children)
    print('LOCK_REAL_ASSERTION_CONTROL',flush=True);directory=isolated('native-wrong-order')
    for part in ['tools/formal-undo-format','tools/formal-mysql-foundation']:shutil.copytree(ROOT/part,directory/part)
    (directory/'docs/phase-2').mkdir(parents=True)
    for path in (ROOT/'docs/phase-2').glob('*contract.md'):shutil.copyfile(path,directory/'docs/phase-2'/path.name)
    path=directory/'crates/seata_at/src/exec/base_transactional_executor.rs';text=path.read_text('utf-8');before='JavaPrimaryKeyOrder::indices(&primary_names)?';need(text.count(before)==1,'Native control anchor differs');path.write_text(text.replace(before,'(0..primary_names.len()).collect::<Vec<_>>()',1),encoding='utf-8')
    raw=command([sys.executable,directory/'tools/formal-lock-key/live.py'],'native-control/wrapper.log',cwd=directory,extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-lock-key-native-controls',SEATA_JAVA_ORACLE_ROOT=str(JAVA),SEATA_UNDO_ORACLE_BUILD=str(BUILD),SEATA_LOCK_ORACLE_BUILD=str(LOCK)),expected=1)
    match=re.search(rb'MYSQL_LOCK_KEYS_RUN ([^\r\n]+)',raw);need(match,'Native negative child missing');child=Path(match[1].decode());result=read(child/'result.json');raw=(child/'live-tests.log').read_bytes()
    need(not result['succeeded'] and result['failed_live_tests']==1 and not result['cleanup_errors'] and b'lock_pk_reverse:2_1' in raw and b'assertion `left == right` failed' in raw and b'error[E' not in raw,'Native wrong-order control did not fail targeted assertion')
    changed=0
    for row in read(child/'source-inputs.json'):
        source=Path(row['path']);need(sha(source)==row['sha256']==sha(child/row['snapshot']),'Negative source drift')
        parent=source_map.get(ROOT/source.relative_to(directory) if source.is_relative_to(directory) else source);need(parent,'Unbound negative input')
        if source==path:need(parent['sha256']!=row['sha256'],'No declared mutation');changed+=1
        else:need(parent['sha256']==row['sha256'],'Undeclared native mutation')
    need(changed==1,'Expected only one native mutation')
    for source in child.rglob('*'):
        if source.is_file():put('native-control/child/'+source.relative_to(child).as_posix(),source.read_bytes())
    save('native-control/result.json',dict(actual_target_assertion_failed=True,only_declared_file_changed=True,owned_container_cleaned=True))
    peers=[]
    for tool,marker,expected in [('tools/formal-lock-key/peer.py',b'LOCK_KEY_PEER_RUN ',1029),('tools/formal-undo-format/document_peer.py',b'UNDO_DOCUMENT_PEER_RUN ',40)]:
        print('LOCK_LINKED_PEER '+tool,flush=True);raw=command([sys.executable,ROOT/tool],Path(tool).parent.name+'-peer.log');match=re.search(marker+rb'([^\r\n]+)',raw);need(match,'Peer missing');peer=Path(match[1].decode());result=read(peer/'result.json')
        need(result.get('Java_Rust_MATCH',result.get('second_Java_runtime_MATCH'))==expected,'Peer comparison count differs')
        for row in read(peer/'source-inputs.json'):
            parent=source_map.get(Path(row['path']))
            if parent:need(parent['sha256']==row['sha256'],'Actual peer source differs from parent')
            else:need(Path(row['path']).is_relative_to(JAVA),'Unbound peer source')
        peers.append(packet(peer)|dict(actual_cases=expected))
    save('peers.json',peers)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Gate input drift')
    for row in artifacts:need(sha(Path(row['path']))==row['sha256'],'Executed workspace artifact drift')
    save('result.json',dict(status='SELECTED_LOCK_KEY_COMPONENT_VERIFIED_PENDING_REVIEW',workspace_tests_per_toolchain=325,new_lock_tests=9,actual_test_artifacts=len(artifacts),
        actual_RM_restart_children=12,pure_assertion_controls=len(controls),actual_native_assertion_controls=1,actual_owned_MySQL_containers=3,
        native_cases_per_toolchain=1,native_profiles_per_toolchain=2,actual_native_lock_logs_per_profile=9,actual_native_codec_logs_per_profile=4,
        original_lock_key_runtime_MATCH=1029,document_runtime_MATCH=40,document_second_runtime_MATCH=40,
        inherited_other_MySQL_cases_per_toolchain=38,registry_archives=185,actual_compiled_registry_packages=150,actual_dependency_sources=6871,source_inputs=len(frozen),
        original_JUnit_tests_executed=False,Java_source_case_parity_complete=False,original_object_states='BaseTransactionalExecutor/ConnectionContext PARTIAL; order/output/errors Rust extensions',
        actual_TC_lock_coverage_verified=False,TC_registration_verified=False,SQL_Undo_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False,
        full_V1_complete=False,release_authorized=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=path.relative_to(out).as_posix(),sha256=sha(path)) for path in sorted(out.rglob('*')) if path.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)

if __name__=='__main__':run()
