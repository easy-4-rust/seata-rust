"""Run original core cases and the next metadata observation peer against bound original classes."""
import datetime
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path
from freeze import ROOT, DOC, SOURCE, MODULE
from qualify import read, sha
from verify import need

HERE = Path(__file__).resolve().parent
PARENT = ROOT/'docs/phase-2/evidence/sqlparser-core-values-gate/run_20261118_022222_bb4f2677'


def run():
    need(sha(PARENT/'catalog.json') == 'b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd', 'Wrong parent catalog')
    for member in read(PARENT/'catalog.json')['members']:
        need(sha(PARENT/member['path']) == member['sha256'], 'Published parent drift')
    bindings = read(PARENT/'oracle/runtime-inputs.json')
    for path, digest in bindings.items():
        need(sha(Path(path)) == digest, 'Original Java classpath drift')
    for row in read(DOC/'inventory.json')['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-2/evidence/sqlparser-column'/('run_'+stamp)
    work = Path('D:/codex-builds')/('seata_column_metadata_'+stamp)
    (work/'classes').mkdir(parents=True)
    (work/'home').mkdir()
    out.mkdir(parents=True)
    original = read(PARENT/'oracle/commands.json')[-1]['argv']
    cp_index = original.index('-cp')
    classpath = os.pathsep.join([str(work/'classes')]+original[cp_index+1].split(os.pathsep)[1:])
    java = original[:cp_index]
    java = ['-Duser.home='+str(work/'home') if arg.startswith('-Duser.home=') else arg for arg in java]
    jdk = Path(java[0]).parent
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', JAVA_HOME=str(jdk.parent))
    for key in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS']:
        env.pop(key, None)
    commands = []
    def invoke(args, name):
        with (out/name).open('wb') as log:
            code = subprocess.run(list(map(str, args)), cwd=work, env=env, stdout=log,
                stderr=subprocess.STDOUT, timeout=120, creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str, args)), cwd=str(work), exit_code=code, log=name))
        (out/'commands.json').write_text(json.dumps(commands, indent=2)+'\n', 'utf-8')
        need(code == 0, 'Original oracle failed: '+name)
    producers = [HERE/'ColumnMetaOracle.java', HERE/'CoreValueOracle.java']
    for path in producers:
        (out/path.name).write_bytes(path.read_bytes())
    invoke([jdk/'javac.exe', '-encoding', 'UTF-8', '-cp', classpath, '-d', work/'classes']+producers, 'compile.log')
    invoke(java+['-cp', classpath, 'SourceJUnitRunner', MODULE/'target/test-classes', out/'junit-events.jsonl', 'execute'], 'junit.log')
    invoke(java+['-cp', classpath, 'CoreValueOracle', out/'foundation-java.tsv', out/'constants-java.tsv', out/'foundation-origins.tsv'], 'foundation.log')
    invoke(java+['-cp', classpath, 'ColumnMetaOracle', out/'column-java.tsv', out/'edges-java.tsv', out/'column-origin.tsv'], 'column.log')
    sys.path.insert(0, str(ROOT/'tools/formal-source-junit'))
    from validate import load, validate_events, controls
    rows = load(out/'junit-events.jsonl')
    validation = validate_events(rows, 'seata-sqlparser-core')
    control = controls(rows, 'seata-sqlparser-core')
    need(len((out/'column-java.tsv').read_text('utf-8').splitlines()) == 18, 'Original column case floor')
    for path, digest in bindings.items():
        need(sha(Path(path)) == digest, 'Original runtime input drift during execution')
    result = dict(status='ORIGINAL_CORE_76_AND_COLUMN_18_OBSERVED', work=str(work), out=str(out),
        parent=str(PARENT), parent_catalog_sha256=sha(PARENT/'catalog.json'),
        original_runtime_inputs=bindings, original_JUnit_validation=validation,
        original_JUnit_controls=control,
        producer_classes=[dict(path=str(path), sha256=sha(path)) for path in sorted((work/'classes').glob('*.class'))],
        source_cases_observed=41, module_complete=False)
    (out/'oracle-result.json').write_text(json.dumps(result, indent=2)+'\n', 'utf-8')
    fixture = ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/column-edges-java-v2.tsv'
    if fixture.exists():
        need(fixture.read_bytes() == (out/'edges-java.tsv').read_bytes(), 'Recorded derived Java fixture differs; preserve it and stop')
    else:
        fixture.write_bytes((out/'edges-java.tsv').read_bytes())
    print(json.dumps(dict(out=str(out), work=str(work), original_JUnit=76, source_cases_observed=41,
                         unicode_case_sets=(out/'edges-java.tsv').read_text('utf-8').splitlines()[:3])), flush=True)


if __name__ == '__main__':
    run()
