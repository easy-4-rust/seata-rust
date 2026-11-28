"""Original Java TC/RM integration for the explicit complete JDBC admission entry."""
import argparse
import datetime
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/formal-tc-lock-coverage'))
from live import IMAGE, PASSWORD, module, need, put, reg, sha, support

BASE = ROOT / 'docs/phase-2/evidence/java-writer-jdbc-profile/20261128_124805_b9ba7380'
ACTOR = ROOT / 'tools/formal-source-junit/JavaWriterJdbcProfileActor.java'
GUARD = ROOT / 'compatibility/seata-java/writer-admission/src/main/java/io/seatarust/compat/mysql/JavaAtWriterAdmission.java'
MIGRATIONS = ROOT / 'crates/seata_db_mysql/migrations'


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2), encoding='utf-8')


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); return sock.getsockname()[1]


def wait(check, process, detail, seconds=90):
    deadline = time.monotonic() + seconds
    while not check():
        need(process.poll() is None, detail + ': process exited')
        need(time.monotonic() < deadline, detail + ': deadline exceeded')
        time.sleep(.1)


def run(profiles, directions):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/java-writer-jdbc-profile-tc' / stamp
    work = Path('D:/codex-builds') / ('java-writer-profile-tc-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True)
    print('JAVA_WRITER_PROFILE_TC ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1',
                            DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'SEATA_CONFIG_NAME', 'SEATA_ENV'):
        env.pop(key, None)
    commands = []

    def command(label, argv, expected=0, data=None, timeout=120):
        if (out / (label + '.stdout')).exists():
            label += '-' + str(len(commands))
        value = subprocess.run(list(map(str, argv)), cwd=work, env=env, input=data, capture_output=True,
                               timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / (label + '.stdout'), value.stdout); put(out / (label + '.stderr'), value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode))
        save(out / 'commands.json', commands)
        if expected is not None:
            need(value.returncode == expected, label + ': ' + (value.stdout + value.stderr)[-2500:].decode(errors='replace'))
        return value

    need(command('java-pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).stdout.decode().strip() == reg.PIN,
         'Original Java pin differs')
    jar = reg.ORACLE / 'server/target/seata-server.jar'
    need(sha(jar) == reg.JAR_SHA, 'Original Java TC differs')
    need(sha(BASE / 'catalog.json') == '59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799',
         'Dependency packet differs')
    jars = []
    for row in json.loads((BASE / 'dependencies.json').read_bytes()):
        path = Path(row['path']); need(sha(path) == row['sha256'], 'Runtime dependency differs')
        if not path.name.startswith(('mockito-', 'byte-buddy-', 'objenesis-', 'lombok-')):
            jars.append(path)
    adapter = ROOT / 'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc'
    sources = [ACTOR, GUARD, *sorted(adapter.glob('*.java'))]
    inputs = [*sources, Path(__file__).resolve(), ROOT / 'tools/java-go-at-shared-lock/proxy.py',
              ROOT / 'tools/java-go-at-shared-lock/wire.py', ROOT / 'tools/go-java-v1-real-at/verify.py',
              *[MIGRATIONS / name for name in ('005_seata_rs_at_resource_binding.sql',
                 '011_seata_rs_at_tinyint_profile_binding.sql', '021_seata_rs_at_jdbc_profile_binding.sql')]]
    for source in inputs:
        put(out / 'source' / source.relative_to(ROOT), source.read_bytes())
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    save(out / 'dependencies.json', [dict(path=str(p), sha256=sha(p)) for p in [jar, *jars]])
    classes = work / 'classes'; classes.mkdir()
    cp = os.pathsep.join(map(str, jars))
    argfile = work / 'compile.args'
    put(argfile, '\n'.join('"' + str(arg).replace('\\', '/') + '"' for arg in [
        '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *sources]))
    command('compile', [reg.JAVA.with_name('javac.exe'), '@' + str(argfile)])
    for compiled in classes.rglob('*.class'):
        put(out / 'classes' / compiled.relative_to(classes), compiled.read_bytes())
    actor_cp = os.pathsep.join([str(classes), cp])
    db_port, tc_port = port(), port()
    need(db_port != tc_port, 'Port overlap')
    data = work / 'data'; data.mkdir()
    container = command('create', ['docker', 'create', '--pull=never', '--name', 'java-profile-tc-' + stamp.replace('_', '-'),
        '--label', 'seata.fixture.owner=' + stamp, '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql',
        '-p', '127.0.0.1:' + str(db_port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
        '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=admission', IMAGE]).stdout.decode().strip()
    sql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-h127.0.0.1', '-uroot', '-N', '-B', 'admission', '-e']
    resource = 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/admission'
    tc = None; actor = None; relay = None
    observations = []
    try:
        command('start', ['docker', 'start', container])
        command('ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot', '-p' + PASSWORD,
                          '--connect-timeout=2', '--wait=60', 'ping'], timeout=150)
        server_schema = command('server-schema', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':script/server/db/mysql.sql']).stdout
        undo_schema = command('undo-schema', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':script/client/at/db/mysql.sql']).stdout
        ddl = server_schema + undo_schema + '\n'.join((MIGRATIONS / name).read_text(encoding='utf-8') for name in (
            '005_seata_rs_at_resource_binding.sql', '011_seata_rs_at_tinyint_profile_binding.sql',
            '021_seata_rs_at_jdbc_profile_binding.sql')).encode()
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-uroot', 'admission'], data=ddl)
        server_uuid = command('server-facts', sql + ['SELECT @@server_uuid,VERSION(),@@global.sql_mode']).stdout.decode().split('\t')[0]
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
        tc_args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx512m', '-Duser.timezone=UTC',
            '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work),
            '-Dspring.config.location=file:' + (work / 'application.yml').as_posix(), '-cp', jar,
            'org.springframework.boot.loader.launch.JarLauncher', '-h', '127.0.0.1']
        save(out / 'tc-command.json', dict(argv=list(map(str, tc_args)), jar_sha256=sha(jar)))
        tc = support.spawn(tc_args, work, env, out, 'tc'); support.wait_owned(tc, tc_port, env)
        proxy_module = module('java_full_profile_proxy', ROOT / 'tools/java-go-at-shared-lock/proxy.py')
        decoder = module('java_full_profile_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
        for profile in profiles:
            for direction in directions:
                label = str(profile) + '-' + direction
                case = out / label; case.mkdir(); case_work = work / label; case_work.mkdir()
                seed = '''DROP TABLE IF EXISTS admission_business,temporal_values,temporal_dates,temporal_times;
CREATE TABLE admission_business(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
INSERT INTO admission_business VALUES(1,0);
CREATE TABLE temporal_values(id INT PRIMARY KEY,d DATE,t TIME(6)) ENGINE=InnoDB;
INSERT INTO temporal_values VALUES(1,'1582-10-05','-00:00:00.000001'),(2,'9999-12-31','838:59:59');
CREATE TABLE temporal_dates(k DATE PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
INSERT INTO temporal_dates VALUES('1582-10-05',10);
CREATE TABLE temporal_times(k TIME(6) PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
INSERT INTO temporal_times VALUES('-25:01:02.123456',20);
REPLACE INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,X'%s',%s,%s);''' % (resource.encode().hex(), profile % 2, profile)
                command(label + '-seed', sql + [seed])
                business = "SELECT id,v FROM admission_business ORDER BY id; SELECT id,CAST(d AS CHAR),CAST(t AS CHAR) FROM temporal_values ORDER BY id; SELECT CAST(k AS CHAR),v FROM temporal_dates ORDER BY k; SELECT CAST(k AS CHAR),v FROM temporal_times ORDER BY k"
                before = command(label + '-before', sql + [business]).stdout
                listener = port(); relay = proxy_module.WireProxy(case, listener, tc_port); relay.start()
                save(case_work / 'database.json', dict(profile=profile, direction=direction, resource=resource,
                    endpoint='127.0.0.1:' + str(listener), server_uuid=server_uuid, schema='admission', password=PASSWORD))
                put(case / 'database.json', (case_work / 'database.json').read_bytes())
                args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx256m', '-Duser.timezone=UTC',
                    '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(case_work),
                    '-Xlog:class+load=info:file=' + (case / 'class-load.log').as_posix(), '-cp', actor_cp,
                    'JavaWriterJdbcProfileActor', case_work]
                save(case / 'client-command.json', dict(argv=list(map(str, args))))
                actor = support.spawn(args, case_work, env, case, 'client')
                wait((case_work / 'admission-held').exists, actor, label + ' admission')
                rejects = json.loads((case_work / 'rejections.json').read_bytes())
                save(case / 'rejections.json', rejects)
                need({r['bound'] for r in rejects} == set(range(1, 7)) - {profile}
                     and all(r['actual'] == profile and r['reason'] == 'jdbc-profile' for r in rejects), 'Mismatch rejection differs')
                with relay.lock:
                    early = [dict(r) for r in relay.events]
                save(case / 'pre-begin-frames.json', early)
                need(not any(decoder.decode(r)['code'] in (1, 7, 9, 11, 13) for r in early if r['event'] == 'wire'),
                     'Rejected writer reached Begin/Register')
                counts = 'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)'
                need(command(label + '-rejected-counts', sql + [counts]).stdout.strip() == b'0\t0\t0\t0', 'Rejected writer persisted state')
                need(command(label + '-rejected-business', sql + [business]).stdout == before, 'Rejected writer changed business')
                blocked = command(label + '-binding-held', sql + ['SET innodb_lock_wait_timeout=1; UPDATE seata_rs_at_resource_binding SET jdbc_profile=NULL WHERE slot=1'], expected=None)
                need(blocked.returncode != 0 and b'1205' in blocked.stderr, 'Original Java transaction did not hold binding')
                put(case_work / 'admission-release', 'Lock observed')
                wait((case_work / 'phase-one.json').exists, actor, label + ' phase one')
                phase = json.loads((case_work / 'phase-one.json').read_bytes()); save(case / 'phase-one.json', phase)
                need(phase['process_id'] == actor.pid and phase['resource'] == resource and phase['profile'] == profile,
                     'Original Java identity differs')
                need('serializer=jackson' in phase['undo_context'], 'Unexpected Undo serializer')
                undo = bytes.fromhex(phase['undo_hex']); put(case / 'original-undo.json', undo)
                graph = json.loads(undo)
                need(graph['xid'] == phase['xid'] and graph['branchId'] == phase['branch_id'], 'Original Undo identity differs')
                expected_logs = 7 if profile >= 5 else 1
                need(len(graph['sqlUndoLogs']) == expected_logs, 'Original SQL Undo count differs')
                phase_business = command(label + '-phase-business', sql + [business]).stdout
                command(label + '-phase-tc', sql + ['SELECT xid,branch_id,resource_id,lock_key,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table ORDER BY table_name,pk'])
                need(command(label + '-binding-released', sql + ['SET innodb_lock_wait_timeout=1; UPDATE seata_rs_at_resource_binding SET jdbc_profile=jdbc_profile WHERE slot=1; SELECT jdbc_profile FROM seata_rs_at_resource_binding']).stdout.strip() == str(profile).encode(), 'Binding not released after original commit')
                put(case_work / 'decision-release', direction)
                wait((case_work / 'finished').exists, actor, label + ' decision')
                wait(lambda: command(label + '-terminal-poll', sql + [counts]).stdout.strip() == b'0\t0\t0\t0',
                     actor, label + ' terminal', seconds=90)
                final = command(label + '-final-business', sql + [business]).stdout
                need(final == (before if direction == 'rollback' else phase_business), 'Original callback changed exact business values')
                relay.close(); relay = None
                frames = json.loads((case / 'proxy-events.json').read_bytes())
                decoded = [(r['direction'], decoder.decode(r)) for r in frames if r['event'] == 'wire']
                registers = [r for d, r in decoded if d == 'sdk-to-tc' and r['code'] == 11]
                need(len(registers) == 1 and registers[0]['xid'] == phase['xid'] and registers[0]['resource'] == resource, 'Original register differs')
                callback_code, reply_code, status = (5, 6, 5) if direction == 'rollback' else (3, 4, 3)
                callbacks = [r for d, r in decoded if d == 'tc-to-sdk' and r['code'] == callback_code]
                replies = [r for d, r in decoded if d == 'sdk-to-tc' and r['code'] == reply_code]
                need(len(callbacks) >= 1 and len(replies) >= 1 and all(r['xid'] == phase['xid'] and r['branch_id'] == phase['branch_id'] for r in callbacks + replies)
                     and any(r['result'] == 1 and r['status'] == status for r in replies), 'Original RM callback missing or unsuccessful')
                put(case_work / 'stop', 'Observation complete'); need(actor.wait(timeout=10) == 0, 'Java actor stop failed'); actor = None
                observations.append(dict(profile=profile, direction=direction, xid=phase['xid'], branch_id=phase['branch_id'],
                    rejected=5, sql_undo_logs=expected_logs, callback_status=status, terminal_counts=[0, 0, 0, 0]))
                save(out / 'observations.json', observations)
                print('ORIGINAL_JAVA_PROFILE_CASE_PASS ' + label, flush=True)
        for source in inputs:
            need(sha(source) == sha(out / 'source' / source.relative_to(ROOT)), 'Source drift during native integration')
        save(out / 'result.json', dict(status='passed', profiles=profiles, directions=directions,
            cases=len(observations), rejected=5 * len(observations), mocks=False, tc_jar_sha256=sha(jar),
            fixture_auth='disabled on owned loopback TC; not production authentication certification'))
    finally:
        if relay: relay.close()
        support.stop(actor); support.stop(tc)
        command('stop-mysql', ['docker', 'stop', '--time=15', container], timeout=40)
    save(out / 'catalog.json', dict(members=[dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
        for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    print('JAVA_WRITER_PROFILE_TC_PASS ' + str(out), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--profiles', nargs='+', type=int, default=list(range(1, 7)))
    parser.add_argument('--directions', nargs='+', choices=['commit', 'rollback'], default=['commit', 'rollback'])
    options = parser.parse_args(); need(all(1 <= p <= 6 for p in options.profiles), 'Invalid profile')
    run(options.profiles, options.directions)
