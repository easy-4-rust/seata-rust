"""Execute the three complete original MySQL Undo executor test classes."""
import base64
import collections
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile

from inventory_rebuilt import BASELINE, JDK, ROOT, need, safe_path, save, sha
from portable_rebuilt import verify

PREFIX = 'org.apache.seata.rm.datasource.undo.mysql.'
CLASSES = ['MySQLUndoInsertExecutorTest', 'MySQLUndoUpdateExecutorTest', 'MySQLUndoDeleteExecutorTest']


def run():
    package = BASELINE / 'portable_inputs_20261127'
    manifest = verify(package)
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_mysql_undo_source_' + stamp)
    work.mkdir()
    out = ROOT / 'docs/phase-2/evidence/mysql-undo-executor-source' / ('oracle_' + stamp)
    out.mkdir(parents=True)
    print('MYSQL_UNDO_EXECUTOR_SOURCE ' + str(out), flush=True)
    with zipfile.ZipFile(package / 'runtime-inputs.zip') as archive:
        for info in archive.infolist():
            target = safe_path(work, info.filename)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(info))
    for role in ['helper', 'original', 'observed', 'observed-source', 'home']:
        (work / role).mkdir()
    cp = [safe_path(work, member) for member in manifest['modules']['rm-datasource']['classpath']]
    for path in cp:
        if path.suffix != '.jar':
            path.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join(map(str, cp))
    here = Path(__file__).resolve().parent
    commands = []
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', JAVA_TOOL_OPTIONS=
        '-Duser.home=' + str(work / 'home') + ' -Djava.io.tmpdir=D:/Temp')
    for key in ['JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'CLASSPATH']:
        env.pop(key, None)

    def command(label, argv):
        argfile = work / (label + '.args')
        argfile.write_text('\n'.join('"' + str(a).replace('\\', '/').replace('"', '\\"') + '"'
                                    for a in argv[1:]), encoding='utf-8')
        result = subprocess.run([str(argv[0]), '@' + str(argfile)], cwd=work, env=env,
                                capture_output=True, timeout=180, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        shutil.copyfile(argfile, out / argfile.name)
        commands.append(dict(label=label, argv=list(map(str, argv)), exit_code=result.returncode))
        save(out / 'commands.json', commands)
        need(result.returncode == 0, label + ' failed: ' + result.stderr[-1600:].decode(errors='replace'))
        return result.stdout

    helper_sources = [here / 'MysqlUndoExecutorAssertions.java', here / 'MysqlUndoExecutorJUnit.java']
    command('compile-helpers', [JDK / 'bin/javac.exe', '-encoding', 'UTF-8', '-cp', classpath,
                               '-d', work / 'helper', *helper_sources])
    sources = []
    original_bytes = {}
    for name in CLASSES:
        source = work / 'stage/source/rm-datasource/src/test/java' / (PREFIX + name).replace('.', '/')
        source = source.with_suffix('.java')
        sources.append(source)
        data = source.read_bytes()
        original_bytes[str(source)] = data
        observed = data.decode('utf-8').replace('Assertions.', 'seata.verification.MysqlUndoExecutorAssertions.')
        (work / 'observed-source' / source.name).write_text(observed, encoding='utf-8')
        (out / source.name).write_bytes(data)
    base = work / 'stage/source/rm-datasource/src/test/java/org/apache/seata/rm/datasource/undo/BaseExecutorTest.java'
    original_bytes[str(base)] = base.read_bytes()
    (out / base.name).write_bytes(base.read_bytes())
    for role in ['original', 'observed']:
        selected = [*sources, base] if role == 'original' else [*sorted((work / 'observed-source').glob('*.java')), base]
        command('compile-' + role, [JDK / 'bin/javac.exe', '-encoding', 'UTF-8', '-source', '8', '-target', '8',
            '-Xlint:-options', '-cp', str(work / 'helper') + os.pathsep + classpath, '-d', work / role, *selected])
    assertions = collections.defaultdict(list)
    inputs = []
    for role in ['original', 'observed']:
        events = work / (role + '-junit-events.jsonl')
        loaded = work / (role + '-class-load.log')
        argv = [JDK / 'bin/java.exe', '-Dseata.junit.scope=' + str(work), '-Dseata.junit.work=' + str(work),
                '-Djava.security.manager=HarnessGuard', '-Dfile.encoding=UTF-8',
                '-Xlog:class+load=info:file=' + loaded.as_posix(),
                *manifest['modules']['rm-datasource']['original_POM_JDK17_arguments'],
                '-cp', os.pathsep.join([str(work / role), str(work / 'helper'), classpath]),
                'seata.verification.MysqlUndoExecutorJUnit', str(events), *[PREFIX + c for c in CLASSES]]
        stdout = command(role + '-junit', argv)
        rows = [json.loads(line) for line in events.read_bytes().splitlines()]
        need(rows[-1] == dict(kind='summary', passed=6, failed=0, skipped=0), 'Original JUnit cases incomplete')
        shutil.copyfile(events, out / events.name)
        shutil.copyfile(loaded, out / loaded.name)
        for line in stdout.decode('utf-8').splitlines():
            if line.startswith('MYSQL_UNDO_EXECUTOR_ASSERT\t'):
                _, case, data = line.split('\t')
                need(role == 'observed', 'Literal original tests unexpectedly instrumented')
                assertions[case].append(base64.b64decode(data).decode('utf-8'))
            if line.startswith('MYSQL_UNDO_EXECUTOR_INPUT\t'):
                _, classname, data = line.split('\t')
                inputs.append(dict(role=role, classname=classname, input=json.loads(data)))
    expected = {PREFIX + c + '#' + method for c in CLASSES for method in ['buildUndoSQL', 'getUndoRows']}
    need(set(assertions) == expected and sum(map(len, assertions.values())) == 13, 'Original assertion inventory differs')
    fixture = json.loads((ROOT / 'crates/seata_db_mysql/tests/fixtures/mysql_undo_executor_source.json').read_bytes())
    need(len(inputs) == 12 and all(row['input'] == fixture for row in inputs), 'Original source input differs from Rust fixture')
    for path, data in original_bytes.items():
        need(Path(path).read_bytes() == data, 'Original source changed during execution')
    for row in manifest['files']:
        need(sha(safe_path(work, row['member'])) == row['sha256'], 'Sealed Java input drift')
    save(out / 'java-assertions.json', assertions)
    save(out / 'original-inputs.json', inputs)
    save(out / 'source-bindings.json', [dict(path=p, sha256=sha(Path(p))) for p in original_bytes])
    for path in [Path(__file__), *helper_sources]:
        shutil.copyfile(path, out / path.name)
    save(out / 'result.json', dict(status='ORIGINAL_MYSQL_UNDO_EXECUTOR_CASES_EXECUTED',
         original_cases=6, original_assertions=13, original_input_snapshots=len(inputs),
         package_manifest_sha256=sha(package / 'manifest.json'), original_sources_changed=False,
         observations_separate_from_literal_original=True, Rust_parity_assessed=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(oracle=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)
    return out


if __name__ == '__main__':
    run()
