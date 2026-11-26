"""Run the unchanged original core JUnit engine and a separate value observation peer."""
import datetime
import hashlib
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path
from freeze import ROOT, SOURCE, MODULE, DOC, JAVA_PIN

HERE = Path(__file__).resolve().parent
JDK = Path('D:/Java/jdk-17.0.12/bin')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-2/evidence/sqlparser-core-values'/('run_'+stamp)
    work = Path('D:/codex-builds')/('seata_core_values_'+stamp)
    out.mkdir(parents=True)
    (work/'classes').mkdir(parents=True)
    (work/'home').mkdir()
    inventory = json.loads((DOC/'inventory.json').read_bytes())
    assert inventory['java_pin'] == JAVA_PIN
    for row in inventory['objects']:
        assert sha(SOURCE/row['java']) == row['sha256']
    for row in inventory['cases']:
        assert sha(SOURCE/row['source']) == sha(ROOT/row['immutable_copy']) == row['source_sha256']
    prior = ROOT/'docs/phase-2/evidence/source-junit-literal/runtime_20261118_013107_9651fdfd'
    result = json.loads((prior/'result.json').read_bytes())
    runtime = Path(result['work'])
    helpers = json.loads((prior/'helper-artifacts.json').read_bytes())
    for row in helpers:
        assert sha(Path(row['path'])) == row['sha256']
    old_command = json.loads((prior/'seata-sqlparser-core-command.json').read_bytes())
    cp = [str(work/'classes'), str(runtime/'classes'), str(MODULE/'target/test-classes'), str(MODULE/'target/classes')]
    cp += (MODULE/'target/declared-test-classpath.txt').read_text('utf-8').strip().split(os.pathsep)
    inputs = {}
    for entry in cp[1:]:
        path = Path(entry)
        assert path.resolve().is_relative_to(Path('D:/codex-builds').resolve())
        for file in sorted(path.rglob('*')) if path.is_dir() else [path]:
            if file.is_file():
                inputs[str(file)] = sha(file)
    env = os.environ | dict(JAVA_HOME=str(JDK.parent), TEMP='D:/Temp', TMP='D:/Temp')
    for key in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS']:
        env.pop(key, None)
    options = ['-Duser.home='+str(work/'home'), '-Djava.io.tmpdir=D:/Temp']
    commands = []

    def invoke(args, name):
        with (out/name).open('wb') as log:
            code = subprocess.run(list(map(str, args)), cwd=work, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, timeout=120,
                                  creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str, args)), cwd=str(work), exit_code=code, log=name))
        (out/'commands.json').write_text(json.dumps(commands, indent=2)+'\n', 'utf-8')
        if code:
            raise RuntimeError('Original Java command failed: '+name)

    invoke([JDK/'javac.exe', '-encoding', 'UTF-8', '-cp', os.pathsep.join(cp), '-d', work/'classes',
            HERE/'CoreValueOracle.java'], 'oracle-compile.log')
    (out/'CoreValueOracle.java').write_bytes((HERE/'CoreValueOracle.java').read_bytes())
    (out/'oracle-class.class').write_bytes((work/'classes/CoreValueOracle.class').read_bytes())
    args = [JDK/'java.exe']+options+old_command['original_POM_JDK17_arguments']
    invoke(args+['-cp', os.pathsep.join(cp), 'SourceJUnitRunner', MODULE/'target/test-classes',
                 out/'junit-events.jsonl', 'execute'], 'original-junit.log')
    sys.path.insert(0, str(ROOT/'tools/formal-source-junit'))
    from validate import load, validate_events, controls
    rows = load(out/'junit-events.jsonl')
    validation = validate_events(rows, 'seata-sqlparser-core')
    counterexamples = controls(rows, 'seata-sqlparser-core')
    invoke(args+['-cp', os.pathsep.join(cp), 'CoreValueOracle', out/'java-values.tsv',
                 out/'java-constants.tsv', out/'original-class-origins.tsv'], 'original-oracle.log')
    for path, digest in inputs.items():
        assert sha(Path(path)) == digest
    for row in inventory['objects']:
        assert sha(SOURCE/row['java']) == row['sha256']
    (out/'runtime-inputs.json').write_text(json.dumps(inputs, indent=2)+'\n', 'utf-8')
    (out/'junit-validation.json').write_text(json.dumps(validation, indent=2)+'\n', 'utf-8')
    (out/'junit-counterexamples.json').write_text(json.dumps(counterexamples, indent=2)+'\n', 'utf-8')
    constants = ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/java-constants.tsv'
    assert not constants.exists()
    constants.write_bytes((out/'java-constants.tsv').read_bytes())
    receipt = dict(status='ORIGINAL_CORE_JUNIT_PASSED_AND_VALUE_ORACLE_RECORDED', java_pin=JAVA_PIN,
                   work=str(work), out=str(out), original_tests_passed=validation['passed'],
                   original_test_source_modified=False, original_junit=rows[0],
                   Java_cases_observed=23, Rust_differential='NOT_RUN', core_migration_complete=False,
                   prior_helper_root=str(prior), helper_artifacts=helpers,
                   raw_fixture=str(constants.relative_to(ROOT)), raw_fixture_sha256=sha(constants))
    (out/'oracle-result.json').write_text(json.dumps(receipt, indent=2)+'\n', 'utf-8')
    print(json.dumps(receipt), flush=True)


if __name__ == '__main__':
    run()
