"""Bind actual JDK Locale data and computed context observations to the current module batch."""
import datetime
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path
from freeze import ROOT, DOC, SOURCE
from qualify import read,sha
from verify import need

PARENT=ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites-gate/run_20261118_044535_5f39165d'


def run():
    need(sha(PARENT/'catalog.json')=='9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb','Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent member drift')
    runtime=read(PARENT/'oracle/oracle-result.json')['original_runtime_inputs']
    for name,digest in runtime.items():need(sha(Path(name))==digest,'Original runtime drift')
    for row in read(DOC/'inventory.json')['objects']:need(sha(SOURCE/row['java'])==row['sha256'],'Original source drift')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_locale_'+stamp);out=ROOT/'docs/phase-2/evidence/sqlparser-locale'/('run_'+stamp)
    (work/'classes').mkdir(parents=True);out.mkdir(parents=True)
    jdk=Path('D:/Java/jdk-17.0.12')
    jdk_inputs={str(p):sha(p) for p in [jdk/'bin/java.exe',jdk/'bin/javac.exe',jdk/'lib/modules',jdk/'lib/src.zip',jdk/'release']}
    sources=[ROOT/'tools/formal-sqlparser-core'/name for name in ['MetadataLocaleProbe.java','MetadataLocaleOracle.java']]
    for source in sources:shutil.copy2(source,out/source.name)
    shutil.copy2(__file__,out/'runner-executed.py')
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    commands=[]
    def invoke(argv,name):
        with (out/name).open('wb') as log:code=subprocess.run(list(map(str,argv)),cwd=work,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(work),exit_code=code,log=name))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8');need(code==0,'Actual JDK execution failed: '+name)
    invoke([jdk/'bin/javac.exe','-encoding','UTF-8','-d',work/'classes']+sources,'compile.log')
    java=[jdk/'bin/java.exe','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work/'home'),'--add-opens','java.base/java.lang=ALL-UNNAMED','--add-opens','java.base/sun.text=ALL-UNNAMED','-cp',work/'classes']
    invoke(java+['MetadataLocaleProbe',out/'profiles'],'probe.log')
    invoke(java+['MetadataLocaleOracle',out],'oracle.log')
    for name,digest in jdk_inputs.items():need(sha(Path(name))==digest,'Actual JDK drift during execution')
    family_lines=(out/'profiles/probe.tsv').read_text('utf-8').splitlines()[:2]
    need(family_lines[0].startswith('sun.text.RuleBasedBreakIterator:') and family_lines[1].startswith('sun.text.DictionaryBasedBreakIterator:'),'Unexpected word profile families')
    for name in ['jdk17-string-casing-input.bin','jdk17-word-root-input.bin','jdk17-word-th-input.bin','jdk17-word-th-dictionary-input.bin']:
        target=ROOT/'crates/seata_sqlparser/src/data'/name.replace('-','_')
        if target.exists():need(sha(target)==sha(out/name),'Existing generated input differs')
        else:shutil.copy2(out/name,target)
    fixture=ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/jdk17-locale-observations.tsv'
    if fixture.exists():need(sha(fixture)==sha(out/'jdk17-locale-observations.tsv'),'Existing computed fixture differs')
    else:shutil.copy2(out/'jdk17-locale-observations.tsv',fixture)
    result=dict(status='ACTUAL_JDK17_LOCALE_CONTEXT_OBSERVED',out=str(out),work=str(work),parent=PARENT.relative_to(ROOT).as_posix(),
        parent_catalog_sha256=sha(PARENT/'catalog.json'),original_runtime_inputs=runtime,jdk_runtime_inputs=jdk_inputs,
        producer_sources=[dict(path=str(source),sha256=sha(source)) for source in sources],
        producer_classes=[dict(path=str(p),sha256=sha(p)) for p in sorted((work/'classes').glob('*.class'))],
        data_role='GENERATED_COMPATIBILITY_INPUTS_NOT_MATCH_RESULTS',full_core_complete=False)
    (out/'oracle-result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(out=str(out),origin=(out/'locale-origin.tsv').read_text('utf-8'))),flush=True)


if __name__=='__main__':run()
