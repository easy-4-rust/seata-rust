"""Bounded actual-JDK search for the surviving optimization candidate; never an equivalence proof."""
import argparse
import datetime
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path
from freeze import ROOT
from qualify import read,sha
from verify import need


def run(oracle):
    receipt=read(oracle/'oracle-result.json');stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_order_search_'+stamp);out=oracle/'order-diagnostics'/('run_'+stamp)
    (work/'classes').mkdir(parents=True);out.mkdir(parents=True)
    source=ROOT/'tools/formal-sqlparser-core/LocaleBoundaryOrderProbe.java';shutil.copy2(source,out/source.name);shutil.copy2(__file__,out/'runner-executed.py')
    jdk=Path('D:/Java/jdk-17.0.12')
    for path,digest in receipt['jdk_runtime_inputs'].items():need(sha(Path(path))==digest,'JDK before search drift')
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    commands=[]
    for label,argv in [('compile.log',[jdk/'bin/javac.exe','-encoding','UTF-8','-d',work/'classes',source]),
        ('search.log',[jdk/'bin/java.exe','--add-opens=java.base/java.lang=ALL-UNNAMED','--add-opens=java.base/sun.text=ALL-UNNAMED','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work/'home'),'-cp',work/'classes','LocaleBoundaryOrderProbe',out/'search.tsv'])]:
        with (out/label).open('wb') as log:code=subprocess.run(list(map(str,argv)),cwd=work,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(work),exit_code=code,log=label));need(code==0,'Search command failed')
    (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
    need((out/'search.tsv').read_text('utf-8')=='checked\t353922\nNO_COUNTEREXAMPLE_IN_DECLARED_SEARCH\ncursor-previous\t2\t55296\t1\n','Bounded search or actual private cursor outcome changed')
    for path,digest in receipt['jdk_runtime_inputs'].items():need(sha(Path(path))==digest,'JDK after search drift')
    result=dict(status='BOUNDED_SEARCH_NO_COUNTEREXAMPLE_NOT_EQUIVALENCE_PROOF',actual_JDK_searches=353922,
        scope='ROOT/th,14 declared fragments,depth<=5,only strings containing Sigma',equivalence_proven=False,
        source_sha256=sha(source),producer_classes=[dict(path=str(p),sha256=sha(p)) for p in (work/'classes').glob('*.class')],
        jdk_runtime_inputs=receipt['jdk_runtime_inputs'],not_source_test_coverage=True)
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(out=str(out),searches=353922,equivalence_proven=False)),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--oracle',type=Path,required=True);run(parser.parse_args().oracle.resolve())
