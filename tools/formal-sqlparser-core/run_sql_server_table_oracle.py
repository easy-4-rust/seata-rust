"""Run the SQL Server metadata peer against the original, pinned Seata classes."""
import datetime
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from freeze import DOC, ROOT, SOURCE
from qualify import read, sha
from verify import need


def run():
    parent = ROOT / 'docs/phase-2/evidence/sqlparser-table-gate/run_20261118_105502_3c628c06'
    digest = '84080e6737b775669c064aed2d792a191ba16da54731125976114792e779d802'
    need(sha(parent / 'catalog.json') == digest, 'Parent catalog drift')
    for row in read(parent / 'catalog.json')['members']:
        need(sha(parent / row['path']) == row['sha256'], 'Parent member drift')
    receipt = read(parent / 'oracle/oracle-result.json')
    runtime = receipt['original_runtime_inputs'] | receipt['jdk_runtime_inputs']
    sources = {
        str(SOURCE / row['java']): row['sha256']
        for row in read(DOC / 'inventory.json')['objects']
        if row['java_type'].split('.')[-1] in {'TableMeta', 'SqlServerTableMeta'}
    }
    need(len(sources) == 2 and len(runtime) == 224, 'Original input scope differs')
    for path, expected in (runtime | sources).items():
        need(sha(Path(path)) == expected, 'Original input drift: ' + path)
    template = next(row['argv'] for row in read(parent / 'oracle/commands.json') if row['log'] == 'TableMetaOracle.log')
    cp_index = template.index('-cp')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_sql_server_table_' + stamp)
    out = ROOT / 'docs/phase-2/evidence/sqlparser-sql-server-table' / ('run_' + stamp)
    classes = work / 'classes'
    classes.mkdir(parents=True)
    out.mkdir(parents=True)
    cp = os.pathsep.join([str(classes)] + template[cp_index + 1].split(os.pathsep)[1:])
    java = ['-Duser.home=' + str(work / 'home') if arg.startswith('-Duser.home=') else arg for arg in template[:cp_index]]
    source = ROOT / 'tools/formal-sqlparser-core/SqlServerTableMetaOracle.java'
    shutil.copy2(source, out / source.name)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS']:
        env.pop(name, None)
    commands = []
    target = out / 'java.tsv'
    for name, argv in [
        ('compile', [Path(java[0]).with_name('javac.exe'), '-encoding', 'UTF-8', '-cp', cp, '-d', classes, out / source.name]),
        ('execute', java + ['-cp', cp, 'SqlServerTableMetaOracle', target]),
    ]:
        argv = list(map(str, argv))
        with (out / (name + '.log')).open('wb') as log:
            code = subprocess.run(argv, cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120).returncode
        commands.append(dict(argv=argv, cwd=str(work), exit_code=code, log=name + '.log'))
        (out / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n', 'utf-8')
        need(code == 0, 'Oracle command failed: ' + name)
    for path, expected in (runtime | sources).items():
        need(sha(Path(path)) == expected, 'Original input changed during run: ' + path)
    producer = out / 'SqlServerTableMetaOracle.class'
    shutil.copy2(classes / producer.name, producer)
    observations = target.read_text('utf-8')
    need([line.split('\t')[0] for line in observations.splitlines()] == ['identity', 'equals', 'hash', 'inherited'], 'Observation scope differs')
    fixture = ROOT / 'crates/seata_test/tests/fixtures/sqlparser_core/sql-server-table-java.tsv'
    if fixture.exists():
        need(fixture.read_text('utf-8') == observations, 'Existing fixture differs')
    else:
        fixture.write_bytes(observations.encode('utf-8'))
    result = dict(parent=parent.relative_to(ROOT).as_posix(), parent_catalog_sha256=digest,
                  original_source_inputs=sources, original_runtime_input_count=len(runtime),
                  producer_source_sha256=sha(out / source.name), producer_class_sha256=sha(producer),
                  observation_sha256=sha(target), records=4, source_JUnit_cases_added=0)
    (out / 'oracle-result.json').write_text(json.dumps(result, indent=2) + '\n', 'utf-8')
    print(out)


if __name__ == '__main__':
    run()
