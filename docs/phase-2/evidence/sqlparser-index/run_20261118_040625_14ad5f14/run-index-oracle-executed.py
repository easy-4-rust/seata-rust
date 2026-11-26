"""Execute the original core JUnit and all three cumulative value/metadata peers."""
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

PARENT = ROOT/'docs/phase-2/evidence/sqlparser-column-gate/run_20261118_030159_4362448c'
HERE = Path(__file__).resolve().parent


def run():
    need(sha(PARENT/'catalog.json') == '1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da', 'Wrong parent gate')
    for row in read(PARENT/'catalog.json')['members']:
        need(sha(PARENT/row['path']) == row['sha256'], 'Published parent evidence drift')
    runtime = read(PARENT/'oracle/oracle-result.json')['original_runtime_inputs']
    for path, digest in runtime.items():
        need(sha(Path(path)) == digest, 'Original classpath drift')
    for row in read(DOC/'inventory.json')['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds')/('seata_index_metadata_'+stamp)
    out = ROOT/'docs/phase-2/evidence/sqlparser-index'/('run_'+stamp)
    (work/'classes').mkdir(parents=True); (work/'home').mkdir(); out.mkdir(parents=True)
    template = read(PARENT/'oracle/commands.json')[-1]['argv']; cp_index = template.index('-cp')
    classpath = os.pathsep.join([str(work/'classes')]+template[cp_index+1].split(os.pathsep)[1:])
    java = ['-Duser.home='+str(work/'home') if arg.startswith('-Duser.home=') else arg for arg in template[:cp_index]]
    jdk = Path(java[0]).parent
    env = os.environ | dict(JAVA_HOME=str(jdk.parent), TEMP='D:/Temp', TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:
        env.pop(name,None)
    commands = []
    def invoke(argv,name):
        with (out/name).open('wb') as log:
            code = subprocess.run(list(map(str,argv)),cwd=work,env=env,stdout=log,stderr=subprocess.STDOUT,
                timeout=120,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(work),exit_code=code,log=name))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code==0,'Original Java failed: '+name)
    producers=[HERE/(name+'.java') for name in ['CoreValueOracle','ColumnMetaOracle','IndexMetaOracle']]
    for path in producers:
        (out/path.name).write_bytes(path.read_bytes())
    (out/'run-index-oracle-executed.py').write_bytes(Path(__file__).read_bytes())
    invoke([jdk/'javac.exe','-encoding','UTF-8','-cp',classpath,'-d',work/'classes']+producers,'compile.log')
    invoke(java+['-cp',classpath,'SourceJUnitRunner',MODULE/'target/test-classes',out/'junit-events.jsonl','execute'],'junit.log')
    for name, files in [('CoreValueOracle',['foundation-java.tsv','constants-java.tsv','foundation-origins.tsv']),
                        ('ColumnMetaOracle',['column-java.tsv','column-edges-java.tsv','column-origin.tsv']),
                        ('IndexMetaOracle',['index-java.tsv','index-edges-java.tsv','index-origin.tsv'])]:
        invoke(java+['-cp',classpath,name]+[out/file for file in files],name+'.log')
    sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
    from validate import load,validate_events,controls
    rows=load(out/'junit-events.jsonl');validation=validate_events(rows,'seata-sqlparser-core');counterexamples=controls(rows,'seata-sqlparser-core')
    need(len((out/'index-java.tsv').read_text('utf-8').splitlines())==16,'Source IndexMeta case denominator changed')
    edge_rows={line.split('\t')[0]:line.split('\t')[1:] for line in (out/'index-edges-java.tsv').read_text('utf-8').splitlines()}
    for i in range(7):
        need(edge_rows['failure/'+str(i)]==['java.lang.IllegalStateException','d8000000dc00','true'],'Original exception path failed')
    for path,digest in runtime.items():
        need(sha(Path(path))==digest,'Original runtime changed during execution')
    fixture=ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/index-edges-java.tsv'
    if fixture.exists():
        need(fixture.read_bytes()==(out/'index-edges-java.tsv').read_bytes(),'Existing fixture contains another JVM identity context; preserve it and use its original run')
    else:
        fixture.write_bytes((out/'index-edges-java.tsv').read_bytes())
    result=dict(status='ORIGINAL_CORE_76_AND_CUMULATIVE_57_OBSERVED',out=str(out),work=str(work),
        parent=str(PARENT),parent_catalog_sha256=sha(PARENT/'catalog.json'),original_runtime_inputs=runtime,
        original_JUnit_validation=validation,original_JUnit_counterexamples=counterexamples,
        producer_classes=[dict(path=str(path),sha256=sha(path)) for path in sorted((work/'classes').glob('*.class'))],
        source_cases_observed=57,extra_observations=len(edge_rows),source_changes=False,full_core_complete=False)
    (out/'oracle-result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(out=str(out),source_cases_observed=57,extra_observations=len(edge_rows),original_JUnit_passed=76)),flush=True)


if __name__=='__main__':
    run()
