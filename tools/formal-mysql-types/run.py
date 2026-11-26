"""Verify native MySQL type identity with fresh execution regression and original Java format discovery; no codec/commit claim."""
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
    out = ROOT / 'docs/phase-2/evidence/mysql-type-gate' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_mysql_type_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('MYSQL_TYPE_GATE_RUN ' + str(out), flush=True)
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
    inventory['Rust_extensions'].append('MySqlType physical subtype/display-width identity, partial driver type-name mapping')
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
    paths.update(p for p in (ROOT/'tests/fixtures/undo').rglob('*') if p.is_file())
    paths.add(ROOT/'docs/phase-2/mysql-undo-format-contract.md')
    paths.add(JAVA/'script/client/at/db/mysql.sql')
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
    base=Path('D:/codex-builds/seata-mysql-type-api-red-20261115')
    need((base/'compile-red.log').is_file(),'Missing original API RED')
    for path in base.rglob('*'):
        if path.is_file():put('api-red/'+path.relative_to(base).as_posix(),path.read_bytes())
    base=ROOT/'docs/phase-2/evidence/mysql-type-identity/run_20261115_213759_0a86456e'
    need(b'native identity lost: id' in (base/'live-tests.log').read_bytes(),'Missing actual metadata identity RED')
    for path in base.rglob('*'):
        if path.is_file():put('tdd/actual-metadata-red/'+path.relative_to(base).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(API='Missing exported type/setter compiler RED only',
        actual_metadata='Real metadata returned None instead of Some(Int); one actual target failed',
        Java_diagnostics='Earlier format exploration had loose CP closure, invalid system variable and readiness on temporary init socket; not production Rust defects',
        final_Java='214333 fixed TCP readiness, actual profile query, complete runtime binding before/after, exact loaded origins and default/Empty class checks'))
    for package in ['seata-at','seata-datasource','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/rustdoc.log',extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-type-doc',RUSTDOCFLAGS='-D warnings'))
    artifacts, restarts = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE_START ' + label, flush=True)
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.txt', extra=extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 297, 'Expected291 previous plus6 native identity tests')
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
        need(count == 42, 'Expected42 actual workspace targets, including5 ignored live suites')
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
    controls=[]
    for name,file,before,after,target in [
        ('native-domain','mysql_type.rs','if valid {','if true {','integer_width_and_boolean_display_identity_remain_distinct'),
        ('post-domain-update','column_meta.rs','if let Some(native) = self.mysql_type','if let Some(native) = None::<MySqlType>','changing_domain_cannot_invalidate_existing_native_identity')]:
        print('ASSERTION_CONTROL '+name,flush=True)
        directory=isolated(name);path=directory/'crates/seata_datasource/src/struct'/file
        text=path.read_text('utf-8');need(text.count(before)==1,'Native control anchor differs')
        path.write_text(text.replace(before,after,1),encoding='utf-8')
        command(['cargo','test','--locked','--offline','-p','seata-datasource','--test','mysql_type_identity',target,'--','--exact','--nocapture'],
            'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-type-controls'),negative=True)
        put('controls/'+name+'.rs',path.read_bytes());controls.append(dict(name=name,target=target,executed_source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target_assertion_failed=True))
    save('controls.json',controls)
    children=[]
    for rust in ['1.99','1.85']:
        for suite,expected in [('metadata',9),('row_images',10),('image_race',1),('execution',17),('mysql_types',1)]:
            print('REAL_MYSQL '+rust+' '+suite,flush=True)
            raw=command([sys.executable,ROOT/'tools/formal-mysql-foundation/live.py','--rust',rust,'--suite',suite],'live-'+rust+'-'+suite+'.log')
            match=re.search(rb'MYSQL_(?:METADATA|ROW_IMAGES|IMAGE_RACE|EXECUTION|TYPE_IDENTITY)_RUN ([^\r\n]+)',raw);need(match is not None,'Child path missing')
            child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes())
            need(result['succeeded'] and result['actual_live_tests']==expected and not result['cleanup_errors'],'Required actual cases missing')
            catalog=json.loads((child/'catalog.json').read_bytes())['members']
            for row in catalog:need(sha(child/row['path'])==row['sha256'],'Child archived member differs')
            for row in json.loads((child/'source-inputs.json').read_bytes()):
                parent=next((x for x in frozen if Path(x['path'])==Path(row['path'])),None)
                need(parent and parent['sha256']==row['sha256'],'Live compiled source differs from parent frozen input')
            children.append(dict(path=str(child),rust=rust,suite=suite,actual_live_tests=expected,members=len(catalog),catalog_sha256=sha(child/'catalog.json')))
    save('live-children.json',children)
    live_controls=[]
    for name,before,after,diagnostic in [
        ('tiny-width','full_type\n                    .split_whitespace()\n                    .next()\n                    .is_some_and(|name| name.eq_ignore_ascii_case("tinyint(1)"))','false',b'native identity lost: t1'),
        ('text-subtype','"tinytext" => MySqlType::TinyText','"tinytext" => MySqlType::Text',b'native identity lost: tt')]:
        print('REAL_MYSQL_ASSERTION_CONTROL '+name,flush=True)
        directory=isolated('live-'+name)
        (directory/'tools/formal-mysql-foundation').mkdir(parents=True)
        for item in ['live.py','schema.sql']:shutil.copy2(ROOT/'tools/formal-mysql-foundation'/item,directory/'tools/formal-mysql-foundation'/item)
        (directory/'docs/phase-2').mkdir(parents=True)
        for item in ['mysql-foundation-contract.md','mysql-row-read-contract.md','mysql-execution-images-contract.md']:
            shutil.copy2(ROOT/'docs/phase-2'/item,directory/'docs/phase-2'/item)
        path=directory/'crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs';text=path.read_text('utf-8')
        need(text.count(before)==1,'Live native anchor differs '+name);path.write_text(text.replace(before,after,1),encoding='utf-8')
        raw=command([sys.executable,directory/'tools/formal-mysql-foundation/live.py','--suite','mysql_types','--case','physical_types_keep_all_native_subtypes_and_tinyint_display_width'],
            'live-controls/'+name+'/wrapper.log',cwd=directory,extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-mysql-type-live-controls'),failure_exit=1)
        match=re.search(rb'MYSQL_TYPE_IDENTITY_RUN ([^\r\n]+)',raw);need(match is not None,'Negative child missing')
        child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes());log=(child/'live-tests.log').read_bytes()
        need(not result['succeeded'] and result['failed_live_tests']==1 and not result['cleanup_errors'] and b'test result: FAILED. 0 passed; 1 failed;' in log
            and diagnostic in log and b'assertion `left == right` failed' in log and b'error[E' not in log,'Native control must fail exact target diagnostic')
        mutated=0
        for row in json.loads((child/'source-inputs.json').read_bytes()):
            source=Path(row['path']);need(sha(source)==row['sha256'] and sha(child/row['snapshot'])==row['sha256'],'Actual negative source drift')
            mapped=ROOT/source.relative_to(directory) if source.is_relative_to(directory) else source
            original=next((x for x in frozen if Path(x['path'])==mapped),None);need(original,'Unbound native negative input')
            if source==path:need(original['sha256']!=row['sha256'],'Missing declared mutation');mutated+=1
            else:need(original['sha256']==row['sha256'],'Undeclared native mutation')
        need(mutated==1,'Expected one mutated file')
        for entry in child.rglob('*'):
            if entry.is_file():put('live-controls/'+name+'/child/'+entry.relative_to(child).as_posix(),entry.read_bytes())
        put('live-controls/'+name+'/executed.rs',path.read_bytes())
        live_controls.append(dict(name=name,executed_source=str(path),snapshot='live-controls/'+name+'/executed.rs',sha256=sha(path),expected_diagnostic=diagnostic.decode(),actual_target_assertion_failed=True))
    save('live-controls.json',live_controls)
    java_live=ROOT/'docs/phase-2/evidence/undo-java-live/run_20261115_214333_d51d4e58'
    java_build=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
    java_links=[]
    for family,directory in [('build',java_build),('live',java_live)]:
        for row in json.loads((directory/'catalog.json').read_bytes())['members']:need(sha(directory/row['path'])==row['sha256'],'Java original packet differs')
        java_links.append(dict(kind=family,path=str(directory),catalog_sha256=sha(directory/'catalog.json')))
    result=json.loads((java_live/'result.json').read_bytes())
    need([x['succeeded'] for x in result['profiles']]==[False,False,True,True] and not result['cleanup_errors'],'Expected original profile outcomes')
    golden=[]
    for profile in ['default','numeric_tinyint']:
        base=ROOT/'tests/fixtures/undo/java_8_0_27'/profile
        for target in sorted(base.iterdir()):
            source=java_live/('8.0.27-'+profile)/target.name
            need(target.read_bytes()==source.read_bytes(),'Generated golden asset differs')
            golden.append(dict(source=str(source),target=str(target.relative_to(ROOT)),sha256=sha(target),kind='Java_generated_golden_NOT_source_test_parity'))
    for profile in ['5.1.42-default','5.1.42-numeric_tinyint']:
        need(b"Unknown system variable 'query_cache_size'" in (java_live/profile/'stdout.log').read_bytes(),'Legacy failure reason differs')
    save('java-oracle-bindings.json',dict(packets=java_links,goldens=golden,successful_original_table_cases=12,
        failed_legacy_connection_profiles=2,full_original_Maven_environment_recreated=False,
        original_source_case_differential=False,Rust_codec_interop=False))
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source/dependency drift')
    for row in artifacts:need(sha(Path(row['path']))==row['sha256'],'Actual workspace artifact drift')
    save('result.json',dict(status='verified_native_type_identity_pending_review',workspace_tests_per_toolchain=297,new_pure_tests_per_toolchain=6,
        Rust_versions=['1.99','1.85'],actual_test_artifacts=len(artifacts),pure_assertion_controls=len(controls),real_MySQL_assertion_controls=len(live_controls),
        actual_RM_restart_children=12,source_inputs=len(frozen),registry_archives=len(dependencies),actual_compiled_registry_packages=len(active),
        actual_dependency_sources=sum(x['actual_sources'] for x in dependencies),actual_live_tests_per_toolchain=38,
        actual_owned_Rust_MySQL_containers=len(children)+len(live_controls),Java_owned_MySQL_containers=1,Java_original_table_cases=12,
        Java_legacy_driver_connection_profiles_failed=2,Java_generated_golden_assets=len(golden),
        native_MySQL_type_identity_verified=True,old_11_execution_controls_rerun=False,
        Java_source_case_parity_complete=False,Rust_Java_codec_interop_verified=False,SQL_Undo_atomicity_verified=False,
        public_COMMIT_available=False,WP05_complete=False,full_V1_complete=False,release_authorized=False,
        external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
