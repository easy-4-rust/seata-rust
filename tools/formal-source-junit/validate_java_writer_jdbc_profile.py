"""Check native Java admission evidence and the exact compiled/runtime inputs."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(out):
    catalog = json.loads((out / 'catalog.json').read_bytes())
    names = set()
    for member in catalog['members']:
        name = member['path']
        assert name not in names and not Path(name).is_absolute() and '..' not in Path(name).parts
        names.add(name)
        assert sha(out / name) == member['sha256'], 'Catalog member differs: ' + name
    assert names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'}
    result = json.loads((out / 'result.json').read_bytes())
    assert result == dict(status='passed', accepted=6, rejected=69, lock_timeouts=6,
        legacy_null_and_missing=True,
        scope='real MySQL and original ConnectionProxy; mocked DataSourceProxy identity; no TC/RM certification')
    commands = json.loads((out / 'commands.json').read_bytes())
    assert [c['label'] for c in commands] == ['compile', 'create', 'start', 'ready', 'schema', 'probe',
        'legacy-8033', 'compile-legacy-actor', 'stop']
    assert all(c['exit_code'] == 0 for c in commands)
    assert b'JAVA_WRITER_JDBC_PROFILE_OK accepted=6 rejected=69 locks=6 legacy_null_and_missing=true' in (out / 'probe.stdout').read_bytes()
    assert b'JAVA_WRITER_LEGACY_8033_OK' in (out / 'legacy-8033.stdout').read_bytes()
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        assert sha(out / 'source' / row['path']) == row['sha256'] == sha(ROOT / row['path']), 'Executed source drift'
    for row in [*json.loads((out / 'dependencies.json').read_bytes()), json.loads((out / 'legacy-driver.json').read_bytes())]:
        assert sha(Path(row['path'])) == row['sha256'], 'Runtime dependency drift'
    probe = next(c for c in commands if c['label'] == 'probe')['argv']
    classes = Path(probe[probe.index('-cp') + 1].split(';')[0])
    for compiled in (out / 'classes').rglob('*.class'):
        assert sha(compiled) == sha(classes / compiled.relative_to(out / 'classes')), 'Compiled class drift'
    for name, driver in [('probe-class-load.log', 'mysql-connector-java-8.0.27.jar'),
                         ('legacy-class-load.log', 'mysql-connector-j-8.0.33.jar')]:
        lines = (out / name).read_text(encoding='utf-8').splitlines()
        expected = [
            ('io.seatarust.compat.mysql.JavaAtWriterAdmission', classes.as_posix()),
            ('org.apache.seata.rm.datasource.ConnectionProxy', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('com.mysql.cj.jdbc.ConnectionImpl', driver),
            ('org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar')]
        if name == 'probe-class-load.log':
            expected.append(('com.fasterxml.jackson.databind.ObjectMapper', 'jackson-databind-2.18.3.jar'))
        for symbol, origin in expected:
            line = next(line for line in lines if ' ' + symbol + ' source: ' in line)
            assert origin in line.replace('file:/D:', 'D:'), 'Unexpected class origin: ' + symbol
    print('JAVA_WRITER_JDBC_PROFILE_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path)
    verify(parser.parse_args().packet)
