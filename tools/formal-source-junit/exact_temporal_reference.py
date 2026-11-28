"""Observe pinned original JDBC/Undo temporal fidelity; no Rust or TC certification."""
import datetime
import itertools
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
import zipfile

from inventory_rebuilt import ROOT, need, save, sha
import exact_temporal_java_oracle

ORACLE = ROOT.parent / 'seata-java-phase09-oracle'
JAVA = Path('D:/Java/jdk-17.0.12/bin/java.exe')
IMAGE = 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
PASSWORD = 'seata-mysql-fixture-only'
SAMPLES = {
    'date': ['1000-01-01', '1500-02-28', '1582-10-04', '1582-10-05',
             '1582-10-10', '1582-10-14', '1582-10-15', '1582-10-29',
             '1969-12-31', '1970-01-01', '2000-02-29', '9999-12-31', None],
    'time': ['-838:59:59.000000', '-25:01:02.000000', '-00:00:00.000001',
             '00:00:00.000000', '00:00:00.000001', '12:34:56.123456',
             '23:59:59.999999', '24:00:00.000000', '838:59:59.000000', None],
    'timestamp': ['1970-01-01 00:00:01.000000', '1969-12-31 23:59:59.999999',
                  '1970-01-01 00:00:01.000001', '2000-02-29 12:34:56.123456',
                  '2038-01-19 03:14:07.000000', '2038-01-19 03:14:07.999999', None],
}


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-exact-temporal-adapter' / ('java_' + stamp)
    work = Path('D:/codex-builds') / ('seata-exact-temporal-' + stamp)
    out.mkdir(parents=True)
    (work / 'data').mkdir(parents=True)
    print('TEMPORAL_REFERENCE ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', DOCKER_CONFIG='D:/Temp/seata-docker-config', PYTHONDONTWRITEBYTECODE='1')
    for key in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS']:
        env.pop(key, None)
    commands = []

    def command(label, argv, expected=0, extra=None, timeout=120):
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env | (extra or {}),
                                capture_output=True, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=result.returncode))
        save(out / 'commands.json', commands)
        if expected is not None:
            need(result.returncode == expected, label + ': ' + result.stderr[-2000:].decode(errors='replace'))
        return result

    need(command('java-pin', ['git', '-C', ORACLE, 'rev-parse', 'HEAD']).stdout.decode().strip() == exact_temporal_java_oracle.PIN, 'Original Java pin changed')
    inputs = [Path(__file__), Path(exact_temporal_java_oracle.__file__), ROOT / 'tools/formal-source-junit/ExactTemporalColumnOracle.java', ROOT/'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc/ExactTemporalValueReader.java', ROOT/'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc/ExactTemporalDataSource.java']
    save(out / 'source-inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in inputs])
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in inputs:
            archive.writestr(path.relative_to(ROOT).as_posix(), path.read_bytes())
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    owner = uuid.uuid4().hex
    container = None
    reports = []
    try:
        container = command('create', ['docker', 'create', '--pull=never', '--name', 'seata-temporal-' + stamp.replace('_', '-'),
            '--label', 'seata.temporal.owner=' + owner, '--mount', 'type=bind,source=' + (work / 'data').as_posix() + ',target=/var/lib/mysql',
            '-p', '127.0.0.1:' + str(port) + ':3306', '-e', 'MYSQL_ROOT_PASSWORD=' + PASSWORD,
            '-e', 'MYSQL_ROOT_HOST=%', '-e', 'MYSQL_DATABASE=temporal_reference', IMAGE]).stdout.decode().strip()
        command('start', ['docker', 'start', container])
        mysql = ['docker', 'exec', '-e', 'MYSQL_PWD=' + PASSWORD, container, 'mysql', '-h127.0.0.1', '-uroot', '-N', '-B', '-e']
        deadline = time.monotonic() + 120
        while subprocess.run(mysql + ['SELECT VERSION()'], env=env, capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW).returncode:
            need(time.monotonic() < deadline, 'Owned MySQL startup timeout')
            time.sleep(.3)
        schema = ["SET time_zone='+00:00'; SET sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION'; USE temporal_reference"]
        for kind, values in SAMPLES.items():
            native = {'date': 'DATE', 'time': 'TIME(6)', 'timestamp': 'TIMESTAMP(6)'}[kind]
            schema.append('CREATE TABLE probe_' + kind + '(id INT PRIMARY KEY,v ' + native + ' NULL) ENGINE=InnoDB')
            schema.append('CREATE TABLE probe_' + kind + '_restored LIKE probe_' + kind)
            for i, value in enumerate(values, 1):
                statement = 'INSERT INTO probe_' + kind + ' VALUES(' + str(i) + ',' + ('NULL' if value is None else "'" + value + "'") + ')'
                if kind == 'timestamp' and i == 2:
                    # 明确记录 MySQL 自身范围错误，不把非法 TIMESTAMP 混入已读样本。
                    rejected_statement = statement
                else:
                    schema.append(statement)
        for fsp in range(6):
            schema.append('CREATE TABLE probe_time_' + str(fsp) + '(id INT PRIMARY KEY,v TIME(' + str(fsp) + ') NULL) ENGINE=InnoDB')
            schema.append('CREATE TABLE probe_time_' + str(fsp) + '_restored LIKE probe_time_' + str(fsp))
            schema.append("INSERT INTO probe_time_" + str(fsp) + " VALUES(1,'12:34:56.123456'),(2,NULL)")
        schema.append('CREATE TABLE probe_time_bounds(id INT PRIMARY KEY,v TIME(6) NULL) ENGINE=InnoDB')
        schema.append('CREATE TABLE probe_adapter_write_probe(id INT PRIMARY KEY,v TIME(6) NULL,d DATE NULL) ENGINE=InnoDB')
        command('schema', mysql + [';'.join(schema)])
        bounds=[]
        for i,value in enumerate(['838:59:59.000000','838:59:59.000001','838:59:59.999999','-838:59:59.000001','-838:59:59.999999','839:00:00.000000','-839:00:00.000000'],1):
            result=command('time-bound-'+str(i),mysql+[schema[0]+";INSERT INTO probe_time_bounds VALUES("+str(i)+",'"+value+"')"],expected=None)
            after=command('time-bound-value-'+str(i),mysql+[schema[0]+';SELECT CAST(v AS CHAR) FROM probe_time_bounds WHERE id='+str(i)])
            bounds.append(dict(input=value,exit_code=result.returncode,physical=after.stdout.decode().strip()))
        save(out/'time-bounds.json',bounds)
        rejected = command('timestamp-below-range', mysql + [schema[0] + ';' + rejected_statement], expected=1)
        need(b'1292' in rejected.stderr, 'TIMESTAMP range must fail for its actual value')
        cp = exact_temporal_java_oracle.build(work, out, env, ORACLE)
        for jvm, session, mode, server_prepare in itertools.product(['UTC', 'Asia/Shanghai'], ['+00:00', '+08:00'], ['read_text', 'read_bytes', 'read_bytes_write_text', 'datasource'], ['false','true']):
            name = jvm.replace('/', '-') + ('-utc' if session == '+00:00' else '-east8') + '-' + mode + '-prepared-' + server_prepare
            target = out / name
            target.mkdir()
            command(name, [JAVA, '-Duser.timezone=' + jvm, '-Djava.io.tmpdir=D:/Temp',
                          '-Xlog:class+load=info:file=' + (target / 'class-load.log').as_posix(),
                          '-cp', cp, 'org.apache.seata.rm.datasource.sql.struct.cache.ExactTemporalColumnOracle', target, session, mode, server_prepare],
                    extra=dict(SEATA_MYSQL_PASSWORD=PASSWORD, SEATA_MYSQL_PORT=str(port), SEATA_MYSQL_DATABASE='temporal_reference'))
            report = json.loads((target / 'observations.json').read_bytes())
            need(len(report['cases']) == 41, 'Temporal case missing')
            reports.append(dict(path=name, jvm_zone=jvm, session_zone=session,
                                adapter_mode=mode, server_prepare=server_prepare, cases=len(report['cases']), original_exceptions=sum(r['status'] == 'ORIGINAL_EXCEPTION' for r in report['cases']),
                                changed_physical=sum(r.get('same_physical') is False for r in report['cases']),
                                changed_value=sum(r.get('same_value') is False for r in report['cases'])))
        exact_temporal_java_oracle.verify(out)
        for row in json.loads((out / 'source-inputs.json').read_bytes()):
            need(sha(ROOT / row['path']) == row['sha256'], 'Executed temporal source drift')
        save(out / 'result.json', dict(status='EXACT_TEMPORAL_ADAPTER_FIDELITY_OBSERVED', profiles=reports,
             actual_cases=1312, rust_implementation_certified=False, full_temporal_support=False, whole_plan_complete=False))
    finally:
        if container:
            state = json.loads(command('inspect', ['docker', 'inspect', container]).stdout)[0]
            need(state['Id'] == container and state['Image'] == IMAGE and state['Config']['Labels'].get('seata.temporal.owner') == owner, 'Owned container identity changed')
            command('mysql-log', ['docker', 'logs', container])
            command('stop', ['docker', 'stop', '--time', '10', container])
            command('remove', ['docker', 'rm', container])
            need(command('confirm-removed', ['docker', 'inspect', container], expected=None).returncode != 0, 'Owned container remains')
        save(out / 'cleanup.json', dict(owned_container_removed=bool(container), work=str(work)))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
