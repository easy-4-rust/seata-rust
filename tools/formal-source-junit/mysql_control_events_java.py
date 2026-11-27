"""Re-run the unchanged Java TC decisions with supplemental first-phase stage counters."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid
import zipfile

from inventory_rebuilt import ROOT, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD

SCENARIOS = ['commit', 'commit_callback_reply_loss', 'rollback_callback_reply_loss']


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_mysql_control_events_java_' + stamp)
    (work / 'data').mkdir(parents=True)
    out = ROOT / 'docs/phase-2/evidence/mysql-control-events' / ('java_' + stamp)
    out.mkdir(parents=True)
    print('MYSQL_CONTROL_EVENTS_ORIGINAL_JAVA ' + str(out), flush=True)
    owner = uuid.uuid4().hex
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1',
        CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', DOCKER_CONFIG='D:/Temp/seata-docker-config',
        SEATA_MYSQL_CONTROL_EVENTS='1', SEATA_MYSQL_BACKEND='desktop', SEATA_CARGO_TARGET_DIR=str(ROOT.parent / '.seata-target-nodebug'),
        SEATA_CARGO_TARGET_DIR_MSRV=str(ROOT.parent / '.seata-target-nodebug-185'))
    commands, records = [], []

    def command(label, argv, expected=0, timeout=300):
        value = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env, capture_output=True,
                               timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(value.stdout)
        (out / (label + '.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode))
        save(out / 'commands.json', commands)
        if expected is not None:
            need(value.returncode == expected, label + ' failed: ' + (value.stdout + value.stderr)[-1800:].decode(errors='replace'))
        return value

    paths = [p for p in (ROOT / 'crates').rglob('*') if p.is_file()]
    paths += [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__),
              ROOT / 'tools/formal-first-phase-commit/java_tc_report.py']
    source_rows = []
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted(paths):
            member = p.relative_to(ROOT).as_posix()
            archive.writestr(member, p.read_bytes())
            source_rows.append(dict(path=member, sha256=sha(p)))
    save(out / 'source-inputs.json', source_rows)
    container = None
    try:
        container = command('create', ['docker', 'create', '--pull=never', '--name', 'seata-control-events-java-' + stamp.replace('_', '-'),
            '--label', 'seata.fixture.owner=java-tc-report', '--label', 'seata.control.events.owner=' + owner,
            '--mount', 'type=bind,source=' + (work / 'data').as_posix() + ',target=/var/lib/mysql',
            '-p', '127.0.0.1:43318:3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD, '-e', 'MYSQL_ROOT_HOST=%', IMAGE]).stdout.decode().strip()
        env['SEATA_MYSQL_CONTAINER'] = container
        save(out / 'owned-runtime.json', dict(container=container, owner=owner, image=IMAGE, port=43318))
        command('start', ['docker', 'start', container])
        mysql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-h127.0.0.1', '-uroot', '-N', '-B', '-e']
        deadline = time.monotonic() + 120
        while subprocess.run(mysql + ['SELECT VERSION()'], env=env, capture_output=True,
                             timeout=10, creationflags=subprocess.CREATE_NO_WINDOW).returncode:
            need(time.monotonic() < deadline, 'Owned Java TC MySQL startup timeout')
            time.sleep(0.3)
        reference = ROOT / 'docs/phase-2/evidence/java-tc-changed-commit-report'
        for version in ['1.99', '1.85']:
            for scenario in SCENARIOS:
                before = set(reference.glob('run-' + scenario + '-' + version + '-*'))
                command(version + '-' + scenario, [sys.executable, '-B',
                    ROOT / 'tools/formal-first-phase-commit/java_tc_report.py', version, scenario])
                created = set(reference.glob('run-' + scenario + '-' + version + '-*')) - before
                need(len(created) == 1, 'Original Java TC evidence directory ambiguous')
                folder = created.pop()
                need((folder / 'verified.json').is_file(), 'Original Java TC scenario did not qualify')
                need(json.loads((folder / 'process-cleanup.json').read_bytes())['stopped'], 'Original Java TC process remains')
                stage_files = sorted(folder.glob('phase-one-stage-*.json'))
                need(stage_files, 'Original Java TC stage metrics not captured')
                records.append(dict(Rust=version, scenario=scenario, evidence=folder.relative_to(ROOT).as_posix(),
                    verified_sha256=sha(folder / 'verified.json'), frames_sha256=sha(folder / 'RM-frames.jsonl'), control_events_sha256=sha(folder / 'mysql-control-events.json'), control_metrics_sha256=sha(folder / 'mysql-control-metrics.json'),
                    stage_files=[dict(path=p.name, sha256=sha(p)) for p in stage_files]))
                save(out / 'scenarios.json', records)
                print('PASS original Java TC stage observations ' + version + ' ' + scenario, flush=True)
    finally:
        if container:
            state = json.loads(command('inspect-owned', ['docker', 'inspect', container]).stdout)[0]
            need(state['Id'] == container and state['Image'] == IMAGE and state['Config']['Labels'].get('seata.control.events.owner') == owner,
                 'Owned Java TC database container identity differs')
            try:
                command('mysql-log', ['docker', 'logs', container])
            finally:
                try:
                    command('stop', ['docker', 'stop', '--time', '10', container])
                finally:
                    command('remove', ['docker', 'rm', container])
            need(command('confirm-removed', ['docker', 'inspect', container], expected=None).returncode != 0, 'Owned container remains')
        save(out / 'cleanup.json', dict(owned_container_removed=bool(container), D_data_retained=str(work)))
    for row in source_rows:
        need(sha(ROOT / row['path']) == row['sha256'], 'Original Java TC consumed source drift')
    save(out / 'result.json', dict(status='ORIGINAL_JAVA_TC_PHASE_ONE_STAGE_SCENARIOS_PASSED',
         Rust_versions=['1.99', '1.85'], scenarios_per_version=3, cases=records, WP07_complete=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
