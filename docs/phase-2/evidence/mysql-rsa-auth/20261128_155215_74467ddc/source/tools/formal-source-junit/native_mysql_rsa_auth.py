"""Original MySQL cold-cache RSA auth, TLS, negative peers and original TC AT callbacks."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import time
import uuid
from java_writer_jdbc_profile_tc import ROOT, BASE, MIGRATIONS, IMAGE, PASSWORD, module, need, put, reg, save, sha, support, port, wait
from mysql_auth_wire_proxy import AuthProxy

NATIVE = 'original_mysql_rsa_authentication_and_formal_at_callback'


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-rsa-auth' / stamp
    work = Path('D:/codex-builds') / ('mysql-rsa-auth-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True); print('MYSQL_RSA_AUTH ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', DOCKER_CONFIG='D:/Temp/seata-docker-config',
        CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'RUSTC', 'RUSTDOC', 'SEATA_CONFIG_NAME', 'SEATA_ENV'): env.pop(key, None)
    commands = []

    def command(label, args, data=None, timeout=180, codes=(0,)):
        if (out / (label + '.stdout')).exists(): label += '-' + str(len(commands))
        p = subprocess.run(list(map(str, args)), cwd=work, env=env, input=data, capture_output=True, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / (label + '.stdout'), p.stdout); put(out / (label + '.stderr'), p.stderr)
        commands.append(dict(label=label, argv=list(map(str, args)), exit_code=p.returncode)); save(out / 'commands.json', commands)
        need(p.returncode in codes, label + ': ' + (p.stdout + p.stderr)[-3500:].decode(errors='replace')); return p.stdout

    jar = reg.ORACLE / 'server/target/seata-server.jar'
    need(command('java-pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).decode().strip() == reg.PIN and sha(jar) == reg.JAR_SHA, 'Original Java changed')
    need(sha(BASE / 'catalog.json') == '59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799', 'Dependency baseline differs')
    jars = []
    for row in json.loads((BASE / 'dependencies.json').read_bytes()):
        p = Path(row['path']); need(sha(p) == row['sha256'], 'Original dependency differs')
        if not p.name.startswith(('mockito-', 'byte-buddy-', 'objenesis-', 'lombok-')): jars.append(p)
    template = ROOT / 'docs/phase-2/evidence/exact-temporal-rust-owner-java/20261128_133635_d5470cdb/application.yml'
    inputs = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', *sorted(p for p in (ROOT / 'crates').rglob('*') if p.is_file()),
        *sorted(p for p in (ROOT / 'third_party/sqlx_mysql').rglob('*') if p.is_file()), Path(__file__).resolve(),
        ROOT / 'tools/formal-source-junit/mysql_auth_wire_proxy.py', ROOT / 'tools/formal-source-junit/SqlMatrixUndoOracle.java',
        ROOT / 'tools/formal-source-junit/java_writer_jdbc_profile_tc.py', ROOT / 'tools/formal-first-phase-commit/mysql_tls_material.py',
        ROOT / 'docs/phase-2/mysql-rsa-auth-replacement-contract.md', template, Path(support.__file__), Path(reg.__file__),
        ROOT / 'tools/formal-tc-lock-coverage/live.py', ROOT / 'tools/formal-rm-java/validate.py',
        ROOT / 'tools/java-go-at-shared-lock/proxy.py', ROOT / 'tools/java-go-at-shared-lock/wire.py', ROOT / 'tools/go-java-v1-real-at/verify.py']
    inputs = list(dict.fromkeys(inputs))
    for p in inputs: put(out / 'source' / p.relative_to(ROOT), p.read_bytes())
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    save(out / 'dependencies.json', [dict(path=str(p), sha256=sha(p)) for p in [jar, *jars]])
    audit_db = ROOT.parent / '.seata-advisory-db'
    command('audit-db', ['git', '-C', audit_db, 'rev-parse', 'HEAD'])
    audit = json.loads(command('cargo-audit', ['cargo-audit', 'audit', '--file', ROOT / 'Cargo.lock', '--db', audit_db, '--no-fetch', '--json'], codes=(0, 1)))
    need(not audit['settings']['ignore'] and {v['advisory']['id'] for v in audit['vulnerabilities']['list']} == {'RUSTSEC-2026-0459', 'RUSTSEC-2026-0009'}, 'RSA remediation or other advisory differs')
    need(b'\nname = "rsa"\n' not in (ROOT / 'Cargo.lock').read_bytes().replace(b'\r\n', b'\n'), 'RSA remains locked')
    binaries = {}
    for version in ('1.99', '1.85'):
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        command('rustc-' + version, ['rustc', '+' + version + '.0', '--version'])
        tree = command('tree-' + version, ['cargo', '+' + version + '.0', 'tree', '--manifest-path', ROOT / 'Cargo.toml', '--offline', '--locked', '-p', 'seata-db-mysql', '--edges', 'normal'])
        need(b'rsa v' not in tree and b'aws-lc-rs v1.19.0' in tree, 'Actual crypto dependency differs')
        args = ['cargo', '+' + version + '.0', 'test', '--manifest-path', ROOT / 'Cargo.toml', '--offline', '--locked', '-p', 'seata-db-mysql', '--features', 'experimental_changed_commit', '--test', 'live_rsa_auth_java']
        raw = command('build-' + version, [*args, '--no-run', '--message-format=json'], timeout=600)
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith(b'{')]
        executable, = [Path(r['executable']) for r in artifacts if r.get('executable') and r['target']['name'] == 'live_rsa_auth_java']
        target = work / ('rust-' + version + '.exe'); put(target, executable.read_bytes()); binaries[version] = dict(path=str(target), sha256=sha(target))
    save(out / 'rust-binaries.json', binaries)
    classes = work / 'classes'; classes.mkdir(); cp = os.pathsep.join(map(str, jars))
    args = work / 'compile.args'; put(args, '\n'.join('"' + str(a).replace('\\', '/') + '"' for a in ['-encoding', 'UTF-8', '-cp', cp, '-d', classes, ROOT / 'tools/formal-source-junit/SqlMatrixUndoOracle.java']))
    command('javac', [reg.JAVA.with_name('javac.exe'), '@' + str(args)])
    for p in classes.rglob('*.class'): put(out / 'classes' / p.relative_to(classes), p.read_bytes())
    material = module('rsa_auth_tls_material', ROOT / 'tools/formal-first-phase-commit/mysql_tls_material.py')
    certs = work / 'certs'; material.generate(certs, reg.JAVA.with_name('keytool.exe'))
    for name in ['ca.pem', 'wrong-ca.pem', 'server.pem']: put(out / 'certificates' / name, (certs / name).read_bytes())
    keys = {}
    for bits in (2048, 3072, 4096):
        key = material.rsa.generate_private_key(public_exponent=65537, key_size=bits)
        private = key.private_bytes(material.serialization.Encoding.PEM, material.serialization.PrivateFormat.PKCS8, material.serialization.NoEncryption())
        public = key.public_key().public_bytes(material.serialization.Encoding.PEM, material.serialization.PublicFormat.SubjectPublicKeyInfo)
        keys[bits] = (private, public); put(out / ('public-' + str(bits) + '.pem'), public)
    weak = material.rsa.RSAPublicNumbers(65537, (1 << 1023) + 1).public_key().public_bytes(material.serialization.Encoding.PEM, material.serialization.PublicFormat.SubjectPublicKeyInfo)
    put(out / 'weak-1024.pem', weak)
    db_port, tc_port = port(), port(); need(db_port != tc_port, 'Port overlap'); data = work / 'data'; data.mkdir()
    for name, raw in zip(['auth-private.pem', 'auth-public.pem'], keys[2048]): put(certs / name, raw)
    container = command('create', ['docker', 'create', '--pull=never', '--name', 'mysql-rsa-' + stamp.replace('_', '-'), '--label', 'seata.fixture.owner=' + stamp,
        '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql', '--mount', 'type=bind,source=' + certs.as_posix() + ',target=/certs,readonly',
        '-p', '127.0.0.1:' + str(db_port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD, '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=admission', IMAGE,
        '--ssl-ca=/certs/ca.pem', '--ssl-cert=/certs/server.pem', '--ssl-key=/certs/server-key.pem', '--general-log=ON', '--log-output=TABLE',
        '--caching-sha2-password-private-key-path=/certs/auth-private.pem', '--caching-sha2-password-public-key-path=/certs/auth-public.pem', '--caching-sha2-password-auto-generate-rsa-keys=OFF',
        '--sha256-password-private-key-path=/certs/auth-private.pem', '--sha256-password-public-key-path=/certs/auth-public.pem', '--sha256-password-auto-generate-rsa-keys=OFF']).decode().strip()
    sql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '--default-character-set=utf8mb4', '--ssl-mode=VERIFY_IDENTITY', '--ssl-ca=/certs/ca.pem', '-h127.0.0.1', '-uroot', '-r', '-N', '-B', 'admission', '-e']
    counts = 'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log WHERE log_status=0)'
    activity = "SELECT COUNT(*) FROM mysql.general_log WHERE (user_host LIKE 'auth_client[%]%' OR user_host LIKE 'auth_sha256[%]%') AND command_type IN ('Query','Prepare','Execute')"
    def ready(): command('ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot', '-p' + PASSWORD, '--wait=60', 'ping'])
    tc = client = mysql_proxy = wire_proxy = None; observations = []
    try:
        command('start', ['docker', 'start', container]); ready()
        ddl = b''.join(command(label + '-ddl', ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':' + path]) for label, path in [('server', 'script/server/db/mysql.sql'), ('undo', 'script/client/at/db/mysql.sql')])
        for name in ('001_seata_rs_at_branch_v1.sql', '002_seata_rs_at_registration_origin.sql', '003_seata_rs_at_commit_decision.sql', '004_seata_rs_at_first_phase_witness.sql', '005_seata_rs_at_resource_binding.sql', '006_seata_rs_at_commit_attempt.sql', '007_seata_rs_at_commit_report_started.sql', '011_seata_rs_at_tinyint_profile_binding.sql', '013_seata_rs_at_undo_checksum.sql', '021_seata_rs_at_jdbc_profile_binding.sql'): ddl += (MIGRATIONS / name).read_bytes() + b'\n'
        ddl += ("CREATE TABLE auth_work(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO auth_work VALUES(1,10); CREATE USER 'auth_client'@'%' IDENTIFIED WITH caching_sha2_password BY '" + PASSWORD + "'; CREATE USER 'auth_sha256'@'%' IDENTIFIED WITH sha256_password BY '" + PASSWORD + "'; GRANT ALL PRIVILEGES ON *.* TO 'auth_client'@'%'; GRANT ALL PRIVILEGES ON *.* TO 'auth_sha256'@'%';").encode()
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-uroot', 'admission'], data=ddl)
        import re
        config = re.sub(r'service-port: \d+', 'service-port: ' + str(tc_port), template.read_text())
        config = re.sub(r'path: .*?/logs', 'path: ' + work.as_posix() + '/logs', config)
        config = re.sub(r'jdbc:mysql://127\.0\.0\.1:\d+/admission', 'jdbc:mysql://127.0.0.1:' + str(db_port) + '/admission', config)
        put(work / 'application.yml', config); put(out / 'application.yml', config)
        tc_args = [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Xmx512m', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work), '-Dspring.config.location=file:' + (work / 'application.yml').as_posix(),
            '-Dloader.path=' + next(p for p in jars if p.name == 'mysql-connector-java-8.0.27.jar').as_posix(), '-cp', jar, 'org.springframework.boot.loader.launch.PropertiesLauncher', '-h', '127.0.0.1']
        save(out / 'tc-command.json', dict(argv=list(map(str, tc_args)), sha256=sha(jar)))
        tc = support.spawn(tc_args, work, env, out, 'tc'); support.wait_owned(tc, tc_port, env)
        proxy_module = module('rsa_auth_tc_proxy', ROOT / 'tools/java-go-at-shared-lock/proxy.py')
        for bits in (2048, 3072, 4096):
            if bits != 2048:
                command('restart-stop', ['docker', 'stop', '--time=15', container], timeout=40)
                for name, raw in zip(['auth-private.pem', 'auth-public.pem'], keys[bits]): (certs / name).write_bytes(raw)
                command('restart-start', ['docker', 'start', container]); ready()
            for version in ('1.99', '1.85'):
                modes = ['caching', 'sha256', 'tls'] + (['bad_password', 'bad_pem', 'weak_key', 'wrong_ca'] if bits == 2048 else [])
                for mode in modes:
                    label = str(bits) + '-' + version + '-' + mode; folder = out / label; folder.mkdir()
                    reject = mode in ['bad_password', 'bad_pem', 'weak_key', 'wrong_ca']; listener, db_listener = port(), port()
                    wire_proxy = proxy_module.WireProxy(folder, listener, tc_port); wire_proxy.start()
                    replacement = b'invalid PEM' if mode == 'bad_pem' else weak if mode == 'weak_key' else None
                    mysql_proxy = AuthProxy(folder, db_listener, db_port, replacement); mysql_proxy.start()
                    resource = 'jdbc:mysql://127.0.0.1:' + str(db_listener) + '/admission'
                    command(label + '-seed', sql + ["UPDATE auth_work SET v=10 WHERE id=1; REPLACE INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,X'" + resource.encode().hex() + "',1,1)"])
                    before = command(label + '-activity-before', sql + [activity]).strip()
                    command(label + '-cold-cache', sql + ['FLUSH PRIVILEGES'])
                    extra = dict(SEATA_AUTH_OUT=str(folder), SEATA_AUTH_MODE=mode, SEATA_AUTH_PORT=str(db_listener), SEATA_AUTH_EXPECT_REJECT=str(reject).lower(), SEATA_AUTH_CA=str(certs / ('wrong-ca.pem' if mode == 'wrong_ca' else 'ca.pem')),
                        SEATA_MYSQL_DATABASE='admission', SEATA_MYSQL_PASSWORD=PASSWORD, SEATA_REAL_MYSQL_PORT=str(db_port), SEATA_MYSQL_PORT=str(db_port), SEATA_RM_ENDPOINT='127.0.0.1:' + str(listener), SEATA_TM_ENDPOINT='127.0.0.1:' + str(listener))
                    argv = [binaries[version]['path'], NATIVE, '--ignored', '--exact', '--nocapture']; save(folder / 'command.json', dict(argv=argv, environment=extra, sha256=binaries[version]['sha256']))
                    client = support.spawn(argv, ROOT, env | extra, folder, 'client')
                    if reject:
                        wait((folder / 'rejected.json').exists, client, label + ' refusal', seconds=40)
                        need(client.wait(timeout=15) == 0, 'Rejected auth test failed')
                        need(command(label + '-activity-after', sql + [activity]).strip() == before, 'Rejected credential executed SQL')
                        need(command(label + '-rejected-state', sql + [counts + '; SELECT v FROM auth_work']).strip() == b'0\t0\t0\t0\n10', 'Rejected auth changed business/TC')
                        observation = dict(bits=bits, version=version, mode=mode, rejected=True)
                    else:
                        wait((folder / 'phase.json').exists, client, label + ' phase', seconds=60)
                        phase = json.loads((folder / 'phase.json').read_bytes()); need(phase['resource'] == resource and phase['checksum_persisted'], 'Original phase differs')
                        need(command(label + '-phase-value', sql + ['SELECT v FROM auth_work']).strip() == b'11', 'Original DML missing')
                        command(label + '-sessions', sql + ["SELECT t.PROCESSLIST_USER,s.VARIABLE_VALUE FROM performance_schema.threads t JOIN performance_schema.status_by_thread s USING (THREAD_ID) WHERE t.PROCESSLIST_USER IN ('auth_client','auth_sha256') AND s.VARIABLE_NAME='Ssl_cipher' ORDER BY t.PROCESSLIST_ID"])
                        command(label + '-phase-identity', sql + ['SELECT xid,branch_id,resource_id,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table'])
                        command(label + '-phase-undo', sql + ['SELECT HEX(u.rollback_info),HEX(b.undo_sha256),b.first_phase_committed,b.jdbc_profile FROM undo_log u JOIN seata_rs_at_branch_v1 b ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.log_status=0'])
                        put(folder / 'release', 'Independent original phase observed')
                        wait((folder / 'finished.json').exists, client, label + ' callback')
                        wait(lambda: command(label + '-terminal', sql + [counts]).strip() == b'0\t0\t0\t0', client, label + ' cleanup')
                        command(label + '-final-value', sql + ['SELECT v FROM auth_work'])
                        command(label + '-history', sql + ["SELECT b.phase,b.terminal,b.commit_terminal,b.first_phase_committed,HEX(b.undo_sha256) FROM seata_rs_at_branch_v1 b WHERE xid=X'" + phase['xid'].encode().hex() + "' AND branch_id=" + str(phase['branch_id'])])
                        put(folder / 'stop', 'Independent terminal observed'); need(client.wait(timeout=15) == 0, 'Positive auth test failed')
                        observation = dict(bits=bits, version=version, mode=mode, rejected=False, phase=phase, final=json.loads((folder / 'finished.json').read_bytes()))
                    need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Native auth test missing')
                    client = None; mysql_proxy.close(); mysql_proxy = None; wire_proxy.close(); wire_proxy = None
                    observations.append(observation); save(out / 'observations.json', observations)
                    print('MYSQL_RSA_AUTH_CASE_PASS ' + label, flush=True)
        command('java-oracle', [reg.JAVA, '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Xlog:class+load=info:file=' + (out / 'oracle-class-load.log').as_posix(), '-cp', os.pathsep.join([str(classes), cp]), 'SqlMatrixUndoOracle', out])
        need(len(json.loads((out / 'java-oracle.json').read_bytes())['cases']) == 18, 'Original parser scope differs')
        for p in inputs: need(sha(p) == sha(out / 'source' / p.relative_to(ROOT)), 'Runtime source drift')
        save(out / 'result.json', dict(status='passed', versions=['1.99','1.85'], key_bits=[2048,3072,4096], globals=18, rejected=8, mocks=False, tc_sha256=sha(jar), rsa_advisory_removed=True, remaining_advisories=2))
    finally:
        if mysql_proxy: mysql_proxy.close()
        if wire_proxy: wire_proxy.close()
        support.stop(client); support.stop(tc); command('stop-mysql', ['docker', 'stop', '--time=15', container], timeout=40)
    save(out / 'catalog.json', dict(members=[dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    print('MYSQL_RSA_AUTH_PASS ' + str(out), flush=True)


if __name__ == '__main__': run()
