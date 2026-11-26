"""Execute selected SQL-recognition profile and all current core regressions; no MySQL/Undo Gate claim."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tomllib
import uuid
import source_inventory

ROOT = Path(__file__).resolve().parents[2]
CACHE = Path('D:/codex-builds/seata-rust-cargo-home')
MSRV = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(value, reason):
    if not value:
        raise ValueError(reason)


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/at-sql' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_at_sql_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('AT_SQL_RUN ' + str(out), flush=True)
    env = os.environ | dict(CARGO_HOME=str(CACHE), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')

    def put(name, data):
        path = out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data.encode('utf-8') if isinstance(data, str) else data)

    def save(name, data):
        put(name, json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def command(args, log, cwd=ROOT, extra=None, failure=False):
        result = subprocess.run([str(v) for v in args], cwd=cwd, env=env | (extra or {}),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW, timeout=240)
        put(log, result.stdout)
        if failure:
            need(result.returncode == 101 and b'panicked at crates' in result.stdout and b'test result: FAILED. 0 passed; 1 failed;' in result.stdout
                and b'error[E' not in result.stdout and b'called `Result::unwrap()`' not in result.stdout
                and b'called `Option::unwrap()`' not in result.stdout, 'Control must compile and fail its target assertion: ' + log)
        else:
            need(result.returncode == 0, 'Command failed: ' + log)
        return result.stdout

    inventory, java_inputs = source_inventory.inventory(ROOT)
    save('source-inventory.json', inventory)
    java_head = command(['git', '-C', source_inventory.JAVA, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip()
    need(java_head == inventory['java_pin'], 'Pinned Java source differs')
    expected_sql = len(re.findall(r'^#\[test\]', (ROOT / 'crates/seata_at/tests/mysql_sql_recognition.rs').read_text('utf-8'), re.M))
    need(expected_sql >= 18, 'SQL contract tests missing')
    expected_total = 234 + expected_sql
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    active_ids = {row['id'] for row in metadata['resolve']['nodes']}
    active_packages = {(row['name'], row['version']) for row in metadata['packages'] if row['id'] in active_ids and row.get('source')}
    inputs = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve(), Path(source_inventory.__file__).resolve(),
        ROOT / 'docs/phase-2/wp05-mysql-at-plan.md', ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md'} | java_inputs
    inputs.update(path for path in (ROOT / 'crates').rglob('*') if path.is_file())
    inputs.update(path for path in (ROOT / 'tests/fixtures/protocol/v1').rglob('*') if path.is_file())
    dependencies = []
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        name = package['name'] + '-' + package['version']
        archive = CACHE / 'registry/cache/index.crates.io-1949cf8c6b5b557f' / (name + '.crate')
        need(sha(archive) == package['checksum'], 'Archive differs: ' + name)
        source = CACHE / 'registry/src/index.crates.io-1949cf8c6b5b557f' / name
        active = (package['name'], package['version']) in active_packages
        count = 0
        with tarfile.open(archive, 'r:gz') as bundle:
            for member in bundle.getmembers():
                if not active or not member.isfile():
                    continue
                relative = Path(member.name).relative_to(name)
                need('..' not in relative.parts, 'Unsafe archive member')
                with bundle.extractfile(member) as stream:
                    expected = stream.read()
                actual = source / relative
                need(actual.read_bytes() == expected, 'Actual unpacked dependency source differs: ' + str(actual))
                inputs.add(actual)
                count += 1
        inputs.add(archive)
        dependencies.append(dict(name=package['name'], version=package['version'], checksum=package['checksum'],
            active_on_windows_msvc=active, unpacked_files_verified=count))
    need(len(dependencies) == 24, 'Expected existing22 plus two selected parser dependencies')
    save('dependencies.json', dependencies)
    frozen = []
    for path in sorted(inputs):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save('source-inputs.json', frozen)
    red_root = Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for name in ['wp05-parser-cached-dependencies.json', 'wp05-sql-api-red.log', 'wp05-sql-boundary-behavior-red.log',
        'wp05-sql-unsupported-operator-red.log', 'wp05-sql-bare-builtin-red.log', 'wp05-sql-consumed-modifier-red.log', 'wp05-sql-parser-panic-red.log']:
        put('tdd/' + name, (red_root / name).read_bytes())
    save('tdd/classification.json', dict(api_red='missing public API compiler error; not a behavior assertion',
        boundary_red='three actual failed assertions', unsupported_operator_red='one actual failed assertion',
        bare_builtin_red='one actual failed assertion', consumed_modifier_red='one actual failed assertion',
        parser_panic_red='actual upstream MySQL DIV unwrap panic, not an assertion or typed parse failure'))
    put('tdd/tm-registration-race.log', (ROOT / 'docs/phase-2/evidence/at-sql/run_20261114_023055_05804c55/rust-1-85/tests.jsonl').read_bytes())
    save('tdd/tm-registration-race.json', dict(classification='existing test fixture: fixed20ms sleep permitted shutdown during fragmented registration ACK write; Windows10053',
        fix='bounded oneshot acknowledgement after the fixture actually finishes registration write', production_TM_changed=False))
    command(['cargo', 'fmt', '-p', 'seata-at', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--offline', '--locked', '-p', 'seata-at', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--offline', '--locked', '-p', 'seata-at', '--no-deps'], 'rustdoc.log',
        extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-at-doc', RUSTDOCFLAGS='-D warnings'))
    artifacts = []
    process_restarts = []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.log', extra=extra)
        raw = command([cargo, 'test', '--offline', '--locked', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra=extra)
        need(sum(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == expected_total, 'Executed workspace count differs')
        folders = re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true', raw)
        need(len(folders) == 2, 'Expected actual commit/rollback forced-restart parents')
        for directory, phase in folders:
            directory = Path(directory.decode())
            need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()), 'Unexpected restart evidence directory')
            copied = []
            for path in sorted(directory.rglob('*')):
                if path.is_file():
                    snapshot = 'process-restarts/' + label + '/' + phase.decode() + '/' + path.relative_to(directory).as_posix()
                    put(snapshot, path.read_bytes())
                    copied.append(dict(snapshot=snapshot, sha256=sha(path)))
            need(copied, 'Missing actual child/restart records')
            process_restarts.append(dict(rust=label, phase=phase.decode(), parent_reported_children=3, original_directory=str(directory), members=copied))
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
                artifacts.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot, target=row['target']['name'], rust=label))
                count += 1
        need(count == 28, 'Expected existing26 +2 SQL actual test targets')
    save('test-artifacts.json', artifacts)
    save('process-restarts.json', process_restarts)
    controls = [
        ('parameter-count', 'mysql/base_mysql_recognizer.rs', '*parameters += 1;', '*parameters += 0;', 'insert_retains_table_columns_rows_original_sql_and_real_parameters'),
        ('where-offset', 'mysql/mysql_update_recognizer.rs', 'let predicate_parameter_start = parameter_count;', 'let predicate_parameter_start = 0;', 'update_separates_assignment_parameters_from_where_image_parameters'),
        ('duplicate-columns', 'mysql/base_mysql_recognizer.rs', 'if !seen.insert(name.to_ascii_lowercase()) {', 'if false && !seen.insert(name.to_ascii_lowercase()) {', 'duplicate_columns_and_ragged_values_are_rejected'),
        ('values-width', 'mysql/mysql_insert_recognizer.rs', 'if row.len() != columns.len() {', 'if false {', 'duplicate_columns_and_ragged_values_are_rejected'),
        ('constant-in', 'mysql/base_mysql_recognizer.rs', 'Self::expression(item, parameters, depth + 1, false)?;', 'Self::expression(item, parameters, depth + 1, true)?;', 'in_values_cannot_silently_expand_the_declared_constant_or_bound_value_matrix'),
        ('operator-preflight', 'sql_recognizer_factory.rs', 'if operators > 256 {', 'if false {', 'flat_operator_budget_is_checked_before_constructing_a_deep_ast'),
        ('canonical-tokens', 'sql_recognizer_factory.rs', 'Self::verify_ast_tokens(&tokens, &statements[0], &dialect)?;', '', 'parser_consumed_modifiers_cannot_disappear_from_the_executable_contract'),
        ('bare-builtins', 'mysql/base_mysql_recognizer.rs', 'if name.quote_style.is_none()', 'if false', 'bare_mysql_builtins_are_not_misclassified_as_deterministic_columns'),
        ('raw-budget', 'sql_recognizer_factory.rs', 'if sql.len() > 65_536 {', 'if false {', 'raw_budget_is_enforced_before_parsing_and_errors_redact_sql_values'),
        ('debug-redaction', 'sql_recognizer.rs', '.field("sql_type", &self.sql_type)', '.field("sql_type", &self.sql_type).field("sql", &self.sql)', 'public_debug_does_not_disclose_literal_sql_or_predicate_values'),
    ]
    results = []
    for name, relative, before, after, test in controls:
        directory = work / name
        directory.mkdir()
        for item in ['Cargo.toml', 'Cargo.lock']:
            shutil.copy2(ROOT / item, directory / item)
        shutil.copytree(ROOT / 'crates', directory / 'crates')
        path = directory / 'crates/seata_at/src' / relative
        source = path.read_text('utf-8')
        need(source.count(before) == 1, 'Mutation anchor differs: ' + name)
        path.write_text(source.replace(before, after, 1), encoding='utf-8')
        command(['cargo', 'test', '--offline', '--locked', '-p', 'seata-at', '--test', 'mysql_sql_recognition', test, '--', '--exact', '--nocapture'],
            'controls/' + name + '.log', directory, extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-at-controls'), failure=True)
        snapshot = 'controls/' + name + '.rs'
        put(snapshot, path.read_bytes())
        results.append(dict(name=name, target_test=test, compiled=True, target_assertion_failed=True,
            executed_source=str(path), sha256=sha(path), snapshot=snapshot))
        print('CONTROL_PASS ' + name, flush=True)
    save('controls.json', results)
    for row in frozen:
        need(sha(Path(row['path'])) == row['sha256'], 'Source/dependency drift during execution')
    for row in artifacts:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual executed artifact drift')
    save('result.json', dict(status='verified_selected_SQL_recognition_profile_pending_review', SQL_tests_per_toolchain=expected_sql,
        workspace_tests_per_toolchain=expected_total, rust_versions=['1.99', '1.85'], test_artifacts=len(artifacts),
        assertion_controls=len(results), registry_archives=len(dependencies), active_dependencies=sum(row['active_on_windows_msvc'] for row in dependencies),
        actual_dependency_source_files=sum(row['unpacked_files_verified'] for row in dependencies), source_inputs=len(frozen),
        Java_source_test_files=len(inventory['source_test_files']), Java_test_assets=len(inventory['source_test_assets']),
        source_parity_complete=False, Java_module_migration_complete=False, real_MySQL_execution=False, SQLx_driver_added=False,
        actual_forced_restart_children=sum(row['parent_reported_children'] for row in process_restarts),
        source_drift=False, WP05_complete=False, WP06_complete=False, SQL_Undo_atomicity_verified=False, full_V1_complete=False,
        release_authorized=False, external_tc_risk_accepted=False, shared_lock_issue='deferred_by_user'))
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path), bytes=path.stat().st_size)
        for path in sorted(out.rglob('*')) if path.is_file()]
    save('catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
