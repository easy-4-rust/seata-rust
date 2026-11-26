"""Execute all original core JUnit and cumulative76 source peers against immutable original classes."""
import datetime
import json
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path
from freeze import ROOT,DOC,SOURCE,MODULE
from qualify import read,sha
from verify import need

PARENT=ROOT/'docs/phase-2/evidence/sqlparser-locale-gate/run_20261118_052535_fb6c7ec8'


def run():
    need(sha(PARENT/'catalog.json')=='c411a512731f65f154c5767bbb3b015af1ee14156537497eed4c4ab232cb34e9','Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent member drift')
    receipt=read(PARENT/'oracle/oracle-result.json');runtime=receipt['original_runtime_inputs']
    for name,digest in (runtime|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Original runtime drift')
    for row in read(DOC/'inventory.json')['objects']:need(sha(SOURCE/row['java'])==row['sha256'],'Original source drift')
    table_parent=ROOT/read(PARENT/'result.json')['parent'];index_parent=ROOT/read(table_parent/'result.json')['parent']
    template=read(index_parent/'oracle/commands.json')[-1]['argv'];cp_pos=template.index('-cp')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_table_metadata_'+stamp);out=ROOT/'docs/phase-2/evidence/sqlparser-table'/('run_'+stamp)
    (work/'classes').mkdir(parents=True);out.mkdir(parents=True)
    cp=os.pathsep.join([str(work/'classes')]+template[cp_pos+1].split(os.pathsep)[1:])
    java=['-Duser.home='+str(work/'home') if arg.startswith('-Duser.home=') else arg for arg in template[:cp_pos]]
    jdk=Path(java[0]).parent;env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    sources=[ROOT/'tools/formal-sqlparser-core'/(name+'.java') for name in ['CoreValueOracle','ColumnMetaOracle','IndexMetaOracle','TableMetaOracle']]
    for source in sources:shutil.copy2(source,out/source.name)
    shutil.copy2(__file__,out/'runner-executed.py');commands=[]
    def invoke(argv,name):
        with(out/name).open('wb')as log:code=subprocess.run(list(map(str,argv)),cwd=work,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(work),exit_code=code,log=name));(out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code==0,'Original Java failed: '+name)
    invoke([jdk/'javac.exe','-encoding','UTF-8','-cp',cp,'-d',work/'classes']+sources,'compile.log')
    invoke(java+['-cp',cp,'SourceJUnitRunner',MODULE/'target/test-classes',out/'junit-events.jsonl','execute'],'junit.log')
    for name,files in [('CoreValueOracle',['foundation-java.tsv','constants-java.tsv','foundation-origin.tsv']),
        ('ColumnMetaOracle',['column-java.tsv','column-edges-java.tsv','column-origin.tsv']),
        ('IndexMetaOracle',['index-java.tsv','index-edges-java.tsv','index-origin.tsv'])]:
        invoke(java+['-cp',cp,name]+[out/file for file in files],name+'.log')
    invoke(java+['-cp',cp,'TableMetaOracle',out],'TableMetaOracle.log')
    sys.path.insert(0,str(ROOT/'tools/formal-source-junit'));from validate import load,validate_events,controls
    events=load(out/'junit-events.jsonl');validation=validate_events(events,'seata-sqlparser-core');negative=controls(events,'seata-sqlparser-core')
    for name,digest in (runtime|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Runtime changed during execution')
    for name in ['primary-hash-java.tsv','table-edges-java.tsv']:
        target=ROOT/'crates/seata_test/tests/fixtures/sqlparser_core'/name
        if target.exists():need(sha(target)==sha(out/name),'Existing derived fixture differs; preserve prior JVM context and use runtime override')
        else:shutil.copy2(out/name,target)
    result=dict(status='ORIGINAL_CORE76_EXECUTED_AND_CUMULATIVE76_OBSERVED',out=str(out),work=str(work),
        parent=PARENT.relative_to(ROOT).as_posix(),parent_catalog_sha256=sha(PARENT/'catalog.json'),
        original_runtime_inputs=runtime,jdk_runtime_inputs=receipt['jdk_runtime_inputs'],
        original_JUnit_validation=validation,original_JUnit_counterexamples=negative,
        producer_sources=[dict(path=str(p),sha256=sha(p))for p in sources],
        producer_classes=[dict(path=str(p),sha256=sha(p))for p in sorted((work/'classes').glob('*.class'))],
        source_cases_observed=76,original_source_changes=False,full_core_complete=False)
    (out/'oracle-result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(out=str(out),source_cases=76,hash_order_observations=len((out/'primary-hash-java.tsv').read_text('utf-8').splitlines()),table_edges=len((out/'table-edges-java.tsv').read_text('utf-8').splitlines()))),flush=True)


if __name__=='__main__':run()
