"""Exercise the shared Undo rendering/selection in owned real MySQL compensation."""
import datetime
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
from portable_rebuilt import verify

IMAGE = 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
PASSWORD = 'seata-mysql-fixture-only'
CASES = [
    'stored_insert_undo_restores_business_and_consumes_original_row_atomically',
    'update_and_delete_restore_original_values_without_pk_cartesian_products',
    'mixed_logs_run_in_reverse_and_consume_only_original_branch',
    'later_dirty_failure_rolls_back_earlier_compensation_and_keeps_all_undo_bytes',
    'already_restored_and_equal_images_skip_dml_then_follow_original_missing_log_fence',
]


def run():
    package = BASELINE / 'portable_inputs_20261127'
    manifest = verify(package)
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_mysql_undo_executor_live_' + stamp)
    for name in ['data', 'init', 'observations']:
        (work / name).mkdir(parents=True)
    out = ROOT / 'docs/phase-2/evidence/mysql-undo-executor-source' / ('live_' + stamp)
    out.mkdir(parents=True)
    print('MYSQL_UNDO_EXECUTOR_LIVE ' + str(out), flush=True)
    member = 'stage/source/script/client/at/db/mysql.sql'
    with zipfile.ZipFile(package / 'runtime-inputs.zip') as archive:
        schema = archive.read(member)
    (work / 'init/00-undo.sql').write_bytes(schema)
    (out / 'original-undo.sql').write_bytes(schema)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp',
        CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_INCREMENTAL='0', CARGO_PROFILE_TEST_DEBUG='0',
        DOCKER_CONFIG='D:/Temp/seata-docker-config', SEATA_UNDO_SCHEMA_PATH=str(work / 'init/00-undo.sql'))
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    owner = uuid.uuid4().hex
    database = 'undo_' + owner[:12]
    env.update(SEATA_MYSQL_PORT=str(port), SEATA_MYSQL_DATABASE=database, SEATA_MYSQL_PASSWORD=PASSWORD)
    commands = []

    def command(label, argv, extra=None, expected=0, timeout=300):
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env | (extra or {}),
                                capture_output=True, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=result.returncode))
        save(out / 'commands.json', commands)
        if expected is not None:
            need(result.returncode == expected, label + ' failed: ' + result.stderr[-1200:].decode(errors='replace'))
        return result

    source_files = [p for p in (ROOT / 'crates').rglob('*') if p.is_file()]
    source_files += [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__)]
    source_rows = []
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(source_files):
            leaf = path.relative_to(ROOT).as_posix()
            archive.writestr(leaf, path.read_bytes())
            source_rows.append(dict(path=leaf, sha256=sha(path)))
    save(out / 'source-inputs.json', source_rows)

    def build(label, version):
        target = ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185')
        result = command(label + '-build', ['cargo', '+' + version + '.0', 'test', '--offline', '--locked',
            '-p', 'seata-db-mysql', '--test', 'live_undo_rollback', '--no-run', '--message-format=json'],
            dict(CARGO_TARGET_DIR=str(target)))
        binaries = []
        for line in result.stdout.splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if row.get('reason') == 'compiler-artifact' and row['target']['name'] == 'live_undo_rollback' and row.get('executable'):
                binaries.append(Path(row['executable']))
        need(len(binaries) == 1, 'Actual live test executable missing')
        binary = binaries[0]
        save(out / (label + '-artifact.json'), dict(path=str(binary), sha256=sha(binary), bytes=binary.stat().st_size))
        return binary, sha(binary)

    container = None
    success = False
    try:
        container = command('create', ['docker', 'create', '--pull=never', '--name', 'seata-undo-executor-' + stamp.replace('_', '-'),
            '--label', 'seata-undo-executor-owner=' + owner,
            '--mount', 'type=bind,source=' + (work / 'data').as_posix() + ',target=/var/lib/mysql',
            '--mount', 'type=bind,source=' + (work / 'init').as_posix() + ',target=/docker-entrypoint-initdb.d,readonly',
            '-p', f'127.0.0.1:{port}:3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
            '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=' + database, IMAGE]).stdout.decode().strip()
        save(out / 'owned-runtime.json', dict(container=container, owner=owner, image=IMAGE, database=database, port=port))
        command('start', ['docker', 'start', container])
        mysql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '--protocol=TCP',
                 '-h127.0.0.1', '-uroot', '-N', '-B', database, '-e']
        deadline = time.monotonic() + 120
        while True:
            check = subprocess.run(mysql + ['SELECT COUNT(*) FROM undo_log'], env=env, capture_output=True,
                                   timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
            if check.returncode == 0:
                break
            need(time.monotonic() < deadline, 'Owned MySQL startup timed out')
            time.sleep(0.3)
        command('server-before', mysql + ['SELECT VERSION(),@@max_allowed_packet; SET GLOBAL log_output="TABLE"; SET GLOBAL general_log=ON'])
        for version in ['1.99', '1.85']:
            observations = work / 'observations' / version
            observations.mkdir()
            binary, digest = build(version, version)
            for index, case in enumerate(CASES):
                result = command(version + '-case-' + str(index), [binary, case, '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                                 dict(SEATA_UNDO_STORE_OUT=str(observations)))
                need(b'test result: ok. 1 passed; 0 failed;' in result.stdout, 'Selected real MySQL case not executed')
                need(sha(binary) == digest, 'Executed live binary drift')
            print('PASS real MySQL five compensation cases Rust ' + version, flush=True)
        path = ROOT / 'crates/seata_db_mysql/src/mysql/mysql_undo_update_executor.rs'
        original = path.read_bytes()
        before, after = b'"UPDATE {} SET {} WHERE {}"', b'"SELECT {} SET {} WHERE {}"'
        need(original.count(before) == 1, 'Live SQL builder mutation site differs')
        try:
            path.write_bytes(original.replace(before, after))
            binary, digest = build('mutated-update-sql', '1.99')
            result = command('mutated-update-sql-target', [binary, CASES[1], '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                             dict(SEATA_UNDO_STORE_OUT=str(work / 'observations')), expected=101)
            need(b'Undo update compensation' in result.stdout + result.stderr and b'test result: FAILED.' in result.stdout,
                 'Mutation failed outside actual database compensation')
            need(sha(binary) == digest, 'Mutated executed binary drift')
        finally:
            path.write_bytes(original)
        binary, digest = build('restored-update-sql', '1.99')
        restored = command('restored-update-sql-target', [binary, CASES[1], '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                           dict(SEATA_UNDO_STORE_OUT=str(work / 'observations')))
        need(b'test result: ok. 1 passed; 0 failed;' in restored.stdout, 'Restored live target did not pass')
        need(sha(binary) == digest, 'Restored executed binary drift')
        command('mysql-compensation-statements', mysql + [
            "SELECT command_type,argument FROM mysql.general_log WHERE command_type IN ('Prepare','Execute') "
            "AND (argument LIKE '%UPDATE `%` SET%' OR argument LIKE '%INSERT INTO `%` (%' OR argument LIKE '%DELETE FROM `%` WHERE%' OR argument LIKE '%SELECT `%` SET%') ORDER BY event_time"])
        command('server-after', mysql + ["SELECT COUNT(*) FROM undo_log; SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID<>CONNECTION_ID() AND DB=DATABASE(); SET GLOBAL general_log=OFF"])
        for p in (work / 'observations').rglob('*'):
            if p.is_file():
                target = out / 'observations' / p.relative_to(work / 'observations')
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(p, target)
        success = True
    finally:
        if container:
            state = json.loads(command('inspect-owned', ['docker', 'inspect', container]).stdout)[0]
            need(state['Id'] == container and state['Image'] == IMAGE and
                 state['Config']['Labels'].get('seata-undo-executor-owner') == owner, 'Owned container identity differs')
            try:
                command('mysql-log', ['docker', 'logs', container])
            finally:
                try:
                    command('stop-owned', ['docker', 'stop', '--time', '10', container])
                finally:
                    command('remove-owned', ['docker', 'rm', container])
            need(command('confirm-removed', ['docker', 'inspect', container], expected=None).returncode != 0,
                 'Owned MySQL container remains')
        save(out / 'cleanup.json', dict(owned_container_removed=bool(container), retained_D_workspace=str(work)))
    for row in source_rows:
        need(sha(ROOT / row['path']) == row['sha256'], 'Rust source not restored after live mutation')
    save(out / 'result.json', dict(status='MYSQL_UNDO_EXECUTOR_REAL_DATABASE_REGRESSION_VERIFIED',
         verified=success, Rust_versions=['1.99', '1.85'], cases_per_version=5, cases=CASES,
         native_SQL_builder_mutation_rejected=True, restored_target_passed=True,
         original_DDL_sha256=sha(out / 'original-undo.sql'), public_COMMIT_available=False, WP05_complete=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
