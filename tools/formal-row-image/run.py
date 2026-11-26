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
    out = ROOT / 'docs/phase-2/evidence/row-image' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_row_image_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('ROW_IMAGE_MODEL_RUN ' + str(out), flush=True)
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

    def command(args, name, cwd=ROOT, extra=None, negative=False, failure_message=b'assertion'):
        result = subprocess.run([str(x) for x in args], cwd=cwd, env=env | (extra or {}),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW, timeout=600)
        put(name, result.stdout)
        if negative:
            need(result.returncode == 101 and b'test result: FAILED. 0 passed; 1 failed;' in result.stdout
                and failure_message in result.stdout and b'panicked at crates' in result.stdout
                and b'error[E' not in result.stdout, 'Control must compile and fail its target assertion')
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
    inventory['Rust_extensions'].extend(['DecimalValue canonical exact text', 'FieldValue exhaustive value domain', 'ImageError fixed redacted validation'])
    save('source-inventory.json', inventory)
    need(command(['git', '-C', JAVA, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip() == inventory['java_pin'], 'Java pin differs')
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    active_ids = {row['id'] for row in metadata['resolve']['nodes']}
    active = {(row['name'], row['version']) for row in metadata['packages'] if row['id'] in active_ids and row.get('source')}
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__), ROOT / 'tools/formal-mysql-foundation/source_inventory.py',
        ROOT / 'docs/phase-2/row-image-contract.md', ROOT / 'docs/phase-2/mysql-foundation-contract.md',
        ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md'} | java_inputs
    paths.update(p for p in (ROOT / 'crates').rglob('*') if p.is_file())
    paths.update(p for p in (ROOT / 'tests/fixtures/protocol/v1').rglob('*') if p.is_file())
    dependencies = []
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        name = package['name'] + '-' + package['version']
        archive = CACHE / 'registry/cache/index.crates.io-1949cf8c6b5b557f' / (name + '.crate')
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
    api_red = Path('D:/codex-builds/seata-row-image-api-red-20261114')
    for path in api_red.rglob('*'):
        if path.is_file():
            put('api-red/' + path.relative_to(api_red).as_posix(), path.read_bytes())
    command(['cargo', 'fmt', '-p', 'seata-datasource', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--locked', '--offline', '-p', 'seata-datasource', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--locked', '--offline', '-p', 'seata-datasource', '--no-deps'], 'rustdoc.log',
        extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-row-image-doc', RUSTDOCFLAGS='-D warnings'))
    artifacts, restarts = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE_START ' + label, flush=True)
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.txt', extra=extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 273, 'Expected259 existing plus14 row model tests')
        count = 0
        for line in raw.splitlines():
            try:
                row = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            if row.get('reason') == 'compiler-artifact' and row.get('profile', {}).get('test') and row.get('executable'):
                path = Path(row['executable'])
                snapshot = 'artifacts/' + label + '/' + path.name
                put(snapshot, path.read_bytes())
                artifacts.append(dict(path=str(path), snapshot=snapshot, sha256=sha(path), rust=label, target=row['target']['name']))
                count += 1
        need(count == 34, 'Expected34 actual test targets')
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
    controls = [
        ('decimal-canonical', 'decimal_value.rs', "text.strip_prefix('-').unwrap_or(text)", "text.strip_prefix('-').or_else(||text.strip_prefix('+')).unwrap_or(text)", 'decimal_rejects_noncanonical_or_out_of_budget_inputs'),
        ('decimal-column', 'field.rs', 'v.scale()==s && v.integer_digits()<=p-s', 'true', 'decimal_column_capacity_and_exact_scale_are_required'),
        ('nullable', 'field.rs', '(FieldValue::Null, _) => column.nullable()', '(FieldValue::Null, _) => true', 'wrong_value_domains_and_nonnullable_null_are_rejected'),
        ('field-budget', 'field.rs', 'if value.value_bytes() > 1_048_576', 'if false', 'single_field_payload_has_independent_hard_budget'),
        ('complete-schema', 'row.rs', 'field.column() != column', 'false', 'row_requires_complete_ordered_and_exact_column_metadata'),
        ('primary-order', 'row.rs', 'primary.sort_unstable();', 'primary.sort_unstable_by_key(|(_, index)| *index);', 'primary_keys_follow_index_order_and_nonkeys_follow_physical_order'),
        ('table-schema', 'table_records.rs', 'if !row.matches_table(&self.table_meta)', 'if false', 'records_reject_other_schema_and_keep_original_state'),
        ('payload-budget', 'table_records.rs', 'next_bytes > self.byte_limit', 'false', 'row_and_byte_budgets_fail_atomically'),
        ('row-budget', 'table_records.rs', 'self.rows.len() >= self.row_limit', 'false', 'row_and_byte_budgets_fail_atomically'),
        ('debug-redaction', 'field_value.rs', '.field("bytes", &self.value_bytes())', '.field("leak", &match self { Self::Text(v) => v.as_str(), _ => "" })', 'debug_and_errors_do_not_disclose_values'),
    ]
    controlled = []
    for name, file, before, after, target in controls:
        print('ASSERTION_CONTROL ' + name, flush=True)
        directory = work / name
        directory.mkdir()
        for item in ['Cargo.toml', 'Cargo.lock']:
            shutil.copy2(ROOT / item, directory / item)
        shutil.copytree(ROOT / 'crates', directory / 'crates')
        path = directory / 'crates/seata_datasource/src/struct' / file
        text = path.read_text('utf-8')
        need(text.count(before) == 1, 'Control anchor differs: ' + name)
        path.write_text(text.replace(before, after, 1), encoding='utf-8')
        command(['cargo', 'test', '--locked', '--offline', '-p', 'seata-datasource', '--test', 'row_image_contract', target, '--', '--exact', '--nocapture'],
            'controls/' + name + '.log', cwd=directory, extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-row-image-controls'),
            negative=True, failure_message=b'accepted +1' if name == 'decimal-canonical' else b'assertion')
        put('controls/' + name + '.rs', path.read_bytes())
        controlled.append(dict(name=name, target=target, executed_source=str(path), snapshot='controls/' + name + '.rs', sha256=sha(path), target_assertion_failed=True))
    save('controls.json', controlled)
    for row in frozen:
        need(sha(Path(row['path'])) == row['sha256'], 'Source/dependency drift during validation')
    for row in artifacts:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual test artifact drift')
    save('result.json', dict(status='verified_row_value_model_pending_review', workspace_tests_per_toolchain=273,
        row_value_tests_per_toolchain=14, Rust_versions=['1.99','1.85'], actual_test_artifacts=len(artifacts), assertion_controls=len(controlled),
        actual_RM_restart_children=12, source_inputs=len(frozen), registry_archives=len(dependencies), active_dependencies=sum(x['active'] for x in dependencies),
        actual_dependency_sources=sum(x['actual_sources'] for x in dependencies), Java_objects_PARTIAL=len(inventory['objects']),
        Java_test_files_inventoried=len(inventory['source_test_files']), Java_assets_copied=len(inventory['source_test_assets']),
        Java_concrete_case_parity_complete=False, actual_MySQL_execution_this_run=False, actual_SQLx_row_decode=False, row_images_complete=False,
        SQL_Undo_atomicity_verified=False, WP05_complete=False, full_V1_complete=False, release_authorized=False, external_tc_risk_accepted=False,
        shared_lock_issue='deferred_by_user'))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out/'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
