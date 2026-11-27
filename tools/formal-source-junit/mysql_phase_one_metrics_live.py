"""Owned real MySQL preparation/commit/report stage metrics and authenticated HTTP."""
import datetime
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time
import uuid
import zipfile

from inventory_rebuilt import BASELINE, ROOT, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD
from portable_rebuilt import verify

CASES = [
    ('live_first_phase_preparation', 'cases::rejected_and_lost_registration_consume_business_without_fabricating_a_receipt'),
    ('live_first_phase_preparation', 'cases::canceled_real_prepare_keeps_registration_unknown_in_caller_slot'),
    ('live_first_phase_preparation', 'cases::outer_registration_deadline_and_explicit_cancellation_close_original_transaction'),
    ('live_first_phase_preparation', 'cases::existing_guard_rejects_normal_undo_after_registration_and_preserves_real_identity'),
    ('live_first_phase_preparation', 'wait_cases::an_unpolled_preparation_future_drops_the_original_transaction_without_registering'),
    ('live_commit_preconditions', 'experimental_changed_commit_persists_business_undo_and_witness_together'),
    ('live_commit_preconditions', 'experimental_changed_commit_rejects_missing_attempt_migration_before_sql_commit'),
    ('live_commit_preconditions', 'experimental_changed_commit_rejects_legacy_witness_before_sending'),
    ('live_commit_preconditions', 'experimental_changed_commit_driver_loss_retains_unknown_receipt'),
    ('live_commit_preconditions', 'experimental_changed_commit_observer_loss_retains_unknown_after_durable_commit'),
    ('live_commit_preconditions', 'experimental_changed_commit_reports_only_after_durable_witness'),
    ('live_commit_preconditions', 'phase_metrics_cases::rejected_success_report_preserves_confirmed_local_commit_stage'),
    ('live_commit_preconditions', 'phase_metrics_cases::unregistered_combined_commit_is_local_rejection_without_report'),
]
HTTP = 'first_phase_source_exports_31_stage_series_without_promoting_preparation_to_commit'
MUTATION_CASE = 'experimental_changed_commit_reports_only_after_durable_witness'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run():
    package = BASELINE / 'portable_inputs_20261127'
    verify(package)
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_mysql_phase_one_metrics_' + stamp)
    for name in ['data', 'init', 'observations']:
        (work / name).mkdir(parents=True)
    out = ROOT / 'docs/phase-2/evidence/mysql-phase-one-metrics' / ('run_' + stamp)
    out.mkdir(parents=True)
    print('MYSQL_PHASE_ONE_METRICS ' + str(out), flush=True)
    with zipfile.ZipFile(package / 'runtime-inputs.zip') as archive:
        schema = archive.read('stage/source/script/client/at/db/mysql.sql')
    (work / 'init/00-undo.sql').write_bytes(schema)
    (out / 'original-undo.sql').write_bytes(schema)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'),
        CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', DOCKER_CONFIG='D:/Temp/seata-docker-config',
        SEATA_UNDO_SCHEMA_PATH=str(work / 'init/00-undo.sql'), SEATA_MYSQL_PASSWORD=PASSWORD)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    owner = uuid.uuid4().hex
    database = 'undo_' + owner[:12]
    env.update(SEATA_MYSQL_PORT=str(port), SEATA_MYSQL_DATABASE=database)
    commands, artifacts = [], []

    def command(label, argv, extra=None, expected=0):
        value = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env | (extra or {}), capture_output=True,
                               timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(value.stdout)
        (out / (label + '.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode))
        save(out / 'commands.json', commands)
        if expected is not None:
            need(value.returncode == expected, label + ' failed: ' + (value.stdout + value.stderr)[-1400:].decode(errors='replace'))
        return value

    def build(version, package_name, target, label):
        target_dir = ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185')
        argv = ['cargo', '+' + version + '.0', 'test', '-p', package_name, '--test', target,
                '--offline', '--locked', '--no-run', '--message-format=json']
        argv += ['--features', 'mysql' if package_name == 'seata-observability' else 'experimental_changed_commit']
        value = command(label + '-build', argv, dict(CARGO_TARGET_DIR=str(target_dir)))
        binaries = []
        for line in value.stdout.splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if row.get('reason') == 'compiler-artifact' and row['target']['name'] == target and row.get('executable'):
                binaries.append(Path(row['executable']))
        need(len(binaries) == 1, 'Actual test executable missing')
        binary = binaries[0]
        artifacts.append(dict(label=label, path=str(binary), sha256=sha(binary)))
        save(out / 'executed-artifacts.json', artifacts)
        return binary

    def execute(label, binary, case, folder, extra=None, expected=0):
        folder.mkdir(parents=True, exist_ok=True)
        digest = sha(binary)
        value = command(label, [binary, case, '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                        dict(SEATA_UNDO_STORE_OUT=str(folder)) | (extra or {}), expected)
        need(sha(binary) == digest, 'Executed test binary drift')
        need((b'test result: ok. 1 passed;' if expected == 0 else b'test result: FAILED.') in value.stdout,
             'Actual selected test result missing')

    source_rows = []
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        paths = [p for p in (ROOT / 'crates').rglob('*') if p.is_file()]
        paths += [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__)]
        for p in sorted(paths):
            member = p.relative_to(ROOT).as_posix()
            archive.writestr(member, p.read_bytes())
            source_rows.append(dict(path=member, sha256=sha(p)))
    save(out / 'source-inputs.json', source_rows)
    container = None
    try:
        container = command('create', ['docker', 'create', '--pull=never', '--name', 'seata-phase-one-metrics-' + stamp.replace('_', '-'),
            '--label', 'seata-phase-one-metrics-owner=' + owner,
            '--mount', 'type=bind,source=' + (work / 'data').as_posix() + ',target=/var/lib/mysql',
            '--mount', 'type=bind,source=' + (work / 'init').as_posix() + ',target=/docker-entrypoint-initdb.d,readonly',
            '-p', f'127.0.0.1:{port}:3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
            '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=' + database, IMAGE]).stdout.decode().strip()
        save(out / 'owned-runtime.json', dict(container=container, owner=owner, image=IMAGE, database=database, port=port))
        command('start', ['docker', 'start', container])
        mysql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '--protocol=TCP', '-h127.0.0.1', '-uroot', '-N', '-B', database, '-e']
        deadline = time.monotonic() + 120
        while subprocess.run(mysql + ['SELECT COUNT(*) FROM undo_log'], env=env, capture_output=True,
                             creationflags=subprocess.CREATE_NO_WINDOW, timeout=10).returncode:
            need(time.monotonic() < deadline, 'Owned MySQL startup timeout')
            time.sleep(0.3)
        for version in ['1.99', '1.85']:
            folder = work / 'observations' / version
            binaries = {target: build(version, 'seata-db-mysql', target, version + '-' + target) for target in sorted({target for target, _ in CASES})}
            for index, (target, case) in enumerate(CASES):
                execute(version + '-case-' + str(index), binaries[target], case, folder / ('case-' + str(index)))
            binary = build(version, 'seata-observability', 'mysql_recovery', version + '-http')
            execute(version + '-http', binary, HTTP, folder / 'http')
            print('PASS 13 native phase-one cases and authenticated HTTP Rust ' + version, flush=True)
        path = ROOT / 'crates/seata_db_mysql/src/mysql_commit_checked_transaction.rs'
        original = path.read_bytes()
        need(original.count(b'guard.finish_changed(&result);') == 1, 'Settlement mutation site differs')
        try:
            path.write_bytes(original.replace(b'guard.finish_changed(&result);', b'drop(guard);'))
            binary = build('1.99', 'seata-db-mysql', 'live_commit_preconditions', 'mutated-settlement')
            execute('mutated-settlement', binary, MUTATION_CASE, work / 'observations/mutation', expected=101)
        finally:
            path.write_bytes(original)
        binary = build('1.99', 'seata-db-mysql', 'live_commit_preconditions', 'restored-settlement')
        execute('restored-settlement', binary, MUTATION_CASE, work / 'observations/restored')
        command('server-after', mysql + ['SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID<>CONNECTION_ID() AND DB=DATABASE()'])
        for p in (work / 'observations').rglob('*'):
            if p.is_file():
                target = out / 'observations' / p.relative_to(work / 'observations')
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(p, target)
    finally:
        if container:
            state = json.loads(command('inspect-owned', ['docker', 'inspect', container]).stdout)[0]
            need(state['Id'] == container and state['Image'] == IMAGE and state['Config']['Labels'].get('seata-phase-one-metrics-owner') == owner, 'Owned container identity differs')
            try:
                command('mysql-log', ['docker', 'logs', container])
            finally:
                try:
                    command('stop', ['docker', 'stop', '--time', '10', container])
                finally:
                    command('remove', ['docker', 'rm', container])
            need(command('confirm-removed', ['docker', 'inspect', container], expected=None).returncode != 0, 'Owned container remains')
        save(out / 'cleanup.json', dict(owned_container_removed=bool(container), D_workspace_retained=str(work)))
    for row in source_rows:
        need(sha(ROOT / row['path']) == row['sha256'], 'Source not restored after mutation')
    save(out / 'result.json', dict(status='PHASE_ONE_STAGE_METRICS_MYSQL_AND_HTTP_VERIFIED', Rust_versions=['1.99', '1.85'],
         live_cases_per_version=14, settlement_mutation_rejected=True,
         restored_target_passed=True, fixed_http_series=63, WP07_complete=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
