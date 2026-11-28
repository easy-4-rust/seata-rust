"""Original Java contender versus formally committed Rust DATE/TIME owners."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import uuid
from java_writer_jdbc_profile_tc import ROOT, BASE, GUARD, MIGRATIONS, IMAGE, PASSWORD, module, need, put, reg, save, sha, support, port, wait
import subprocess

ACTOR = ROOT / 'tools/formal-source-junit/JavaExactTemporalContender.java'
BUSINESS = 'SELECT CAST(k AS CHAR),v FROM temporal_dates ORDER BY k; SELECT CAST(k AS CHAR),v FROM temporal_times ORDER BY k'
COUNTS = 'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)'
STATE = 'SELECT xid,branch_id,resource_id,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table ORDER BY table_name,pk'


def run(versions, profiles, directions):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/exact-temporal-rust-owner-java' / stamp
    work = Path('D:/codex-builds') / ('reverse-temporal-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True)
    print('REVERSE_TEMPORAL ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1',
        DOCKER_CONFIG='D:/Temp/seata-docker-config', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'),
        CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'RUSTC', 'RUSTDOC', 'SEATA_CONFIG_NAME', 'SEATA_ENV'):
        env.pop(key, None)
    commands = []

    def command(label, argv, data=None, timeout=150):
        if (out / (label + '.stdout')).exists(): label += '-' + str(len(commands))
        value = subprocess.run(list(map(str, argv)), cwd=work, env=env, capture_output=True, input=data,
                               timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / (label + '.stdout'), value.stdout); put(out / (label + '.stderr'), value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode)); save(out / 'commands.json', commands)
        need(value.returncode == 0, label + ': ' + (value.stdout + value.stderr)[-2000:].decode(errors='replace'))
        return value.stdout

    need(command('java-pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).decode().strip() == reg.PIN, 'Original Java pin differs')
    for label, path in [('session-helper', 'server/src/main/java/org/apache/seata/server/session/SessionHelper.java'),
                        ('default-values', 'common/src/main/java/org/apache/seata/common/DefaultValues.java')]:
        command('original-' + label, ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':' + path])
    jar = reg.ORACLE / 'server/target/seata-server.jar'; need(sha(jar) == reg.JAR_SHA, 'Original TC changed')
    need(sha(BASE / 'catalog.json') == '59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799', 'Baseline changed')
    jars = []
    for row in json.loads((BASE / 'dependencies.json').read_bytes()):
        p = Path(row['path']); need(sha(p) == row['sha256'], 'Java runtime dependency changed')
        if not p.name.startswith(('mockito-', 'byte-buddy-', 'objenesis-', 'lombok-')): jars.append(p)
    sources = [ACTOR, GUARD, *sorted((ROOT / 'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc').glob('*.java'))]
    inputs = [*sources, Path(__file__).resolve(), ROOT / 'Cargo.toml', ROOT / 'Cargo.lock',
        *sorted(p for p in (ROOT / 'crates').rglob('*') if p.is_file()),
        ROOT / 'tools/formal-source-junit/java_writer_jdbc_profile_tc.py',
        ROOT / 'tools/formal-tc-lock-coverage/live.py', Path(support.__file__), Path(reg.__file__),
        ROOT / 'tools/formal-rm-java/validate.py', ROOT / 'tools/java-go-at-shared-lock/proxy.py',
        ROOT / 'tools/java-go-at-shared-lock/wire.py', ROOT / 'tools/go-java-v1-real-at/verify.py']
    inputs = list(dict.fromkeys(inputs))
    for p in inputs: put(out / 'source' / p.relative_to(ROOT), p.read_bytes())
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    save(out / 'dependencies.json', [dict(path=str(p), sha256=sha(p)) for p in [jar, *jars]])
    classes = work / 'classes'; classes.mkdir(); cp = os.pathsep.join(map(str, jars))
    argfile = work / 'compile.args'
    put(argfile, '\n'.join('"' + str(a).replace('\\', '/') + '"' for a in ['-encoding', 'UTF-8', '-cp', cp, '-d', classes, *sources]))
    command('compile-java', [reg.JAVA.with_name('javac.exe'), '@' + str(argfile)])
    for compiled in classes.rglob('*.class'): put(out / 'classes' / compiled.relative_to(classes), compiled.read_bytes())
    java_cp = os.pathsep.join([str(classes), cp]); binaries = {}
    for version in versions:
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        command('rustc-' + version, ['rustc', '+' + version + '.0', '--version'])
        raw = command('cargo-' + version, ['cargo', '+' + version + '.0', 'test', '--manifest-path', ROOT / 'Cargo.toml',
            '--offline', '--locked', '-p', 'seata-db-mysql', '--features', 'experimental_changed_commit',
            '--test', 'live_exact_temporal_rust_owner_java', '--no-run', '--message-format=json'], timeout=240)
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith(b'{')]
        binary, = [Path(r['executable']) for r in artifacts if r.get('executable') and r['target']['name'] == 'live_exact_temporal_rust_owner_java']
        target = work / ('rust-' + version + '.exe'); put(target, binary.read_bytes())
        binaries[version] = dict(path=str(target), sha256=sha(target))
        command('clippy-' + version, ['cargo', '+' + version + '.0', 'clippy', '--manifest-path', ROOT / 'Cargo.toml',
            '--offline', '--locked', '-p', 'seata-db-mysql', '--features', 'experimental_changed_commit',
            '--test', 'live_exact_temporal_rust_owner_java', '--', '-D', 'warnings'], timeout=240)
    save(out / 'rust-binaries.json', binaries)
    db_port, tc_port = port(), port(); need(db_port != tc_port, 'Port overlap')
    resource = 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/admission'
    data = work / 'data'; data.mkdir()
    container = command('create', ['docker', 'create', '--pull=never', '--name', 'reverse-temporal-' + stamp.replace('_', '-'),
        '--label', 'seata.fixture.owner=' + stamp, '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql',
        '-p', '127.0.0.1:' + str(db_port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
        '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=admission', IMAGE]).decode().strip()
    sql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-h127.0.0.1', '-uroot', '-N', '-B', 'admission', '-e']
    tc = rust = java = relay = None; observations = []
    try:
        command('start', ['docker', 'start', container])
        command('ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot', '-p' + PASSWORD, '--connect-timeout=2', '--wait=60', 'ping'])
        ddl = b''
        for label, path in [('server', 'script/server/db/mysql.sql'), ('undo', 'script/client/at/db/mysql.sql')]:
            ddl += command(label + '-schema', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':' + path])
        for name in ['001_seata_rs_at_branch_v1.sql', '002_seata_rs_at_registration_origin.sql', '003_seata_rs_at_commit_decision.sql',
                     '004_seata_rs_at_first_phase_witness.sql', '005_seata_rs_at_resource_binding.sql',
                     '006_seata_rs_at_commit_attempt.sql', '007_seata_rs_at_commit_report_started.sql',
                     '011_seata_rs_at_tinyint_profile_binding.sql', '013_seata_rs_at_undo_checksum.sql', '021_seata_rs_at_jdbc_profile_binding.sql']:
            ddl += (MIGRATIONS / name).read_bytes() + b'\n'
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-uroot', 'admission'], data=ddl)
        server_uuid = command('server-facts', sql + ['SELECT @@server_uuid,VERSION()']).decode().split('\t')[0]
        config = f'''spring:
  main:
    web-application-type: none
logging:
  file:
    path: {work.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: {tc_port}
    enable-check-auth: false
    retry-dead-threshold: 1000
  store:
    mode: db
    session:
      mode: db
    lock:
      mode: db
    db:
      datasource: druid
      db-type: mysql
      driver-class-name: com.mysql.cj.jdbc.Driver
      url: {resource}?useSSL=false&allowPublicKeyRetrieval=true
      user: root
      password: {PASSWORD}
      min-conn: 1
      max-conn: 4
'''
        put(work / 'application.yml', config); put(out / 'application.yml', config)
        tc_args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx512m', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp',
            '-Duser.home=' + str(work), '-Dspring.config.location=file:' + (work / 'application.yml').as_posix(),
            '-Dloader.path=' + next(p for p in jars if p.name == 'mysql-connector-java-8.0.27.jar').as_posix(), '-cp', jar,
            'org.springframework.boot.loader.launch.PropertiesLauncher', '-h', '127.0.0.1']
        save(out / 'tc-command.json', dict(argv=list(map(str, tc_args)), sha256=sha(jar)))
        tc = support.spawn(tc_args, work, env, out, 'tc'); support.wait_owned(tc, tc_port, env)
        proxy = module('reverse_temporal_proxy', ROOT / 'tools/java-go-at-shared-lock/proxy.py')
        for version in versions:
            for profile in profiles:
                for direction in directions:
                    label = version + '-' + str(profile) + '-' + direction
                    case = out / label; case.mkdir(); case_work = work / label; case_work.mkdir()
                    command(label + '-seed', sql + ["DROP TABLE IF EXISTS temporal_dates,temporal_times;"
                        "CREATE TABLE temporal_dates(k DATE PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO temporal_dates VALUES('1582-10-05',10);"
                        "CREATE TABLE temporal_times(k TIME(6) PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO temporal_times VALUES('-25:01:02.123456',20);"
                        "REPLACE INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,X'" + resource.encode().hex() + "'," + str(profile % 2) + ',' + str(profile) + ')'])
                    before = command(label + '-before', sql + [BUSINESS])
                    listener = port(); relay = proxy.WireProxy(case, listener, tc_port); relay.start()
                    rust_out = case / 'rust'; rust_out.mkdir(); java_out = case / 'java'; java_out.mkdir()
                    binary = Path(binaries[version]['path']); need(sha(binary) == binaries[version]['sha256'], 'Rust binary changed')
                    args = [binary, 'original_rust_owner_excludes_java_exact_temporal_writer', '--ignored', '--exact', '--nocapture']
                    extra = dict(SEATA_REVERSE_TEMPORAL_OUT=str(rust_out), SEATA_REVERSE_TEMPORAL_PROFILE=str(profile),
                        SEATA_REVERSE_TEMPORAL_DIRECTION=direction, SEATA_MYSQL_PORT=str(db_port), SEATA_MYSQL_DATABASE='admission',
                        SEATA_MYSQL_PASSWORD=PASSWORD, SEATA_RM_ENDPOINT='127.0.0.1:' + str(listener), SEATA_TM_ENDPOINT='127.0.0.1:' + str(listener))
                    save(rust_out / 'command.json', dict(argv=list(map(str, args)), sha256=sha(binary)))
                    rust = support.spawn(args, ROOT, env | extra, rust_out, 'client')
                    wait((rust_out / 'owner-held.json').exists, rust, label + ' Rust owner')
                    owner = json.loads((rust_out / 'owner-held.json').read_bytes())
                    held = command(label + '-held-business', sql + [BUSINESS]); need(held == b'1582-10-05\t20\n-25:01:02.123456\t30\n', 'Rust phase one differs')
                    locks = command(label + '-held-tc', sql + [STATE])
                    undo_query = "SELECT HEX(rollback_info),HEX(b.undo_sha256),b.jdbc_profile,b.first_phase_committed,u.log_status FROM undo_log u JOIN seata_rs_at_branch_v1 b ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.xid='" + owner['xid'] + "' AND u.branch_id=" + str(owner['branch_id'])
                    raw = command(label + '-held-undo', sql + [undo_query]).decode().strip().split('\t')
                    need(len(raw) == 5 and raw[1].lower() == hashlib.sha256(bytes.fromhex(raw[0])).hexdigest()
                         and raw[2:] == [str(profile), '1', '0'], 'Rust Undo checksum/profile not persisted')
                    put(case / 'rust-original-undo.json', bytes.fromhex(raw[0]))
                    save(case_work / 'database.json', dict(profile=profile, resource=resource, endpoint='127.0.0.1:' + str(listener),
                        server_uuid=server_uuid, schema='admission', password=PASSWORD))
                    args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx256m', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp',
                        '-Duser.home=' + str(case_work), '-Xlog:class+load=info:file=' + (java_out / 'class-load.log').as_posix(),
                        '-cp', java_cp, 'JavaExactTemporalContender', case_work]
                    save(java_out / 'command.json', dict(argv=list(map(str, args))))
                    java = support.spawn(args, case_work, env, java_out, 'client')
                    wait((case_work / 'rejected.json').exists, java, label + ' Java conflicts')
                    put(java_out / 'rejected.json', (case_work / 'rejected.json').read_bytes())
                    need(command(label + '-rejected-business', sql + [BUSINESS]) == held, 'Rejected Java leaked changes')
                    need(command(label + '-rejected-tc', sql + [STATE]) == locks, 'Rejected Java changed Rust locks')
                    need(command(label + '-rejected-undo', sql + [undo_query]).decode().strip().split('\t') == raw, 'Rejected Java changed Rust Undo')
                    put(rust_out / 'owner-release', 'Java conflict observed')
                    wait((rust_out / 'owner-finished.json').exists, rust, label + ' Rust terminal')
                    wait(lambda: command(label + '-owner-terminal', sql + [COUNTS]).strip() == b'0\t0\t0\t0', rust, label + ' Rust cleanup')
                    outcome = command(label + '-owner-business', sql + [BUSINESS]); need(outcome == (held if direction == 'commit' else before), 'Rust completion values differ')
                    put(case_work / 'owner-released', 'Rust terminal observed')
                    for name in ['date', 'time']:
                        wait((case_work / ('admitted-' + name + '.json')).exists, java, label + ' Java admitted ' + name)
                        admission = json.loads((case_work / ('admitted-' + name + '.json')).read_bytes())
                        save(java_out / ('admitted-' + name + '.json'), admission)
                        put(java_out / ('undo-' + name + '.json'), bytes.fromhex(admission['undo_hex']))
                        command(label + '-admitted-' + name + '-tc', sql + [STATE])
                        command(label + '-admitted-' + name + '-business', sql + [BUSINESS])
                        put(case_work / ('admitted-' + name + '.release'), 'Java phase observed')
                        wait((case_work / ('finished-' + name)).exists, java, label + ' Java rollback ' + name)
                        wait(lambda: command(label + '-' + name + '-terminal', sql + [COUNTS]).strip() == b'0\t0\t0\t0', java, label + ' Java cleanup')
                        need(command(label + '-' + name + '-final', sql + [BUSINESS]) == outcome, 'Java compensation changed Rust outcome')
                        put(case_work / ('finished-' + name + '.ack'), 'Java terminal observed')
                    wait((case_work / 'finished').exists, java, label + ' Java finished')
                    relay.close(); relay = None
                    put(case_work / 'stop', 'Observed'); need(java.wait(timeout=10) == 0, 'Java exit failed'); java = None
                    put(rust_out / 'stop', 'Observed'); need(rust.wait(timeout=10) == 0, 'Rust exit failed'); rust = None
                    need(b'test result: ok. 1 passed;' in (rust_out / 'client-stdout.log').read_bytes(), 'Rust native test missing')
                    observations.append(dict(version=version, profile=profile, direction=direction, owner=owner, rejected=2, admitted=2))
                    save(out / 'observations.json', observations); print('REVERSE_CASE_PASS ' + label, flush=True)
        for p in inputs: need(sha(p) == sha(out / 'source' / p.relative_to(ROOT)), 'Source drift during reverse fixture')
        save(out / 'result.json', dict(status='passed', versions=versions, profiles=profiles, directions=directions,
            cases=len(observations), java_rejected=2 * len(observations), java_admitted=2 * len(observations),
            mocks=False, tc_sha256=sha(jar), fixture_auth='owned loopback TC auth disabled; root fixture account'))
    finally:
        if relay: relay.close()
        support.stop(java); support.stop(rust); support.stop(tc)
        command('stop-mysql', ['docker', 'stop', '--time=15', container], timeout=40)
    save(out / 'catalog.json', dict(members=[dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
        for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    print('REVERSE_TEMPORAL_PASS ' + str(out), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--versions', nargs='+', choices=['1.99', '1.85'], default=['1.99', '1.85'])
    parser.add_argument('--profiles', nargs='+', type=int, choices=[5, 6], default=[5, 6])
    parser.add_argument('--directions', nargs='+', choices=['commit', 'rollback'], default=['commit', 'rollback'])
    args = parser.parse_args(); run(args.versions, args.profiles, args.directions)
