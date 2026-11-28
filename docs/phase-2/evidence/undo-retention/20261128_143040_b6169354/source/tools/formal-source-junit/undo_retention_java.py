"""Owned original TC schedule and real Java/MySQL Undo retention differential."""
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid
from java_writer_jdbc_profile_tc import ROOT, BASE, MIGRATIONS, IMAGE, PASSWORD, module, need, put, reg, save, sha, support, port, wait

NATIVE = 'original_tc_cleanup_preserves_channel_and_matches_retention_rows'
KEEP = ['keep-micro', 'keep-noon', 'keep-normal', 'keep-young-fence']


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/undo-retention' / stamp
    work = Path('D:/codex-builds') / ('undo-retention-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True); print('UNDO_RETENTION ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', DOCKER_CONFIG='D:/Temp/seata-docker-config',
        CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2', SEATA_MYSQL_PASSWORD=PASSWORD)
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'RUSTC', 'RUSTDOC', 'SEATA_CONFIG_NAME', 'SEATA_ENV'): env.pop(key, None)
    commands = []

    def command(label, args, data=None, timeout=180):
        label += '-' + str(len(commands))
        p = subprocess.run(list(map(str, args)), cwd=work, env=env, input=data, capture_output=True, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / (label + '.stdout'), p.stdout); put(out / (label + '.stderr'), p.stderr)
        commands.append(dict(label=label, argv=list(map(str, args)), exit_code=p.returncode)); save(out / 'commands.json', commands)
        need(p.returncode == 0, label + ': ' + (p.stdout + p.stderr)[-3500:].decode(errors='replace')); return p.stdout

    jar = reg.ORACLE / 'server/target/seata-server.jar'
    need(command('pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).decode().strip() == reg.PIN and sha(jar) == reg.JAR_SHA, 'Java original differs')
    need(sha(BASE / 'catalog.json') == '59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799', 'Dependency baseline differs')
    jars = []
    for row in json.loads((BASE / 'dependencies.json').read_bytes()):
        p = Path(row['path']); need(sha(p) == row['sha256'], 'Dependency differs')
        if not p.name.startswith(('mockito-', 'byte-buddy-', 'objenesis-', 'lombok-')): jars.append(p)
    template_path = ROOT / 'docs/phase-2/evidence/exact-temporal-rust-owner-java/20261128_133635_d5470cdb/application.yml'
    inputs = [Path(__file__).resolve(), ROOT / 'tools/formal-source-junit/UndoRetentionJavaOracle.java', ROOT / 'Cargo.toml', ROOT / 'Cargo.lock',
        *sorted(p for p in (ROOT / 'crates').rglob('*') if p.is_file()), template_path,
        ROOT / 'tools/formal-source-junit/java_writer_jdbc_profile_tc.py', Path(support.__file__), Path(reg.__file__),
        ROOT / 'tools/formal-tc-lock-coverage/live.py', ROOT / 'tools/formal-rm-java/validate.py',
        ROOT / 'tools/java-go-at-shared-lock/proxy.py', ROOT / 'tools/java-go-at-shared-lock/wire.py', ROOT / 'tools/go-java-v1-real-at/verify.py']
    inputs = list(dict.fromkeys(inputs))
    for p in inputs: put(out / 'source' / p.relative_to(ROOT), p.read_bytes())
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    save(out / 'dependencies.json', [dict(path=str(p), sha256=sha(p)) for p in [jar, *jars]])
    for name, path in [('handler', 'rm-datasource/src/main/java/org/apache/seata/rm/RMHandlerAT.java'),
                       ('mysql', 'rm-datasource/src/main/java/org/apache/seata/rm/datasource/undo/mysql/MySQLUndoLogManager.java'),
                       ('date', 'common/src/main/java/org/apache/seata/common/util/DateUtil.java'),
                       ('coordinator', 'server/src/main/java/org/apache/seata/server/coordinator/DefaultCoordinator.java')]:
        command('source-' + name, ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':' + path])
    classes = work / 'classes'; classes.mkdir(); cp = os.pathsep.join(map(str, jars)); argfile = work / 'compile.args'
    put(argfile, '\n'.join('"' + str(a).replace('\\', '/') + '"' for a in ['-encoding', 'UTF-8', '-cp', cp, '-d', classes, ROOT / 'tools/formal-source-junit/UndoRetentionJavaOracle.java']))
    command('javac', [reg.JAVA.with_name('javac.exe'), '@' + str(argfile)])
    for p in classes.rglob('*.class'): put(out / 'classes' / p.relative_to(classes), p.read_bytes())
    binaries = {}
    for version in ('1.99', '1.85'):
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        cargo = ['cargo', '+' + version + '.0']; base = ['--manifest-path', ROOT / 'Cargo.toml', '--offline', '--locked']
        command('rustc-' + version, ['rustc', '+' + version + '.0', '--version'])
        command('rm-tests-' + version, [*cargo, 'test', *base, '-p', 'seata-rm'], timeout=300)
        command('db-tests-' + version, [*cargo, 'test', *base, '-p', 'seata-db-mysql', '--lib'], timeout=300)
        command('clippy-' + version, [*cargo, 'clippy', *base, '-p', 'seata-rm', '-p', 'seata-db-mysql', '--all-targets', '--features', 'experimental_changed_commit', '--', '-D', 'warnings'], timeout=300)
        raw = command('build-' + version, [*cargo, 'test', *base, '-p', 'seata-db-mysql', '--test', 'live_undo_retention_java', '--no-run', '--message-format=json'], timeout=300)
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith(b'{')]
        original, = [Path(r['executable']) for r in artifacts if r.get('executable') and r['target']['name'] == 'live_undo_retention_java']
        target = work / ('rust-' + version + '.exe'); put(target, original.read_bytes()); binaries[version] = dict(path=str(target), sha256=sha(target))
    save(out / 'rust-binaries.json', binaries)
    db_port, tc_port = port(), port(); data = work / 'data'; data.mkdir(); need(db_port != tc_port, 'Port overlap')
    container = command('create', ['docker', 'create', '--pull=never', '--name', 'undo-retention-' + stamp.replace('_', '-'), '--label', 'seata.fixture.owner=' + stamp,
        '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql', '-p', '127.0.0.1:' + str(db_port) + ':3306',
        '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD, '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=tc_store', IMAGE]).decode().strip()
    sql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-h127.0.0.1', '-uroot', '-r', '-N', '-B', '-e']
    def query(label, schema, text): return command(label, sql + ['USE ' + schema + '; ' + text])
    def seed(schema):
        query('seed-' + schema, schema, """TRUNCATE undo_log; SET SESSION cte_max_recursion_depth=7000;
INSERT INTO undo_log(branch_id,xid,context,rollback_info,log_status,log_created,log_modified)
WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM s WHERE n<6105)
SELECT n,CONCAT('expired-',LPAD(n,5,'0')),'legacy',X'0102',MOD(n,2),UTC_DATE()-INTERVAL 8 DAY,UTC_TIMESTAMP(6) FROM s;
INSERT INTO undo_log(branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES
(70001,'boundary-normal','legacy',X'0102',0,UTC_DATE()-INTERVAL 7 DAY,UTC_TIMESTAMP(6)),
(70002,'boundary-fence','legacy',X'0102',1,UTC_DATE()-INTERVAL 7 DAY,UTC_TIMESTAMP(6)),
(80001,'keep-micro','legacy',X'0102',0,UTC_DATE()-INTERVAL 7 DAY+INTERVAL 1 MICROSECOND,UTC_TIMESTAMP(6)),
(80002,'keep-noon','legacy',X'0102',1,UTC_DATE()-INTERVAL 7 DAY+INTERVAL 12 HOUR,UTC_TIMESTAMP(6)),
(80003,'keep-normal','legacy',X'0102',0,UTC_DATE(),UTC_TIMESTAMP(6)),
(80004,'keep-young-fence','legacy',X'0102',1,UTC_DATE(),UTC_TIMESTAMP(6)); SELECT COUNT(*) FROM undo_log;""")
    tc = None; clients = []; relays = []; observations = []
    try:
        command('start', ['docker', 'start', container]); command('mysql-ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot', '-p' + PASSWORD, '--wait=60', 'ping'])
        server = command('server-ddl', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':script/server/db/mysql.sql'])
        undo = command('undo-ddl', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':script/client/at/db/mysql.sql'])
        ddl = b'USE tc_store;\n' + server
        for schema in ('cleanup_java', 'cleanup_199', 'cleanup_185'):
            ddl += ('CREATE DATABASE ' + schema + '; USE ' + schema + ';\n').encode() + undo
            if schema != 'cleanup_java':
                for name in ('001_seata_rs_at_branch_v1.sql', '002_seata_rs_at_registration_origin.sql', '003_seata_rs_at_commit_decision.sql', '004_seata_rs_at_first_phase_witness.sql', '005_seata_rs_at_resource_binding.sql', '006_seata_rs_at_commit_attempt.sql', '007_seata_rs_at_commit_report_started.sql', '011_seata_rs_at_tinyint_profile_binding.sql', '013_seata_rs_at_undo_checksum.sql', '021_seata_rs_at_jdbc_profile_binding.sql'):
                    ddl += (MIGRATIONS / name).read_bytes() + b'\n'
                resource = 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/' + schema
                ddl += ("CREATE TABLE cleanup_probe(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO cleanup_probe VALUES(1,10); INSERT INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,X'" + resource.encode().hex() + "',1,2);\n").encode()
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-uroot'], data=ddl)
        for days in (7, 0, -1):
            seed('cleanup_java'); result = out / ('java-' + str(days) + '.json')
            command('java-oracle', [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work), '-cp', os.pathsep.join([str(classes), cp]),
                'UndoRetentionJavaOracle', 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/cleanup_java', str(days), result])
            need(json.loads(result.read_bytes())['batches'] == [3000, 3000, 107], 'Original Java batch semantics differ')
            need(query('java-kept', 'cleanup_java', 'SELECT xid FROM undo_log ORDER BY xid').decode().splitlines() == KEEP, 'Java retention boundary differs')
        template = template_path.read_text(); config = re.sub(r'service-port: \d+', 'service-port: ' + str(tc_port), template)
        config = re.sub(r'path: .*?/logs', 'path: ' + work.as_posix() + '/logs', config)
        config = re.sub(r'jdbc:mysql://127\.0\.0\.1:\d+/admission', 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/tc_store', config)
        config = config.replace('retry-dead-threshold: 1000', 'retry-dead-threshold: 1000\n    undo:\n      log-delete-period: 1000\n      log-save-days: 7')
        put(work / 'application.yml', config); put(out / 'application.yml', config)
        tc_args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx512m', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work),
            '-Dspring.config.location=file:' + (work / 'application.yml').as_posix(), '-Dloader.path=' + next(p for p in jars if p.name == 'mysql-connector-java-8.0.27.jar').as_posix(),
            '-cp', jar, 'org.springframework.boot.loader.launch.PropertiesLauncher', '-h', '127.0.0.1']
        save(out / 'tc-command.json', dict(argv=list(map(str, tc_args)), sha256=sha(jar)))
        tc = support.spawn(tc_args, work, env, out, 'tc'); support.wait_owned(tc, tc_port, env)
        proxy = module('retention_proxy', ROOT / 'tools/java-go-at-shared-lock/proxy.py')
        def requests(relay):
            with relay.lock: return [dict(r) for r in relay.events if r['event'] == 'wire' and r['direction'] == 'tc-to-sdk' and r.get('payload_type') == 111]
        for version, schema in [('1.99', 'cleanup_199'), ('1.85', 'cleanup_185')]:
            seed(schema); folder = out / version; folder.mkdir(); listener = port()
            relay = proxy.WireProxy(folder, listener, tc_port); relay.start(); relays.append(relay)
            args = [binaries[version]['path'], NATIVE, '--ignored', '--exact', '--nocapture']
            extra = dict(SEATA_UNDO_RETENTION_OUT=str(folder), SEATA_MYSQL_PORT=str(db_port), SEATA_MYSQL_DATABASE=schema, SEATA_RM_ENDPOINT='127.0.0.1:' + str(listener), SEATA_TM_ENDPOINT='127.0.0.1:' + str(listener))
            save(folder / 'command.json', dict(argv=args, environment=extra, sha256=binaries[version]['sha256']))
            client = support.spawn(args, ROOT, env | extra, folder, 'client'); clients.append((schema, folder, client, relay))
            wait((folder / 'ready.json').exists, client, version + ' ready')
        print('WAITING_ORIGINAL_TC_INITIAL_180_SECOND_SCHEDULE', flush=True)
        for schema, folder, client, relay in clients:
            wait(lambda: len(requests(relay)) >= 1, client, schema + ' original TC cleanup', seconds=240)
            time.sleep(.4)
            need(query('binding-rejected', schema, 'SELECT COUNT(*) FROM undo_log').strip() == b'6111', 'Mismatched binding deleted Undo')
            save(folder / 'binding-rejected.json', dict(count=6111, frames=requests(relay)))
            query('binding-correct', schema, 'UPDATE seata_rs_at_resource_binding SET jdbc_profile=1 WHERE slot=1')
            wait((folder / 'cleaned.json').exists, client, schema + ' cleanup', seconds=40)
            need(query('rust-kept', schema, 'SELECT xid FROM undo_log ORDER BY xid').decode().splitlines() == KEEP, 'Rust retention differs')
            query('trigger-install', schema, "CREATE TRIGGER retention_block BEFORE DELETE ON undo_log FOR EACH ROW SET @retention_block=1; INSERT INTO undo_log(branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES(90001,'blocked','legacy',X'0102',0,UTC_DATE()-INTERVAL 8 DAY,UTC_TIMESTAMP(6))")
            previous = len(requests(relay)); wait(lambda: len(requests(relay)) > previous, client, schema + ' trigger refusal'); time.sleep(.4)
            need(query('trigger-rejected', schema, "SELECT COUNT(*) FROM undo_log WHERE xid='blocked'").strip() == b'1', 'Trigger guard did not reject')
            save(folder / 'trigger-rejected.json', dict(count=1, frames=requests(relay)))
            query('trigger-drop', schema, 'DROP TRIGGER retention_block')
            wait(lambda: query('trigger-released', schema, "SELECT COUNT(*) FROM undo_log WHERE xid='blocked'").strip() == b'0', client, schema + ' cleanup retry')
            put(folder / 'probe', 'Run real registered rollback after cleanup')
            wait((folder / 'probe.json').exists, client, schema + ' original callback', seconds=100)
            need(query('probe-value', schema, 'SELECT v FROM cleanup_probe').strip() == b'10', 'Business changed')
            wait(lambda: query('tc-terminal', 'tc_store', 'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table)').strip() == b'0\t0\t0', client, 'TC terminal')
            observations.append(dict(version=folder.name, schema=schema, expired_deleted=6107, remaining=KEEP, binding_rejected=True, trigger_rejected=True, probe=json.loads((folder / 'probe.json').read_bytes())))
        for relay in relays: relay.close()
        relays.clear()
        for schema, folder, client, relay in clients:
            put(folder / 'stop', 'Independent observation complete'); need(client.wait(timeout=15) == 0, 'Rust test failed')
            need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Rust test did not run')
        for p in inputs: need(sha(p) == sha(out / 'source' / p.relative_to(ROOT)), 'Native source drift')
        save(out / 'observations.json', observations)
        save(out / 'result.json', dict(status='passed', versions=['1.99', '1.85'], mocks=False, tc_jar_sha256=sha(jar), java_days=[7, 0, -1], deleted_per_rust=6107, fixture_auth='disabled on owned loopback TC; not production certification'))
    finally:
        for relay in relays: relay.close()
        for _, _, client, _ in clients: support.stop(client)
        support.stop(tc); command('stop-mysql', ['docker', 'stop', '--time=15', container], timeout=40)
    save(out / 'catalog.json', dict(members=[dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    print('UNDO_RETENTION_PASS ' + str(out), flush=True)


if __name__ == '__main__': run()
