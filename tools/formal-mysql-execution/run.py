"""Freeze and verify bounded native DML, full images and memory-only Undo preparation; no commit/atomic Undo claim."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[2]
JAVA = ROOT.parent / 'seata-java-phase09-oracle'
CACHE = Path('D:/codex-builds/seata-rust-cargo-home')
MSRV = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(value, message):
    if not value:
        raise ValueError(message)


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-execution-gate' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_mysql_execution_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('MYSQL_EXECUTION_GATE_RUN ' + str(out), flush=True)
    env = os.environ | dict(CARGO_HOME=str(CACHE), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')
    env.pop('RUSTC', None)
    env.pop('RUSTDOC', None)

    def put(name, data):
        p = out / name
        p.parent.mkdir(parents=True, exist_ok=True)
        with p.open('xb') as f:
            f.write(data.encode('utf-8') if isinstance(data, str) else data)

    def save(name, value):
        put(name, json.dumps(value, ensure_ascii=False, indent=2) + '\n')

    def command(args, name, cwd=ROOT, extra=None, negative=False, failure_message=b'assertion', failure_exit=None):
        result = subprocess.run([str(x) for x in args], cwd=cwd, env=env | (extra or {}),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW, timeout=600)
        put(name, result.stdout)
        if negative:
            need(result.returncode == 101 and b'test result: FAILED. 0 passed; 1 failed;' in result.stdout
                and failure_message in result.stdout and b'panicked at crates' in result.stdout
                and b'error[E' not in result.stdout, 'Control must compile and fail its target assertion')
        elif failure_exit is not None:
            need(result.returncode == failure_exit, name + ' unexpected control exit')
        else:
            need(result.returncode == 0, name + ' failed')
        return result.stdout

    spec = importlib.util.spec_from_file_location('mysql_source_inventory', ROOT / 'tools/formal-mysql-foundation/source_inventory.py')
    inventory_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(inventory_module)
    inventory, java_inputs = inventory_module.inventory(ROOT)
    for name in ['Field', 'Row', 'TableRecords']:
        source = 'rm-datasource/src/main/java/org/apache/seata/rm/datasource/sql/struct/' + name + '.java'
        target = {'Field': 'field', 'Row': 'row', 'TableRecords': 'table_records'}[name]
        java_inputs.add(JAVA / source)
        inventory['objects'].append(dict(source=source, target='crates/seata_datasource/src/struct/' + target + '.rs',
            source_sha256=sha(JAVA / source), state='PARTIAL', reason='Bounded exact value model only; JDBC reader and original serialization/differential not implemented'))
    inventory['Rust_extensions'].extend(['DecimalValue canonical exact text', 'FieldValue exhaustive value domain', 'ImageError fixed redacted validation', 'ColumnDomain physical ranges and CharacterSet', 'MySqlImageReader native MySQL preflight/locked read'])
    inventory['objects'][-1]['reason']='Bounded MySQL row model/read path only; full multi-JDBC and original serialization/source-case parity incomplete'
    inventory['method_references']=[]
    for name in ['UpdateExecutor','DeleteExecutor']:
        source='rm-datasource/src/main/java/org/apache/seata/rm/datasource/exec/'+name+'.java'
        java_inputs.add(JAVA/source)
        inventory['method_references'].append(dict(source=source,sha256=sha(JAVA/source),method='beforeImage',state='PARTIAL'))
    for name,java_sub,rust_path in [
        ('SQLUndoLog','undo','crates/seata_at/src/undo/sql_undo_log.rs'),
        ('StatementCallback','exec','crates/seata_db_mysql/src/exec/statement_callback.rs'),
        ('UpdateExecutor','exec','crates/seata_db_mysql/src/exec/update_executor.rs'),
        ('DeleteExecutor','exec','crates/seata_db_mysql/src/exec/delete_executor.rs'),
        ('MySQLInsertExecutor','exec/mysql','crates/seata_db_mysql/src/mysql/mysql_insert_executor.rs')]:
        source='rm-datasource/src/main/java/org/apache/seata/rm/datasource/'+java_sub+'/'+name+'.java'
        java_inputs.add(JAVA/source)
        inventory['objects'].append(dict(source=source,target=rust_path,source_sha256=sha(JAVA/source),state='PARTIAL',
            reason='Bounded MySQL DML/images or memory DTO only; Java API/serialization/source-case differential incomplete'))
    inventory['Rust_extensions'].append('Native typed temporary INSERT key staging; CREATE TEMPORARY privilege profile')
    save('source-inventory.json', inventory)
    need(command(['git', '-C', JAVA, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip() == inventory['java_pin'], 'Java pin differs')
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    raw_tree=command(['cargo','tree','--locked','--offline','--workspace','--target','x86_64-pc-windows-msvc','-e','normal,build,dev','--prefix','none','--format','{p}','--no-dedupe'],'actual-tree.txt')
    tree_packages={tuple(line.split()[:2]) for line in raw_tree.decode().splitlines() if len(line.split())>1}
    active={(name,version[1:]) for name,version in tree_packages if not name.startswith('seata-')}
    need(len(active)==148,'Expected actual148 registry packages from cargo tree; metadata weak optional graph is not the compiled closure')
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__), ROOT / 'tools/formal-mysql-foundation/source_inventory.py',
        ROOT / 'docs/phase-2/mysql-execution-images-contract.md', ROOT / 'docs/phase-2/row-image-contract.md', ROOT / 'docs/phase-2/mysql-foundation-contract.md', ROOT / 'docs/phase-2/mysql-row-read-contract.md', ROOT / 'tools/formal-mysql-foundation/live.py', ROOT / 'tools/formal-mysql-foundation/schema.sql',
        ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md'} | java_inputs
    paths.update(p for p in (ROOT / 'crates').rglob('*') if p.is_file())
    paths.update(p for p in (ROOT / 'tests/fixtures/protocol/v1').rglob('*') if p.is_file())
    dependencies = []
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        name = package['name'] + '-' + package['version']
        archive = CACHE / 'registry/cache/index.crates.io-1949cf8c6b5b557f' / (name + '.crate')
        if not archive.exists():
            cached=Path('C:/Users/hnxyh/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f')/archive.name
            need(cached.exists() and sha(cached)==package['checksum'],'Locked public cached archive missing/different')
            shutil.copyfile(cached,archive)
        need(sha(archive) == package['checksum'], 'Archive checksum differs: ' + name)
        used = (package['name'], package['version']) in active
        count = 0
        with tarfile.open(archive, 'r:gz') as bundle:
            for member in bundle.getmembers():
                if used and member.isfile():
                    relative = Path(member.name).relative_to(name)
                    need('..' not in relative.parts, 'Unsafe archive member')
                    path = CACHE / 'registry/src/index.crates.io-1949cf8c6b5b557f' / name / relative
                    with bundle.extractfile(member) as f:
                        need(path.read_bytes() == f.read(), 'Actual dependency source differs: ' + str(path))
                    paths.add(path)
                    count += 1
        paths.add(archive)
        dependencies.append(dict(name=package['name'], version=package['version'], checksum=package['checksum'], active=used, actual_sources=count))
    save('dependencies.json', dependencies)
    frozen = []
    for path in sorted(paths):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save('source-inputs.json', frozen)
    for label,directory in [('insert-template-api','D:/codex-builds/seata-insert-template-api-red-20261115'),('undo-image-api','D:/codex-builds/seata-sql-undo-image-api-red-20261115')]:
        base=Path(directory);need(base.is_dir(),'Missing actual API RED archive')
        for path in base.rglob('*'):
            if path.is_file():put('api-red/'+label+'/'+path.relative_to(base).as_posix(),path.read_bytes())
    for label,directory in [('literal-assignment-red','run_20261115_203258_04949f0a'),('undo-count-red','run_20261115_203543_77297304'),
        ('bound-text-positive-not-red','run_20261115_203008_0de1cc62'),('test-log-type-error-not-product-red','run_20261115_205255_d201f8d9')]:
        base=ROOT/'docs/phase-2/evidence/mysql-execution'/directory
        need((base/'result.json').is_file(),'Missing actual TDD diagnostic')
        for path in base.rglob('*'):
            if path.is_file():put('tdd/'+label+'/'+path.relative_to(base).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(API_red='Compile failures for missing template/Undo API, not behavior RED',
        literal_assignment='Actual CAST implementation captured wrong integer primary image for original literal 1.6; target failed',
        bound_text='Prepared bound text 1.6 passed old CAST; explicitly NOT the defect reproducer',
        undo_count='129th execution returned budget but incorrectly allowed metadata access; actual assertion RED',
        test_log_type_error='General log argument is BLOB; new namespace test decoded String directly,16passed1failed; test fixture defect only',
        historical_failed_count='Old live helper actual_live_tests=0 on failed runs meant no successful terminal count; raw test logs retain actual executed cases'))
    for package in ['seata-at','seata-datasource','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/rustdoc.log',extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-execution-doc',RUSTDOCFLAGS='-D warnings'))
    artifacts, restarts = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE_START ' + label, flush=True)
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.txt', extra=extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 291, 'Expected282 existing plus3 INSERT template and6 Undo shape tests')
        count = 0
        compiled=set()
        for line in raw.splitlines():
            try:
                row = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            if row.get('reason')=='compiler-artifact' and row.get('package_id','').startswith('registry+'):
                compiled.add(tuple(row['package_id'].rsplit('#',1)[1].split('@')))
            if row.get('reason') == 'compiler-artifact' and row.get('profile', {}).get('test') and row.get('executable'):
                path = Path(row['executable'])
                snapshot = 'artifacts/' + label + '/' + path.name
                put(snapshot, path.read_bytes())
                artifacts.append(dict(path=str(path), snapshot=snapshot, sha256=sha(path), rust=label, target=row['target']['name']))
                count += 1
        need(compiled==active,'Actual compiler-artifact packages differ from cargo tree closure')
        save(label+'/compiled-registry-packages.json',sorted(compiled))
        need(count == 40, 'Expected40 actual workspace test targets, including4 ignored live suites')
        groups = re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true', raw)
        need(len(groups) == 2, 'Actual RM recovery groups missing')
        for directory, phase in groups:
            directory = Path(directory.decode())
            need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()), 'Unexpected RM evidence path')
            for path in directory.rglob('*'):
                if path.is_file():
                    put('process-restarts/' + label + '/' + phase.decode() + '/' + path.relative_to(directory).as_posix(), path.read_bytes())
            restarts.append(dict(rust=label, phase=phase.decode(), actual_children=3))
    save('test-artifacts.json', artifacts)
    save('process-restarts.json', restarts)
    def isolated(name):
        directory=work/name;directory.mkdir()
        for item in ['Cargo.toml','Cargo.lock']:shutil.copy2(ROOT/item,directory/item)
        shutil.copytree(ROOT/'crates',directory/'crates')
        return directory
    pure=[]
    for name,file,before,after,suite,target in [
        ('template-offset','mysql/mysql_insert_recognizer.rs','let start = parameter_count;','let start = 0;','insert_value_templates','templates_keep_original_row_column_and_binding_positions'),
        ('undo-schema','undo/sql_undo_log.rs','if before_image.table_meta() != after_image.table_meta()','if false','sql_undo_image_contract','update_identity_order_and_schema_cannot_change'),
        ('undo-identity','undo/sql_undo_log.rs','SqlType::Update => before_keys == after_keys','SqlType::Update => true','sql_undo_image_contract','update_identity_order_and_schema_cannot_change'),
        ('undo-direction','undo/sql_undo_log.rs','SqlType::Insert => before_keys.is_empty() && !after_keys.is_empty()','SqlType::Insert => true','sql_undo_image_contract','insert_and_delete_require_the_correct_empty_side'),
        ('undo-duplicate','undo/sql_undo_log.rs','if !seen.insert(key.clone())','if false && !seen.insert(key.clone())','sql_undo_image_contract','duplicate_raw_primary_identities_are_rejected')]:
        print('ASSERTION_CONTROL '+name,flush=True)
        directory=isolated(name);path=directory/'crates/seata_at/src'/file
        text=path.read_text('utf-8');need(text.count(before)==1,'Control anchor '+name)
        path.write_text(text.replace(before,after,1),encoding='utf-8')
        command(['cargo','test','--locked','--offline','-p','seata-at','--test',suite,target,'--','--exact','--nocapture'],
            'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-execution-controls'),negative=True)
        put('controls/'+name+'.rs',path.read_bytes())
        pure.append(dict(name=name,target=target,executed_source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target_assertion_failed=True))
    save('controls.json',pure)
    children=[]
    for rust in ['1.99','1.85']:
        for suite,expected in [('metadata',9),('row_images',10),('image_race',1),('execution',17)]:
            print('REAL_MYSQL '+rust+' '+suite,flush=True)
            raw=command([sys.executable,ROOT/'tools/formal-mysql-foundation/live.py','--rust',rust,'--suite',suite],'live-'+rust+'-'+suite+'.log')
            match=re.search(rb'MYSQL_(?:METADATA|ROW_IMAGES|IMAGE_RACE|EXECUTION)_RUN ([^\r\n]+)',raw);need(match is not None,'Child path missing')
            child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes())
            need(result['succeeded'] and result['actual_live_tests']==expected and not result['cleanup_errors'],'Required actual cases missing')
            catalog=json.loads((child/'catalog.json').read_bytes())['members']
            for row in catalog:need(sha(child/row['path'])==row['sha256'],'Child archived member differs')
            children.append(dict(path=str(child),rust=rust,suite=suite,actual_live_tests=expected,members=len(catalog),catalog_sha256=sha(child/'catalog.json')))
    save('live-children.json',children)
    live_controls=[]
    for name,target in [
        ('literal-cast','decimal_text_primary_assignment_cannot_capture_an_adjacent_old_row'),
        ('undo-count-poison','pending_undo_record_limit_poisons_and_forces_full_rollback'),
        ('undo-payload','execution_failures::cumulative_undo_payload_limit_poisons_and_rolls_back_all_statements'),
        ('after-original-where','update_after_image_uses_original_keys_when_the_where_column_changes_then_delete_is_empty'),
        ('execute-transaction-owner','execution_failures::cancellation_after_key_assignment_cleans_temp_and_rolls_back_prior_business'),
        ('temporary-shadow','execution_failures::business_table_matching_next_internal_name_is_not_shadowed_or_dropped')]:
        print('REAL_MYSQL_ASSERTION_CONTROL '+name,flush=True)
        directory=isolated('live-'+name)
        (directory/'tools/formal-mysql-foundation').mkdir(parents=True)
        for item in ['live.py','schema.sql']:shutil.copy2(ROOT/'tools/formal-mysql-foundation'/item,directory/'tools/formal-mysql-foundation'/item)
        (directory/'docs/phase-2').mkdir(parents=True)
        for item in ['mysql-foundation-contract.md','mysql-row-read-contract.md','mysql-execution-images-contract.md']:
            shutil.copy2(ROOT/'docs/phase-2'/item,directory/'docs/phase-2'/item)
        filename={'literal-cast':'mysql/mysql_insert_executor.rs','after-original-where':'exec/update_executor.rs','temporary-shadow':'mysql/mysql_insert_executor.rs'}.get(name,'connection_proxy.rs')
        path=directory/'crates/seata_db_mysql/src'/filename;text=path.read_text('utf-8')
        if name=='literal-cast':
            base=ROOT/'docs/phase-2/evidence/mysql-execution/run_20261115_203258_04949f0a'
            row=next(x for x in json.loads((base/'source-inputs.json').read_bytes()) if x['path'].replace(chr(92),'/').endswith('/mysql/mysql_insert_executor.rs'))
            text=(base/row['snapshot']).read_text('utf-8')
        elif name=='undo-count-poison':
            anchor='if self.undo_logs.len() >= 128 {\n            self.poisoned = true;';need(text.count(anchor)==1,'Count anchor')
            text=text.replace(anchor,'if self.undo_logs.len() >= 128 {',1)
        elif name=='undo-payload':
            anchor='if bytes > 16_777_216';need(text.count(anchor)==1,'Payload anchor');text=text.replace(anchor,'if false',1)
        elif name=='after-original-where':
            anchor='MySqlImageReader::read_keys(tx, before.table_meta(), &keys).await?';need(text.count(anchor)==1,'After WHERE anchor')
            text=text.replace(anchor,'MySqlImageReader::read(tx, database, sql, parameters).await?',1)
        elif name=='temporary-shadow':
            anchor='if !name.eq_ignore_ascii_case(table.name())';need(text.count(anchor)==1,'Temporary name anchor');text=text.replace(anchor,'if true',1)
        else:
            a=text.index('    pub async fn execute(');b=text.index('    /// 无参数；只读查看事务内准备数据',a);region=text[a:b]
            need(region.count('let mut transaction = self.transaction.take()')==1,'Owner anchor')
            region=region.replace('let mut transaction = self.transaction.take()','let mut transaction = self.transaction.as_mut()').replace('                self.transaction = Some(transaction);','').replace('                drop(transaction);','')
            text=text[:a]+region+text[b:]
        path.write_text(text,encoding='utf-8')
        raw=command([sys.executable,directory/'tools/formal-mysql-foundation/live.py','--suite','execution','--case',target],
            'live-controls/'+name+'/wrapper.log',cwd=directory,extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-mysql-execution-live-controls'),failure_exit=1)
        match=re.search(rb'MYSQL_EXECUTION_RUN ([^\r\n]+)',raw);need(match is not None,'Negative child missing')
        child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes());log=(child/'live-tests.log').read_bytes()
        need(not result['succeeded'] and result['failed_live_tests']==1 and not result['cleanup_errors'] and b'test result: FAILED. 0 passed; 1 failed;' in log
            and b'panicked at crates' in log and b'error[E' not in log,'Real control must compile and fail its selected target')
        if name=='literal-cast':need(b'adjacent_old_row_captured=true' in log,'Original wrong old-row capture not reproduced')
        for row in json.loads((child/'source-inputs.json').read_bytes()):
            need(sha(Path(row['path']))==row['sha256'] and sha(child/row['snapshot'])==row['sha256'],'Executed negative source drift')
        for entry in child.rglob('*'):
            if entry.is_file():put('live-controls/'+name+'/child/'+entry.relative_to(child).as_posix(),entry.read_bytes())
        put('live-controls/'+name+'/executed.rs',path.read_bytes())
        live_controls.append(dict(name=name,target=target,actual_target_failed=True,executed_source=str(path),snapshot='live-controls/'+name+'/executed.rs',sha256=sha(path),actual_child=str(child),owned_container_cleaned=True))
    save('live-controls.json',live_controls)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source/dependency drift')
    for row in artifacts:need(sha(Path(row['path']))==row['sha256'],'Actual test artifact drift')
    save('result.json',dict(status='verified_bounded_execution_images_component_pending_review',workspace_tests_per_toolchain=291,
        new_pure_tests_per_toolchain=9,Rust_versions=['1.99','1.85'],actual_test_artifacts=len(artifacts),pure_assertion_controls=len(pure),
        real_MySQL_assertion_controls=len(live_controls),actual_RM_restart_children=12,source_inputs=len(frozen),registry_archives=len(dependencies),
        actual_compiled_registry_packages=len(active),actual_dependency_sources=sum(x['actual_sources'] for x in dependencies),
        metadata_resolve_candidate_registry_packages=sum(1 for x in metadata['packages'] if x.get('source') and x['id'] in {n['id'] for n in metadata['resolve']['nodes']}),
        actual_owned_MySQL_containers=len(children)+len(live_controls),actual_live_tests_per_toolchain=37,
        Java_objects_PARTIAL=14,Java_source_test_files_inventoried=189,Java_assets_copied=7,Java_concrete_case_parity_complete=False,
        bounded_INSERT_UPDATE_DELETE_execution_and_images=True,memory_only_SQL_Undo_preparation=True,
        temporary_privilege_profile='CREATE TEMPORARY TABLES required for INSERT; no automatic grants',
        SQL_Undo_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False,
        full_V1_complete=False,release_authorized=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
