"""Freeze and verify the bounded row-value model; does not claim database images or Undo."""
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
    out = ROOT / 'docs/phase-2/evidence/mysql-row' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_mysql_row_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('MYSQL_ROW_GATE_RUN ' + str(out), flush=True)
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
    save('source-inventory.json', inventory)
    need(command(['git', '-C', JAVA, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip() == inventory['java_pin'], 'Java pin differs')
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    raw_tree=command(['cargo','tree','--locked','--offline','--workspace','--target','x86_64-pc-windows-msvc','-e','normal,build,dev','--prefix','none','--format','{p}','--no-dedupe'],'actual-tree.txt')
    tree_packages={tuple(line.split()[:2]) for line in raw_tree.decode().splitlines() if len(line.split())>1}
    active={(name,version[1:]) for name,version in tree_packages if not name.startswith('seata-')}
    need(len(active)==148,'Expected actual148 registry packages from cargo tree; metadata weak optional graph is not the compiled closure')
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__), ROOT / 'tools/formal-mysql-foundation/source_inventory.py',
        ROOT / 'docs/phase-2/row-image-contract.md', ROOT / 'docs/phase-2/mysql-foundation-contract.md', ROOT / 'docs/phase-2/mysql-row-read-contract.md', ROOT / 'tools/formal-mysql-foundation/live.py', ROOT / 'tools/formal-mysql-foundation/schema.sql',
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
    for label,directory in [('domain-api','D:/codex-builds/seata-column-domain-api-red-20261114'),('row-api','D:/codex-builds/seata-mysql-row-read-api-red-20261114')]:
        base=Path(directory)
        for path in base.rglob('*'):
            if path.is_file():put('api-red/'+label+'/'+path.relative_to(base).as_posix(),path.read_bytes())
    for label,run in [('physical-domain-red','run_20261114_043633_cb52016a'),('fixture-encoding-failure','run_20261114_043521_d44d020a')]:
        base=ROOT/'docs/phase-2/evidence/mysql-metadata'/run
        for path in base.rglob('*'):
            if path.is_file():put('tdd/'+label+'/'+path.relative_to(base).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(API_red='Missing exported domain/before_image APIs, compiler failure only',
        physical_domain_red='Actual MySQL metadata returned domain None where Integer8 was required; target assertion failed',
        fixture_failure='0 actual tests; initialization CLI lacked SET NAMES utf8mb4, emoji split as multiple characters; not a production RED',
        first_row_suite='044354 nine positive cases passed; cancellation proof was weaker, then revised to establish actual blocked MDL before cancellation',
        stronger_row_suite='045644 ten actual positive cases include MDL blocked before cancellation and DDL success before proxy shutdown'))
    for package in ['seata-datasource','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/rustdoc.log',extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-row-doc',RUSTDOCFLAGS='-D warnings'))
    artifacts, restarts = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE_START ' + label, flush=True)
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.txt', extra=extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 282, 'Expected273 existing plus8 physical domain and1 parameter budget test')
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
        need(count == 36, 'Expected36 actual test targets, including two ignored live suites')
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
        directory=work/name
        directory.mkdir()
        for item in ['Cargo.toml','Cargo.lock']:shutil.copy2(ROOT/item,directory/item)
        shutil.copytree(ROOT/'crates',directory/'crates')
        return directory
    controls=[
        ('signed-width','column_domain.rs','i128::from(*v) >= -bound && i128::from(*v) < bound','true','signed_physical_widths_reject_one_past_each_boundary'),
        ('unsigned-width','column_domain.rs','u128::from(*v) < (1u128 << bits)','u128::from(*v) <= (1u128 << bits)','unsigned_physical_widths_do_not_wrap'),
        ('character-count','column_domain.rs','v.chars().count() as u64 <= limit','true','utf8_character_limit_is_not_an_octet_limit'),
        ('character-set','character_set.rs','text.chars().all(|c| u32::from(c) <= 0xffff)','true','character_sets_reject_unrepresentable_values'),
        ('binary-padding','column_domain.rs','!fixed_width || Some(v.len() as u64) == column.char_octet_length()','true','text_lob_uses_octets_and_fixed_binary_requires_padding'),
        ('unsigned-decimal','column_domain.rs',"!unsigned || !v.as_str().starts_with('-')",'true','unsigned_decimal_rejects_negative_values'),
        ('domain-definition','column_domain.rs','matches!(bits, 8 | 16 | 24 | 32 | 64)','true','inconsistent_domain_metadata_is_rejected'),
    ]
    controlled=[]
    for name,file,before,after,target in controls:
        print('ASSERTION_CONTROL '+name,flush=True)
        directory=isolated(name);path=directory/'crates/seata_datasource/src/struct'/file
        text=path.read_text('utf-8');need(text.count(before)==1,'Control anchor differs '+name)
        path.write_text(text.replace(before,after,1),encoding='utf-8')
        command(['cargo','test','--locked','--offline','-p','seata-datasource','--test','column_domain_contract',target,'--','--exact','--nocapture'],
            'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-row-controls'),negative=True)
        put('controls/'+name+'.rs',path.read_bytes());controlled.append(dict(name=name,target=target,executed_source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target_assertion_failed=True))
    name='parameter-budget';directory=isolated(name);path=directory/'crates/seata_db_mysql/src/mysql_image_reader.rs'
    text=path.read_text('utf-8');before='if bytes > 1_048_576 || total > 8_388_608';need(text.count(before)==2,'Separate parameter/preflight guards required')
    path.write_text(text.replace(before,'if false',1),encoding='utf-8')
    target='mysql_image_reader::tests::oversized_parameters_are_rejected_before_native_argument_encoding'
    command(['cargo','test','--locked','--offline','-p','seata-db-mysql','--lib',target,'--','--exact','--nocapture'],'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-row-controls'),negative=True)
    put('controls/'+name+'.rs',path.read_bytes());controlled.append(dict(name=name,target=target,executed_source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target_assertion_failed=True))
    save('controls.json',controlled)
    children=[]
    for rust in ['1.99','1.85']:
        for suite,expected in [('metadata',9),('row_images',10)]:
            print('REAL_MYSQL '+rust+' '+suite,flush=True)
            raw=command([sys.executable,ROOT/'tools/formal-mysql-foundation/live.py','--rust',rust,'--suite',suite],'live-'+rust+'-'+suite+'.log')
            match=re.search(rb'MYSQL_(?:METADATA|ROW_IMAGES)_RUN ([^\r\n]+)',raw);need(match is not None,'Child path missing')
            child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes())
            need(result['succeeded'] and result['actual_live_tests']==expected and not result['cleanup_errors'],'Required actual database cases missing')
            catalog=json.loads((child/'catalog.json').read_bytes())['members']
            for row in catalog:need(sha(child/row['path'])==row['sha256'],'Actual child member differs')
            children.append(dict(path=str(child),rust=rust,suite=suite,actual_live_tests=expected,members=len(catalog),catalog_sha256=sha(child/'catalog.json')))
    save('live-children.json',children)
    live_controls=[]
    for name,target in [
        ('where-offset','update_before_image_uses_where_parameters_and_preserves_full_exact_rows'),
        ('native-decimal','decimal_and_unsigned_predicate_parameters_keep_native_numeric_semantics'),
        ('preflight-budget','row_field_and_total_budgets_reject_before_full_payload_select'),
        ('transaction-owner','caller_cancellation_releases_the_owned_transaction_before_proxy_shutdown'),
        ('database-primary-order','composite_primary_order_is_database_index_order_not_physical_column_order')]:
        print('REAL_MYSQL_ASSERTION_CONTROL '+name,flush=True)
        directory=isolated('live-'+name)
        (directory/'tools/formal-mysql-foundation').mkdir(parents=True)
        for item in ['live.py','schema.sql']:shutil.copy2(ROOT/'tools/formal-mysql-foundation'/item,directory/'tools/formal-mysql-foundation'/item)
        (directory/'docs/phase-2').mkdir(parents=True)
        for item in ['mysql-foundation-contract.md','mysql-row-read-contract.md']:shutil.copy2(ROOT/'docs/phase-2'/item,directory/'docs/phase-2'/item)
        path=directory/'crates/seata_db_mysql/src'/('connection_proxy.rs' if name=='transaction-owner' else 'mysql_image_reader.rs')
        text=path.read_text('utf-8')
        if name=='where-offset':
            before='&parameters[sql.predicate_parameter_start()..]';need(text.count(before)==1,'Offset anchor');text=text.replace(before,'parameters',1)
        elif name=='native-decimal':
            a=text.index('                FieldValue::Decimal(v) =>');b=text.index('            };',a)
            text=text[:a]+'                FieldValue::Decimal(v) => arguments.add(v.as_str()),\n'+text[b:]
        elif name=='preflight-budget':
            before='if bytes > 1_048_576 || total > 8_388_608';need(text.count(before)==2,'Preflight anchor')
            a=text.index('    fn preflight_sizes(');text=text[:a]+text[a:].replace(before,'if false',1)
        elif name=='database-primary-order':
            before='.primary_key_columns()';need(text.count(before)==1,'Primary order anchor');text=text.replace(before,'.columns().iter().filter(|c| c.primary_key_ordinal().is_some())',1)
        else:
            a=text.index('    pub async fn before_image(');b=text.index('    /// 参数为独占SQLx事务',a);region=text[a:b]
            region=region.replace('let mut transaction = self.transaction.take()','let transaction = self.transaction.as_mut()').replace('MySqlImageReader::read(&mut transaction,','MySqlImageReader::read(transaction,').replace('                self.transaction = Some(transaction);','').replace('                drop(transaction);','')
            need('self.transaction.take()' not in region,'Owner control differs');text=text[:a]+region+text[b:]
        path.write_text(text,encoding='utf-8')
        raw=command([sys.executable,directory/'tools/formal-mysql-foundation/live.py','--suite','row_images','--case',target],
            'live-controls/'+name+'/wrapper.log',cwd=directory,extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-mysql-row-live-controls'),failure_exit=1)
        match=re.search(rb'MYSQL_ROW_IMAGES_RUN ([^\r\n]+)',raw);need(match is not None,'Actual negative child missing')
        child=Path(match[1].decode());result=json.loads((child/'result.json').read_bytes());log=(child/'live-tests.log').read_bytes()
        need(not result['succeeded'] and not result['cleanup_errors'] and b'test result: FAILED. 0 passed; 1 failed;' in log and b'assertion' in log and b'error[E' not in log,'Actual control must compile and fail target assertion')
        frozen_control=json.loads((child/'source-inputs.json').read_bytes())
        for row in frozen_control:need(sha(Path(row['path']))==row['sha256'] and sha(child/row['snapshot'])==row['sha256'],'Negative executed source drift')
        for entry in child.rglob('*'):
            if entry.is_file():put('live-controls/'+name+'/child/'+entry.relative_to(child).as_posix(),entry.read_bytes())
        put('live-controls/'+name+'/executed.rs',path.read_bytes())
        live_controls.append(dict(name=name,target=target,actual_assertion_failed=True,executed_source=str(path),snapshot='live-controls/'+name+'/executed.rs',sha256=sha(path),actual_child=str(child),owned_container_cleaned=True))
    save('live-controls.json',live_controls)
    for row in frozen:
        need(sha(Path(row['path'])) == row['sha256'], 'Source/dependency drift during validation')
    for row in artifacts:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual test artifact drift')
    save('result.json',dict(status='verified_MySQL_before_image_component_pending_review',workspace_tests_per_toolchain=282,
        new_pure_tests_per_toolchain=9,Rust_versions=['1.99','1.85'],actual_test_artifacts=len(artifacts),pure_assertion_controls=len(controlled),
        real_MySQL_assertion_controls=len(live_controls),actual_RM_restart_children=12,source_inputs=len(frozen),registry_archives=len(dependencies),
        actual_compiled_registry_packages=len(active),actual_dependency_sources=sum(x['actual_sources'] for x in dependencies),
        metadata_resolve_candidate_registry_packages=sum(1 for x in metadata['packages'] if x.get('source') and x['id'] in {n['id'] for n in metadata['resolve']['nodes']}),
        actual_owned_MySQL_containers=len(children)+len(live_controls),actual_live_tests_per_toolchain=19,
        Java_objects_PARTIAL=9,Java_source_test_files_inventoried=189,Java_assets_copied=7,Java_concrete_case_parity_complete=False,
        physical_column_domain_profile_verified=True,UPDATE_DELETE_before_image_verified=True,INSERT_images_complete=False,
        production_business_SQL_execution=False,after_images_complete=False,SQL_Undo_atomicity_verified=False,WP05_complete=False,
        full_V1_complete=False,release_authorized=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out/'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
