"""Supplement a stopped native matrix with the actual original Java SQL recognizer."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from java_writer_jdbc_profile_tc import ROOT, need, put, reg, save, sha


def run(packet):
    out = packet / 'alias-reference'; out.mkdir()
    work = Path('D:/codex-builds') / ('sql-alias-reference-' + packet.name); work.mkdir()
    source = ROOT / 'tools/formal-source-junit/SqlAliasRecognizerOracle.java'
    matrix = ROOT / 'crates/seata_db_mysql/tests/data/mysql_sql_matrix.json'
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS'): env.pop(key, None)
    commands = []

    def command(label, argv):
        value = subprocess.run(list(map(str, argv)), cwd=work, env=env, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW, timeout=90)
        put(out / (label + '.stdout'), value.stdout); put(out / (label + '.stderr'), value.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=value.returncode)); save(out / 'commands.json', commands)
        need(value.returncode == 0, label + ': ' + value.stderr[-2500:].decode(errors='replace')); return value.stdout

    need(command('java-pin', ['git', '-C', reg.ORACLE, 'rev-parse', 'HEAD']).decode().strip() == reg.PIN, 'Java pin differs')
    jars = []
    for row in json.loads((packet / 'dependencies.json').read_bytes()):
        p = Path(row['path']); need(sha(p) == row['sha256'], 'Original dependency changed')
        if p.name != 'seata-server.jar': jars.append(p)
    for p in [source, Path(__file__).resolve(), matrix]: put(out / 'source' / p.relative_to(ROOT), p.read_bytes())
    save(out / 'inputs.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in [source, Path(__file__).resolve(), matrix]])
    for name in ['BaseMySQLRecognizer', 'MySQLUpdateRecognizer', 'MySQLDeleteRecognizer']:
        command('original-' + name, ['git', '-C', reg.ORACLE, 'show', reg.PIN + ':sqlparser/seata-sqlparser-druid/src/main/java/org/apache/seata/sqlparser/druid/mysql/' + name + '.java'])
    classes = work / 'classes'; classes.mkdir(); cp = os.pathsep.join(map(str, jars))
    args = work / 'javac.args'; put(args, '\n'.join('"' + str(a).replace('\\', '/') + '"' for a in ['-encoding', 'UTF-8', '-cp', cp, '-d', classes, source]))
    command('javac', [reg.JAVA.with_name('javac.exe'), '@' + str(args)])
    for p in classes.rglob('*.class'): put(out / 'classes' / p.relative_to(classes), p.read_bytes())
    command('oracle', [reg.JAVA, '-XX:ActiveProcessorCount=2', '-Duser.timezone=UTC', '-Djava.io.tmpdir=D:/Temp', '-Duser.home=' + str(work),
        '-Xlog:class+load=info:file=' + (out / 'class-load.log').as_posix(), '-cp', os.pathsep.join([str(classes), cp]), 'SqlAliasRecognizerOracle', matrix, out / 'result.json'])
    results = json.loads((out / 'result.json').read_bytes())
    need(len(results) == 6 and all(r['table'].replace('`', '') == 'sql_matrix_records' and r['type'] in ['UPDATE', 'DELETE'] for r in results), 'Original table/type differs')
    need(all((r['alias'] or '').replace('`', '') == ('' if r['id'] == 'qualified_without_alias' else 'r') for r in results), 'Original alias differs')
    print('ORIGINAL_SQL_ALIAS_REFERENCE_PASS ' + str(out))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); run(parser.parse_args().packet.resolve())
