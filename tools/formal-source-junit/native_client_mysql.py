"""Initial approved SQL grammar: actual original TC, formal Rust commit/compensation, original Java parser."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid
from java_writer_jdbc_profile_tc import ROOT, BASE, MIGRATIONS, IMAGE, PASSWORD, module, need, put, reg, save, sha, support, port, wait

DATA = ROOT / 'crates/seata_orm_rbatis/tests/data/entity_cases.json'
NATIVE = 'native::original_java_tc_executes_unified_client'


def run(versions, directions):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/client-mysql' / stamp
    work = Path('D:/codex-builds') / ('client-mysql-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True); print('CLIENT_MYSQL ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', DOCKER_CONFIG='D:/Temp/seata-docker-config',
        CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'RUSTC', 'RUSTDOC', 'SEATA_CONFIG_NAME', 'SEATA_ENV'): env.pop(key, None)
    commands = []

    def command(label, args, data=None, timeout=150):
        if (out / (label + '.stdout')).exists(): label += '-' + str(len(commands))
        value = subprocess.run(list(map(str, args)), cwd=work, env=env, input=data, capture_output=True, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / (label + '.stdout'), value.stdout); put(out / (label + '.stderr'), value.stderr)
        commands.append(dict(label=label, argv=list(map(str, args)), exit_code=value.returncode)); save(out / 'commands.json', commands)
        need(value.returncode == 0, label + ': ' + (value.stdout + value.stderr)[-2500:].decode(errors='replace')); return value.stdout

    need(command('java-pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).decode().strip() == reg.PIN, 'Java pin differs')
    jar = reg.ORACLE / 'server/target/seata-server.jar'; need(sha(jar) == reg.JAR_SHA, 'Original TC changed')
    need(sha(BASE / 'catalog.json') == '59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799', 'Frozen dependency baseline differs')
    jars = []
    for row in json.loads((BASE / 'dependencies.json').read_bytes()):
        p = Path(row['path']); need(sha(p) == row['sha256'], 'Dependency differs')
        if not p.name.startswith(('mockito-', 'byte-buddy-', 'objenesis-', 'lombok-')): jars.append(p)
    inputs = [ROOT / 'tools/formal-source-junit/validate_client_mysql.py', ROOT / 'docs/phase-1/client-facade-contract.md', Path(__file__).resolve(), ROOT / 'tools/formal-source-junit/SqlMatrixUndoOracle.java', ROOT / 'Cargo.toml', ROOT / 'Cargo.lock',
        *sorted(p for p in (ROOT / 'crates').rglob('*') if p.is_file()),
        *sorted(p for p in (ROOT / 'third_party').rglob('*') if p.is_file()), ROOT / 'tools/formal-source-junit/java_writer_jdbc_profile_tc.py',
        Path(support.__file__), Path(reg.__file__), ROOT / 'tools/formal-tc-lock-coverage/live.py', ROOT / 'tools/formal-rm-java/validate.py',
        ROOT / 'tools/java-go-at-shared-lock/proxy.py', ROOT / 'tools/java-go-at-shared-lock/wire.py', ROOT / 'tools/go-java-v1-real-at/verify.py']
    template_path = ROOT / 'docs/phase-2/evidence/exact-temporal-rust-owner-java/20261128_133635_d5470cdb/application.yml'
    inputs.append(template_path)
    inputs = list(dict.fromkeys(inputs))
    for p in inputs: put(out / 'source' / p.relative_to(ROOT), p.read_bytes())
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    save(out / 'dependencies.json', [dict(path=str(p), sha256=sha(p)) for p in [jar, *jars]])
    classes = work / 'classes'; classes.mkdir(); cp = os.pathsep.join(map(str, jars))
    argfile = work / 'compile.args'; put(argfile, '\n'.join('"' + str(a).replace('\\', '/') + '"' for a in [
        '-encoding', 'UTF-8', '-cp', cp, '-d', classes, ROOT / 'tools/formal-source-junit/SqlMatrixUndoOracle.java']))
    command('compile-oracle', [reg.JAVA.with_name('javac.exe'), '@' + str(argfile)])
    for p in classes.rglob('*.class'): put(out / 'classes' / p.relative_to(classes), p.read_bytes())
    binaries = {}
    for version in versions:
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        command('rustc-' + version, ['rustc', '+' + version + '.0', '--version'])
        args = ['cargo', '+' + version + '.0', 'test', '--manifest-path', ROOT / 'Cargo.toml', '--offline', '--locked', '-p', 'seata-client',
                '--features', 'experimental_changed_commit', '--test', 'live_client_mysql_java']
        command('classify-' + version, [*args, 'rbatis_generated_writes_are_classified_without_interpolating_bindings', '--', '--exact'])
        raw = command('build-' + version, [*args, '--no-run', '--message-format=json'], timeout=240)
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith(b'{')]
        original, = [Path(r['executable']) for r in artifacts if r.get('executable') and r['target']['name'] == 'live_client_mysql_java']
        target = work / ('rust-' + version + '.exe'); put(target, original.read_bytes()); binaries[version] = dict(path=str(target), sha256=sha(target))
        command('clippy-' + version, ['cargo', '+' + version + '.0', 'clippy', '--manifest-path', ROOT / 'Cargo.toml', '--offline', '--locked', '-p', 'seata-client',
            '--features', 'experimental_changed_commit', '--test', 'live_client_mysql_java', '--', '-D', 'warnings'], timeout=240)
    save(out / 'rust-binaries.json', binaries)
    db_port, tc_port = port(), port(); need(db_port != tc_port, 'Port overlap')
    resource = 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/admission'; data = work / 'data'; data.mkdir()
    container = command('create', ['docker', 'create', '--pull=never', '--name', 'sql-matrix-' + stamp.replace('_', '-'), '--label', 'seata.fixture.owner=' + stamp,
        '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql', '-p', '127.0.0.1:' + str(db_port) + ':3306',
        '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD, '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=admission', IMAGE, '--auto-increment-increment=7', '--auto-increment-offset=3']).decode().strip()
    sql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '--default-character-set=utf8mb4', '-h127.0.0.1', '-uroot', '-r', '-N', '-B', 'admission', '-e']
    counts = 'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log WHERE log_status=0)'
    business = 'SELECT JSON_ARRAY(CAST(id AS CHAR),v,note,CAST(amount AS CHAR),HEX(bytes),payload) FROM orm_items ORDER BY id'
    tc = client = relay = None; observations = []
    matrix = json.loads(DATA.read_bytes())
    try:
        command('start', ['docker', 'start', container]); command('ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot', '-p' + PASSWORD, '--wait=60', 'ping'])
        ddl = b''.join(command(label + '-schema', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':' + path]) for label, path in [
            ('server', 'script/server/db/mysql.sql'), ('undo', 'script/client/at/db/mysql.sql')])
        for name in ['001_seata_rs_at_branch_v1.sql', '002_seata_rs_at_registration_origin.sql', '003_seata_rs_at_commit_decision.sql',
            '004_seata_rs_at_first_phase_witness.sql', '005_seata_rs_at_resource_binding.sql', '006_seata_rs_at_commit_attempt.sql',
            '007_seata_rs_at_commit_report_started.sql', '011_seata_rs_at_tinyint_profile_binding.sql', '013_seata_rs_at_undo_checksum.sql', '021_seata_rs_at_jdbc_profile_binding.sql']:
            ddl += (MIGRATIONS / name).read_bytes() + b'\n'
        ddl += ("INSERT INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,X'" + resource.encode().hex() + "',1,1)").encode()
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-uroot', 'admission'], data=ddl)
        template = template_path.read_text()
        import re
        config = re.sub(r'service-port: \d+', 'service-port: ' + str(tc_port), template)
        config = re.sub(r'path: .*?/logs', 'path: ' + work.as_posix() + '/logs', config)
        config = re.sub(r'jdbc:mysql://127\.0\.0\.1:\d+/admission', resource, config)
        put(work / 'application.yml', config); put(out / 'application.yml', config)
        tc_args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx512m', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work),
            '-Dspring.config.location=file:' + (work / 'application.yml').as_posix(), '-Dloader.path=' + next(p for p in jars if p.name == 'mysql-connector-java-8.0.27.jar').as_posix(),
            '-cp', jar, 'org.springframework.boot.loader.launch.PropertiesLauncher', '-h', '127.0.0.1']
        save(out / 'tc-command.json', dict(argv=list(map(str, tc_args)), sha256=sha(jar)))
        tc = support.spawn(tc_args, work, env, out, 'tc'); support.wait_owned(tc, tc_port, env)
        proxy = module('sql_matrix_native_proxy', ROOT / 'tools/java-go-at-shared-lock/proxy.py')
        for version in versions:
            for direction in directions:
                label = version + '-' + direction; folder = out / label; folder.mkdir(); listener = port()
                relay = proxy.WireProxy(folder, listener, tc_port); relay.start()
                args = [Path(binaries[version]['path']), NATIVE, '--ignored', '--exact', '--nocapture']
                save(folder / 'command.json', dict(argv=list(map(str, args)), sha256=binaries[version]['sha256']))
                extra = dict(SEATA_SQL_MATRIX_OUT=str(folder), SEATA_SQL_MATRIX_DIRECTION=direction, SEATA_MYSQL_PORT=str(db_port), SEATA_MYSQL_DATABASE='admission',
                    SEATA_MYSQL_PASSWORD=PASSWORD, SEATA_RM_ENDPOINT='127.0.0.1:' + str(listener), SEATA_TM_ENDPOINT='127.0.0.1:' + str(listener))
                client = support.spawn(args, ROOT, env | extra, folder, 'client')
                for case in matrix['accepted']:
                    id = case['id']; prefix = label + '-' + id
                    wait((folder / (id + '.held.json')).exists, client, prefix + ' phase')
                    phase = json.loads((folder / (id + '.held.json')).read_bytes())
                    rows = [json.loads(line) for line in command(prefix + '-business', sql + [business]).decode().splitlines()]
                    need(rows == case['expected'] == phase['phase_one'], 'Independent SQL matrix phase differs')
                    state = command(prefix + '-tc', sql + ['SELECT xid,branch_id,resource_id,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table ORDER BY table_name,pk'])
                    raw = command(prefix + '-undo', sql + ['SELECT HEX(u.rollback_info),HEX(b.undo_sha256),b.first_phase_committed,b.jdbc_profile FROM undo_log u JOIN seata_rs_at_branch_v1 b ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.log_status=0']).decode().strip().split('\t')
                    undo = (folder / (id + '.undo.json')).read_bytes()
                    need(bytes.fromhex(raw[0]) == undo and raw[1].lower() == hashlib.sha256(undo).hexdigest() and raw[2:] == ['1', '1'], 'Atomic matrix Undo checksum differs')
                    put(folder / (id + '.release'), 'Observed')
                    wait((folder / (id + '.finished.json')).exists, client, prefix + ' finished')
                    wait(lambda: command(prefix + '-terminal', sql + [counts]).strip() == b'0\t0\t0\t0', client, prefix + ' cleanup')
                    final = [json.loads(line) for line in command(prefix + '-final', sql + [business]).decode().splitlines()]
                    need(final == (case['expected'] if direction == 'commit' else matrix['seed']), 'Independent SQL matrix final differs')
                    recovery = command(prefix + '-recovery', sql + ["SELECT phase,terminal,commit_terminal,first_phase_committed,HEX(undo_sha256) FROM seata_rs_at_branch_v1 WHERE xid=X'"
                        + phase['xid'].encode().hex() + "' AND branch_id=" + str(phase['branch_id'])]).decode().strip().split('\t')
                    need(recovery[:4] == (['1','0','1','1'] if direction == 'commit' else ['2','1','0','1'])
                         and recovery[4].lower() == raw[1].lower(), 'Final matrix recovery/checksum differs')
                    fences = command(prefix + '-fences', sql + ['SELECT xid,branch_id,log_status FROM undo_log ORDER BY xid,branch_id']).decode().splitlines()
                    observations.append(dict(version=version, direction=direction, id=id, phase=phase, final=final, fences=fences)); save(out / 'observations.json', observations)
                    put(folder / (id + '.ack'), 'Final observed')
                need(client.wait(timeout=20) == 0, 'Native matrix client failed'); client = None; relay.close(); relay = None
                print('SQL_MATRIX_STREAM_PASS ' + label, flush=True)
        oracle_args = [reg.JAVA, '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Xlog:class+load=info:file=' + (out / 'oracle-class-load.log').as_posix(),
            '-cp', os.pathsep.join([str(classes), cp]), 'SqlMatrixUndoOracle', out]
        command('java-oracle', oracle_args)
        need(len(json.loads((out / 'java-oracle.json').read_bytes())['cases']) == len(observations), 'Original Java parser coverage differs')
        for p in inputs: need(sha(p) == sha(out / 'source' / p.relative_to(ROOT)), 'Source drift during SQL matrix')
        save(out / 'result.json', dict(status='passed', versions=versions, directions=directions, accepted=3, rejected=0, execution_rejected=0,
            globals=len(observations), mocks=False, tc_sha256=sha(jar), scope='unified TM/RM client lifecycle with native rbatis AT CRUD and original Java TC; not full WP07 or production admission'))
    except Exception as error:
        save(out / 'result.json',dict(status='incomplete_evidence',reason=str(error)))
        raise
    finally:
        if relay: relay.close()
        support.stop(client); support.stop(tc); command('stop-mysql', ['docker', 'stop', '--time=15', container], timeout=40)
        save(out / 'catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json']))
    save(out / 'catalog.json', dict(members=[dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    print('CLIENT_MYSQL_PASS ' + str(out), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--versions', nargs='+', choices=['1.99', '1.85'], default=['1.99', '1.85'])
    parser.add_argument('--directions', nargs='+', choices=['commit', 'rollback'], default=['commit', 'rollback'])
    args = parser.parse_args(); run(args.versions, args.directions)
