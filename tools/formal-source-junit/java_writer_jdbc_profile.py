"""Native JDBC transaction admission; Seata DataSource identity is mocked, TC is outside scope."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT.parent / 'seata-java-phase09-oracle'
JAVA = Path('D:/Java/jdk-17.0.12/bin/java.exe')
ADMISSION = ROOT / 'compatibility/seata-java/writer-admission/src/main/java/io/seatarust/compat/mysql/JavaAtWriterAdmission.java'
PACKET = ROOT / 'docs/phase-2/evidence/mysql-jdbc-profile-admission/java_20261128_120508_5f38fa21/java-build'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(red):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/java-writer-jdbc-profile' / stamp
    work = Path('D:/codex-builds') / ('java-writer-jdbc-profile-' + stamp)
    out.mkdir(parents=True); work.mkdir(parents=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', DOCKER_CONFIG='D:/Temp/seata-docker-config',
                            SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS'):
        env.pop(key, None)
    commands = []

    def command(label, argv, timeout=120, expected=0, data=None):
        value = subprocess.run(list(map(str, argv)), cwd=work, env=env, input=data, capture_output=True,
                               timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(value.stdout)
        (out / (label + '.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode))
        (out / 'commands.json').write_text(json.dumps(commands, indent=2), encoding='utf-8')
        if expected is not None and value.returncode != expected:
            raise AssertionError(label + ': ' + (value.stdout + value.stderr)[-3000:].decode(errors='replace'))
        return value

    binding = json.loads((PACKET / 'binding.json').read_bytes())
    jars = []
    for row in binding['files']:
        if row['role'] in ('library', 'driver'):
            source = PACKET / 'runtime' / row['relative']
            assert sha(source) == row['sha256']
            target = work / source.name
            target.write_bytes(source.read_bytes()); jars.append(target)
    jars.extend(sorted((ORACLE / 'server/target/lib').glob('*.jar')))
    for module in ('rm', 'rm-datasource', 'tm', 'sqlparser/seata-sqlparser-core', 'sqlparser/seata-sqlparser-druid'):
        artifact = module.rsplit('/', 1)[-1].removeprefix('seata-')
        jars.append(ORACLE / module / 'target' / ('seata-' + artifact + '-2.8.0-SNAPSHOT.jar'))
    m2 = Path.home() / '.m2/repository'
    jars.extend(m2 / name for name in (
        'org/projectlombok/lombok/1.18.36/lombok-1.18.36.jar',
        'org/mockito/mockito-core/4.11.0/mockito-core-4.11.0.jar',
        'net/bytebuddy/byte-buddy/1.12.23/byte-buddy-1.12.23.jar',
        'org/objenesis/objenesis/3.3/objenesis-3.3.jar'))
    sources = [ADMISSION, ROOT / 'tools/formal-source-junit/JavaWriterJdbcProfileProbe.java',
               *sorted((ROOT / 'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc').glob('*.java'))]
    inputs = [*sources, Path(__file__).resolve(), ROOT / 'tools/java-go-at-shared-lock/SharedLockJavaActor.java',
        *[ROOT / 'crates/seata_db_mysql/migrations' / name for name in (
        '005_seata_rs_at_resource_binding.sql', '011_seata_rs_at_tinyint_profile_binding.sql',
        '021_seata_rs_at_jdbc_profile_binding.sql')]]
    for source in inputs:
        dest = out / 'source' / source.relative_to(ROOT)
        dest.parent.mkdir(parents=True, exist_ok=True); dest.write_bytes(source.read_bytes())
    classes = work / 'classes'; classes.mkdir()
    cp = os.pathsep.join(map(str, jars))
    args = ['-XDrawDiagnostics', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *sources]
    argfile = work / 'compile.args'
    argfile.write_text('\n'.join('"' + str(arg).replace('\\', '/') + '"' for arg in args), encoding='utf-8')
    result = command('compile', [JAVA.with_name('javac.exe'), '@' + str(argfile)], expected=None)
    if red:
        assert result.returncode != 0 and b'compiler.err.cant.apply.symbol' in result.stderr
        print('EXPECTED_RED ' + str(out), flush=True)
        return
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    (out / 'source-inputs.json').write_text(json.dumps([dict(path=source.relative_to(ROOT).as_posix(),
        sha256=sha(source)) for source in inputs], indent=2), encoding='utf-8')
    for compiled in classes.rglob('*.class'):
        target = out / 'classes' / compiled.relative_to(classes)
        target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(compiled.read_bytes())
    (out / 'dependencies.json').write_text(json.dumps([dict(path=str(jar), sha256=sha(jar)) for jar in jars], indent=2), encoding='utf-8')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    name = 'java-profile-' + stamp.replace('_', '-')
    data = work / 'data'; data.mkdir()
    container = command('create', ['docker', 'create', '--pull=never', '--name', name,
        '--label', 'seata.fixture.owner=' + stamp, '--mount', 'type=bind,source=' + data.as_posix() + ',target=/var/lib/mysql',
        '-p', '127.0.0.1:' + str(port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + env['SEATA_MYSQL_PASSWORD'],
        '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=admission',
        'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902']).stdout.decode().strip()
    try:
        command('start', ['docker', 'start', container])
        # mysqladmin itself has a bounded retry loop; no new fixture is started after an observation timeout.
        command('ready', ['docker', 'exec', container, 'mysqladmin', '-h127.0.0.1', '-uroot',
                          '-p' + env['SEATA_MYSQL_PASSWORD'], '--connect-timeout=2', '--wait=60', 'ping'], timeout=150)
        ddl = '\n'.join((ROOT / 'crates/seata_db_mysql/migrations' / name).read_text(encoding='utf-8') for name in (
            '005_seata_rs_at_resource_binding.sql', '011_seata_rs_at_tinyint_profile_binding.sql',
            '021_seata_rs_at_jdbc_profile_binding.sql'))
        command('schema', ['docker', 'exec', '-i', '-e', 'MYSQL_PWD=' + env['SEATA_MYSQL_PASSWORD'],
                          container, 'mysql', '-uroot', 'admission'], data=ddl.encode())
        result = command('probe', [JAVA, '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp',
            '-Xlog:class+load=info:file=' + (out / 'probe-class-load.log').as_posix(), '-cp',
            os.pathsep.join([str(classes), cp]), 'JavaWriterJdbcProfileProbe',
            'jdbc:mysql://127.0.0.1:' + str(port) + '/admission'], timeout=90)
        assert b'JAVA_WRITER_JDBC_PROFILE_OK accepted=6 rejected=69 locks=6 legacy_null_and_missing=true' in result.stdout
        legacy_driver = Path('E:/Java/.m2/repository/com/mysql/mysql-connector-j/8.0.33/mysql-connector-j-8.0.33.jar')
        legacy_deps = os.pathsep.join(map(str, [legacy_driver, *[jar for jar in jars if 'mysql-connector' not in jar.name]]))
        legacy_cp = os.pathsep.join([str(classes), legacy_deps])
        legacy = command('legacy-8033', [JAVA, '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp',
            '-Xlog:class+load=info:file=' + (out / 'legacy-class-load.log').as_posix(), '-cp',
            legacy_cp, 'JavaWriterJdbcProfileProbe', 'jdbc:mysql://127.0.0.1:' + str(port) + '/admission', 'legacy'])
        assert b'JAVA_WRITER_LEGACY_8033_OK' in legacy.stdout
        (out / 'legacy-driver.json').write_text(json.dumps(dict(path=str(legacy_driver), sha256=sha(legacy_driver))), encoding='utf-8')
        legacy_classes = work / 'legacy-no-adapter-classes'; legacy_classes.mkdir()
        legacy_args = work / 'legacy.args'
        legacy_args.write_text('\n'.join('"' + str(arg).replace('\\', '/') + '"' for arg in [
            '-encoding', 'UTF-8', '-cp', legacy_deps, '-d', legacy_classes,
            ROOT / 'tools/java-go-at-shared-lock/SharedLockJavaActor.java', ADMISSION]), encoding='utf-8')
        command('compile-legacy-actor', [JAVA.with_name('javac.exe'), '@' + str(legacy_args)])
        for source in inputs:
            assert sha(source) == sha(out / 'source' / source.relative_to(ROOT)), 'Source changed during native probe'
        (out / 'result.json').write_text(json.dumps(dict(status='passed', accepted=6, rejected=69,
            lock_timeouts=6, legacy_null_and_missing=True, scope='real MySQL and original ConnectionProxy; mocked DataSourceProxy identity; no TC/RM certification'), indent=2), encoding='utf-8')
        print('JAVA_WRITER_JDBC_PROFILE_PASS ' + str(out), flush=True)
    finally:
        command('stop', ['docker', 'stop', '--time=15', container], timeout=40)
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*'))
               if p.is_file() and p.name != 'catalog.json']
    (out / 'catalog.json').write_text(json.dumps(dict(members=members), indent=2), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--red', action='store_true')
    run(parser.parse_args().red)
