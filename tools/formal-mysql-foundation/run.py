"""Freeze actual MySQL foundation inputs; execute full Rust regressions and two fresh real-server casesets."""
import datetime
import hashlib
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
import source_inventory

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
CACHE = Path('D:/codex-builds/seata-rust-cargo-home')
MSRV = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(value, reason):
    if not value:
        raise ValueError(reason)


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-foundation' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_mysql_foundation_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('MYSQL_FOUNDATION_RUN ' + str(out), flush=True)
    env = os.environ | dict(CARGO_HOME=str(CACHE), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        DOCKER_CONFIG='D:/Temp/seata-docker-config', TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')
    env.pop('RUSTC', None)
    env.pop('RUSTDOC', None)

    def put(name, data):
        path = out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data.encode('utf-8') if isinstance(data, str) else data)

    def save(name, data):
        put(name, json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def command(args, name, cwd=ROOT, extra=None, failure=False, timeout=600):
        result = subprocess.run([str(v) for v in args], cwd=cwd, env=env | (extra or {}),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW, timeout=timeout)
        put(name, result.stdout)
        if failure:
            need(result.returncode == 101 and b'test result: FAILED. 0 passed; 1 failed;' in result.stdout
                and b'panicked at crates' in result.stdout and b'error[E' not in result.stdout
                and b'assertion' in result.stdout, 'Control must compile and fail the target assertion')
        else:
            need(result.returncode == 0, name + ' failed')
        return result.stdout

    inventory, java_inputs = source_inventory.inventory(ROOT)
    save('source-inventory.json', inventory)
    head = command(['git', '-C', source_inventory.JAVA, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip()
    need(head == inventory['java_pin'], 'Pinned Java source differs')
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    active_ids = {row['id'] for row in metadata['resolve']['nodes']}
    active = {(row['name'], row['version']) for row in metadata['packages'] if row['id'] in active_ids and row.get('source')}
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'docs/phase-2/mysql-foundation-contract.md',
        ROOT / 'docs/phase-2/wp05-mysql-at-plan.md', ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md'} | java_inputs
    paths.update(path for path in HERE.rglob('*') if path.is_file() and path.suffix in ['.py', '.sql'])
    paths.update(path for path in (ROOT / 'crates').rglob('*') if path.is_file())
    paths.update(path for path in (ROOT / 'tests/fixtures/protocol/v1').rglob('*') if path.is_file())
    dependencies = []
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))
    for package in lock['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        name = package['name'] + '-' + package['version']
        archive = CACHE / 'registry/cache/index.crates.io-1949cf8c6b5b557f' / (name + '.crate')
        if not archive.exists():
            cached = Path('C:/Users/hnxyh/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f') / archive.name
            need(cached.exists() and sha(cached) == package['checksum'], 'Locked archive absent from existing public cache: ' + name)
            shutil.copyfile(cached, archive)
        need(sha(archive) == package['checksum'], 'Archive checksum differs: ' + name)
        used = (package['name'], package['version']) in active
        source = CACHE / 'registry/src/index.crates.io-1949cf8c6b5b557f' / name
        count = 0
        with tarfile.open(archive, 'r:gz') as bundle:
            for member in bundle.getmembers():
                if not used or not member.isfile():
                    continue
                relative = Path(member.name).relative_to(name)
                need('..' not in relative.parts, 'Unsafe archive member')
                with bundle.extractfile(member) as stream:
                    expected = stream.read()
                path = source / relative
                need(path.read_bytes() == expected, 'Actual dependency source differs: ' + str(path))
                paths.add(path)
                count += 1
        paths.add(archive)
        dependencies.append(dict(name=package['name'], version=package['version'], checksum=package['checksum'],
            active_on_windows_msvc=used, unpacked_files_verified=count))
    save('dependencies.json', dependencies)
    frozen = []
    for path in sorted(paths):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save('source-inputs.json', frozen)
    for label, previous in [('client-deadline', 'run_20261114_032614_f6059c6f'), ('partial-revoke-and-dimension', 'run_20261114_033108_0152055c')]:
        old = ROOT / 'docs/phase-2/evidence/mysql-metadata' / previous
        for item in ['live-tests.log', 'result.json', 'source-inputs.json']:
            put('tdd/' + label + '/' + item, (old / item).read_bytes())
    save('tdd/classification.json', dict(client_deadline='43 actual response bytes held; outer watchdog elapsed instead of inner typed deadline, target assertion failed',
        partial_revoke='actual partially revoked account returned Ok(table), target assertion failed',
        dimensions='positive metadata compatibility failure at column length decoder; not a negative assertion control',
        preliminary_fixtures='031738 lacked UPDATE required by FOR UPDATE, then032128 had earlier metadata text decode error; neither proves the partial-revoke bug',
        API_red='missing exported Rust APIs; compiler failure only'))
    for package in ['seata-datasource', 'seata-db-mysql']:
        command(['cargo', 'fmt', '-p', package, '--', '--check'], package + '/fmt.log')
        command(['cargo', 'clippy', '--locked', '--offline', '-p', package, '--all-targets', '--', '-D', 'warnings'], package + '/clippy.log')
        command(['cargo', 'doc', '--locked', '--offline', '-p', package, '--no-deps'], package + '/rustdoc.log',
            extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-doc', RUSTDOCFLAGS='-D warnings'))
    artifacts, restarts = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE_START ' + label, flush=True)
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.txt', extra=extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 259, 'Expected252 existing +4 metadata +3 options executed tests')
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
        need(count == 33, 'Expected33 actual test targets, including explicitly ignored live entry')
        folders = re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true', raw)
        need(len(folders) == 2, 'Actual forced restart records missing')
        for directory, phase in folders:
            directory = Path(directory.decode())
            need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()), 'Unexpected process evidence directory')
            copied = []
            for path in sorted(directory.rglob('*')):
                if path.is_file():
                    snapshot = 'process-restarts/' + label + '/' + phase.decode() + '/' + path.relative_to(directory).as_posix()
                    put(snapshot, path.read_bytes())
                    copied.append(dict(snapshot=snapshot, sha256=sha(path)))
            restarts.append(dict(rust=label, phase=phase.decode(), actual_children=3, members=copied))
    save('test-artifacts.json', artifacts)
    save('process-restarts.json', restarts)
    controls = [
        ('empty-primary', 'table_meta.rs', 'if primary.is_empty()', 'if false', 'incomplete_duplicate_or_nullable_primary_keys_are_rejected'),
        ('primary-order', 'table_meta.rs', '.map(|(_, index)| index)', '.map(|(position, _)| position as usize - 1)', 'primary_index_order_is_independent_of_physical_column_order'),
        ('nullable-primary', 'column_meta.rs', '|| (primary_key_ordinal.is_some() && nullable)', '', 'incomplete_duplicate_or_nullable_primary_keys_are_rejected'),
        ('decimal-scale', 'column_meta.rs', 's<=30 && s<=p', 'true', 'decimal_metadata_and_schema_budgets_are_checked'),
    ]
    controlled = []
    for name, file, before, after, target in controls:
        directory = work / name
        directory.mkdir()
        for item in ['Cargo.toml', 'Cargo.lock']:
            shutil.copy2(ROOT / item, directory / item)
        shutil.copytree(ROOT / 'crates', directory / 'crates')
        path = directory / 'crates/seata_datasource/src/struct' / file
        source = path.read_text('utf-8')
        need(source.count(before) == 1, 'Control anchor differs: ' + name)
        path.write_text(source.replace(before, after, 1), encoding='utf-8')
        command(['cargo', 'test', '--locked', '--offline', '-p', 'seata-datasource', '--test', 'metadata_contract', target, '--', '--exact', '--nocapture'],
            'controls/' + name + '.log', directory, extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-mysql-controls'), failure=True)
        put('controls/' + name + '.rs', path.read_bytes())
        controlled.append(dict(name=name, target=target, actual_assertion_failed=True, executed_source=str(path),
            snapshot='controls/' + name + '.rs', sha256=sha(path)))
    save('controls.json', controlled)
    children = []
    for rust in ['1.99', '1.85']:
        print('REAL_MYSQL_START ' + rust, flush=True)
        raw = command([sys.executable, HERE / 'live.py', '--rust', rust], 'live-' + rust + '.log')
        match = re.search(rb'MYSQL_METADATA_RUN ([^\r\n]+)', raw)
        need(match is not None, 'Actual child evidence path missing')
        child = Path(match[1].decode())
        result = json.loads((child / 'result.json').read_bytes())
        need(result['succeeded'] and result['actual_live_tests'] == 8 and result['rust'] == rust and not result['cleanup_errors'], 'Eight real MySQL cases missing')
        catalog = json.loads((child / 'catalog.json').read_bytes())['members']
        for row in catalog:
            need(sha(child / row['path']) == row['sha256'] and (child / row['path']).stat().st_size == row['bytes'], 'Actual child member differs')
        children.append(dict(path=str(child), rust=rust, catalog_sha256=sha(child / 'catalog.json'), members=len(catalog), actual_live_tests=8))
    save('live-children.json', children)
    for row in frozen:
        need(sha(Path(row['path'])) == row['sha256'], 'Source/dependency drift during parent Gate')
    for row in artifacts:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual test artifact drift')
    save('result.json', dict(status='verified_MySQL_metadata_foundation_pending_review', workspace_tests_per_toolchain=259,
        pure_new_tests_per_toolchain=7, real_MySQL_tests_per_toolchain=8, actual_MySQL_processes=2,
        Rust_versions=['1.99', '1.85'], workspace_test_artifacts=len(artifacts), assertion_controls=len(controlled),
        actual_forced_restart_children=12, source_inputs=len(frozen), registry_archives=len(dependencies),
        active_dependencies=sum(row['active_on_windows_msvc'] for row in dependencies), dependency_source_files=sum(row['unpacked_files_verified'] for row in dependencies),
        Java_objects_PARTIAL=6, Java_test_files_inventoried=len(inventory['source_test_files']), Java_assets_copied=len(inventory['source_test_assets']),
        Java_source_case_parity_complete=False, business_SQL_execution=False, row_images_complete=False,
        SQL_Undo_atomicity_verified=False, WP05_complete=False, full_V1_complete=False, release_authorized=False,
        external_tc_risk_accepted=False, shared_lock_issue='deferred_by_user'))
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path), bytes=path.stat().st_size)
        for path in sorted(out.rglob('*')) if path.is_file()]
    save('catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
