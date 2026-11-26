"""Run actual MySQL metadata tests in an owned cached-image container with D: bind data; never shared databases."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/formal-mysql-foundation'
import sys
sys.path.insert(0,str(ROOT/'tools/formal-lock-key'))
from native_peer import extra_inputs,run_native_peer
JAVA = Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
IMAGE = 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
PASSWORD = 'seata-mysql-fixture-only'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(case, rust='1.99', suite='metadata'):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    family={'undo_codec':'mysql-lock-key','metadata':'mysql-metadata','row_images':'mysql-row-images','image_race':'mysql-image-race','execution':'mysql-execution','mysql_types':'mysql-type-identity'}[suite]
    out = ROOT / 'docs/phase-2/evidence' / family / ('run_' + stamp)
    target = 'live_' + suite
    runtime = Path('D:/codex-builds') / ('seata_mysql_' + stamp)
    out.mkdir(parents=True)
    (runtime / 'data').mkdir(parents=True)
    (runtime / 'init').mkdir()
    owner = uuid.uuid4().hex
    database = 'at_md_' + owner[:12]
    name = 'seata-at-metadata-' + stamp.replace('_', '-').lower()
    env = os.environ | dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home', CARGO_TARGET_DIR=os.environ.get('SEATA_MYSQL_TARGET_DIR','D:/cargo-target/seata-rust-production'),
        DOCKER_CONFIG='D:/Temp/seata-docker-config', TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')
    cargo = 'cargo'
    env.pop('RUSTC', None)
    env.pop('RUSTDOC', None)
    if rust == '1.85':
        toolchain = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
        cargo = toolchain / 'cargo.exe'
        env.update(RUSTC=str(toolchain / 'rustc.exe'), RUSTDOC=str(toolchain / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
    with socket.socket() as reserved:
        reserved.bind(('127.0.0.1', 0))
        port = reserved.getsockname()[1]
    marker={'undo_codec':'MYSQL_LOCK_KEYS_RUN ','metadata':'MYSQL_METADATA_RUN ','row_images':'MYSQL_ROW_IMAGES_RUN ','image_race':'MYSQL_IMAGE_RACE_RUN ','execution':'MYSQL_EXECUTION_RUN ','mysql_types':'MYSQL_TYPE_IDENTITY_RUN '}[suite]
    print(marker + str(out), flush=True)

    def put(path, data):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data.encode('utf-8') if isinstance(data, str) else data)

    def save(path, value):
        put(path, json.dumps(value, ensure_ascii=False, indent=2) + '\n')

    def command(args, log=None, timeout=90, check=True):
        value = subprocess.run([str(arg) for arg in args], cwd=ROOT, env=env, creationflags=subprocess.CREATE_NO_WINDOW,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        if log:
            put(out / log, value.stdout)
        if check and value.returncode:
            raise ValueError((log or str(args[0])) + ' failed, exit=' + str(value.returncode))
        return value

    image = json.loads(command(['docker', 'image', 'inspect', IMAGE], 'image.json').stdout)[0]
    if image['Id'] != IMAGE:
        raise ValueError('Cached image identity differs')
    original_schema = JAVA / 'script/client/at/db/mysql.sql'
    put(runtime / 'init/00-java-undo.sql', original_schema.read_bytes())
    fixture = (HERE / 'schema.sql').read_text('utf-8').replace('__DATABASE__', database) + '\n' + (ROOT/'tools/formal-lock-key/schema-extra.sql').read_text('utf-8')
    put(runtime / 'init/01-fixture.sql', fixture)
    put(out / 'fixture.sql', fixture)
    put(out / 'java-undo.sql', original_schema.read_bytes())
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve(), HERE / 'schema.sql', original_schema,
        ROOT / 'docs/phase-2/mysql-foundation-contract.md', ROOT / 'docs/phase-2/mysql-row-read-contract.md',
        ROOT / 'docs/phase-2/mysql-execution-images-contract.md'}
    paths.update(path for path in (ROOT / 'crates').rglob('*') if path.is_file())
    paths.update(extra_inputs())
    frozen = []
    for path in sorted(paths):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(out / snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save(out / 'source-inputs.json', frozen)
    command([env.get('RUSTC','rustc'), '--version', '--verbose'], 'rust-version.txt')
    build = command([cargo, 'test', '--locked', '--offline', '-p', 'seata-db-mysql', '--test', target, '--no-run', '--message-format=json'], 'build.jsonl', timeout=240)
    artifacts = []
    for line in build.stdout.splitlines():
        try:
            row = json.loads(line)
        except (ValueError, UnicodeDecodeError):
            continue
        if row.get('reason') == 'compiler-artifact' and row['target']['name'] == target and row.get('executable') and row['profile'].get('test'):
            artifacts.append(Path(row['executable']))
    if len(artifacts) != 1:
        raise ValueError('Expected one actual compiled live test executable')
    binary = artifacts[0]
    binary_sha = sha(binary)
    put(out / ('artifacts/' + target + '.exe'), binary.read_bytes())
    save(out / 'test-artifact.json', dict(path=str(binary), sha256=binary_sha, snapshot='artifacts/' + target + '.exe', rust=rust))
    command([cargo, 'tree', '--locked', '--offline', '-p', 'seata-db-mysql', '-e', 'features'], 'features.txt')
    env.update(SEATA_MYSQL_PORT=str(port), SEATA_MYSQL_PASSWORD=PASSWORD, SEATA_MYSQL_DATABASE=database, SEATA_MYSQL_CODEC_OUT=str(runtime/'codec-rust'), SEATA_MYSQL_LOCK_KEYS_OUT=str(runtime/'lock-rust'))
    container = None
    success = False
    cleanup = []
    executed = 0
    passed = failed = 0
    try:
        args = ['docker', 'create', '--pull=never', '--name', name, '--label', 'seata-rust-owner=' + owner,
            '--mount', 'type=bind,source=' + (runtime / 'data').as_posix() + ',target=/var/lib/mysql',
            '--mount', 'type=bind,source=' + (runtime / 'init').as_posix() + ',target=/docker-entrypoint-initdb.d,readonly',
            '-p', '127.0.0.1:' + str(port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
            '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=' + database, IMAGE]
        container = command(args, 'create.log').stdout.decode().strip()
        save(out / 'owned-runtime.json', dict(container_id=container, name=name, label_owner=owner, database=database,
            port=port, runtime=str(runtime), fixture_credentials_public=True, image=IMAGE))
        command(['docker', 'start', container], 'start.log')
        mysql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '--protocol=TCP', '-h127.0.0.1', '-uroot', '-N', '-B', database, '-e']
        until = time.monotonic() + 120
        while True:
            state = json.loads(command(['docker', 'inspect', container]).stdout)[0]
            if not state['State']['Running']:
                raise ValueError('Owned MySQL exited during startup')
            value = command(mysql + ['SELECT COUNT(*) FROM account'], check=False)
            if value.returncode == 0 and value.stdout.strip() == b'2':
                break
            if time.monotonic() >= until:
                raise ValueError('Owned MySQL readiness deadline')
            time.sleep(.25)
        command(mysql + ['SELECT VERSION(),@@sql_mode,@@transaction_isolation,@@lower_case_table_names; SELECT COUNT(*),MIN(b),MAX(unsigned_id) FROM account'], 'server-before.txt')
        command(['docker', 'inspect', container], 'container-before.json')
        args = [binary]
        if case:
            args.append(case)
        args += ['--ignored', '--nocapture', '--test-threads=1']
        if case:
            args.append('--exact')
        value = command(args, 'live-tests.log', timeout=120, check=False)
        terminal = re.search(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;', value.stdout)
        if terminal:
            passed, failed = map(int, terminal.groups())
            executed = passed + failed
        command(mysql + ['SELECT COUNT(*),MIN(b),MAX(unsigned_id) FROM account; SELECT COUNT(*) FROM trigger_log; SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID<>CONNECTION_ID() AND DB=DATABASE()'], 'server-after.txt')
        if value.returncode:
            raise ValueError('Actual MySQL test assertions failed')
        count = re.search(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;', value.stdout)
        if not count or int(count[1]) == 0:
            raise ValueError('No actual selected MySQL test executed')
        executed = int(count[1])
        run_native_peer(out,runtime,env)
        success = True
    finally:
        if container:
            current = json.loads(command(['docker', 'inspect', container]).stdout)[0]
            if current['Config']['Labels'].get('seata-rust-owner') != owner or current['Image'] != IMAGE:
                raise ValueError('Refusing cleanup of non-owned container')
            command(['docker', 'logs', container], 'container.log', check=False)
            command(['docker', 'stop', '-t', '10', container], 'stop.log', timeout=30)
            state = json.loads(command(['docker', 'inspect', container], 'container-stopped.json').stdout)[0]
            if state['State']['Running']:
                cleanup.append('owned container still running')
            command(['docker', 'rm', container], 'remove.log')
        save(out / 'result.json', dict(succeeded=success, selected_case=case, actual_MySQL=True, image=IMAGE,
            source_inputs=len(frozen), actual_live_tests=executed, passed_live_tests=passed, failed_live_tests=failed,
            rust=rust, suite=suite, cleanup_errors=cleanup, WP05_complete=False, SQL_Undo_atomicity_verified=False,
            full_V1_complete=False, external_tc_risk_accepted=False, shared_lock_issue='deferred_by_user'))
    if cleanup:
        raise ValueError('Owned MySQL cleanup failed')
    for row in frozen:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Compiled/source input drift during live run')
    if sha(binary) != binary_sha:
        raise ValueError('Actual executed binary drift')
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path), bytes=path.stat().st_size)
        for path in sorted(out.rglob('*')) if path.is_file()]
    save(out / 'catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case')
    parser.add_argument('--rust',choices=['1.99','1.85'],default='1.99')
    parser.add_argument('--suite',choices=['metadata','row_images','image_race','execution','mysql_types','undo_codec'],default='undo_codec')
    args = parser.parse_args()
    run(args.case,args.rust,args.suite)
