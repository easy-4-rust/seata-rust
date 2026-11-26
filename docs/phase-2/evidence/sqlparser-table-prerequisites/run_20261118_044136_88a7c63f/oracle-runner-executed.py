"""Observe the pinned JDK comparator and original TableMeta before implementing its dependency."""
import datetime
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path
from freeze import ROOT, DOC, SOURCE
from qualify import read, sha
from verify import need

PARENT = ROOT/'docs/phase-2/evidence/sqlparser-index-gate/run_20261118_042124_511b04b6'


def run():
    need(sha(PARENT/'catalog.json') == '6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774', 'Parent differs')
    for row in read(PARENT/'catalog.json')['members']: need(sha(PARENT/row['path']) == row['sha256'], 'Parent drift')
    runtime = read(PARENT/'oracle/oracle-result.json')['original_runtime_inputs']
    for name,digest in runtime.items(): need(sha(Path(name)) == digest, 'Original runtime drift')
    for row in read(DOC/'inventory.json')['objects']: need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds')/('seata_metadata_case_'+stamp)
    output = ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites'/('run_'+stamp)
    (work/'classes').mkdir(parents=True); output.mkdir(parents=True)
    template = read(PARENT/'oracle/commands.json')[-1]['argv']; pos=template.index('-cp')
    cp = os.pathsep.join([str(work/'classes')]+template[pos+1].split(os.pathsep)[1:])
    java = template[:pos]; jdk=Path(java[0]).parent
    java = ['-Duser.home='+str(work/'home') if s.startswith('-Duser.home=') else s for s in java]
    jdk_runtime = {str(path):sha(path) for path in [jdk/'java.exe',jdk/'javac.exe',jdk.parent/'lib/modules',jdk.parent/'release',jdk.parent/'lib/src.zip']}
    producer=ROOT/'tools/formal-sqlparser-core/MetadataCaseOracle.java'
    shutil.copy2(producer, output/producer.name); shutil.copy2(__file__, output/'oracle-runner-executed.py')
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    commands=[]
    for name,argv in [('compile.log',[jdk/'javac.exe','-encoding','UTF-8','-cp',cp,'-d',work/'classes',producer]),
                      ('run.log',java+['-cp',cp,'MetadataCaseOracle',output])]:
        with (output/name).open('wb') as log:
            code=subprocess.run(list(map(str,argv)),cwd=work,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(work),exit_code=code,log=name))
        (output/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code==0,'Original comparator peer failed: '+name)
    for name,digest in runtime.items(): need(sha(Path(name)) == digest, 'Runtime drift during execution')
    for name,digest in jdk_runtime.items(): need(sha(Path(name)) == digest, 'JDK runtime drift during execution')
    # Generated runtime compatibility data is a distinct production input, never a source test asset.
    binary=ROOT/'crates/seata_sqlparser/src/data/java17_character_case.bin';binary.parent.mkdir(exist_ok=True)
    if binary.exists():need(sha(binary)==sha(output/'java17-character-case-input.bin'),'Existing runtime map differs')
    else:shutil.copy2(output/'java17-character-case-input.bin',binary)
    fixture=ROOT/'crates/seata_test/tests/fixtures/sqlparser_core'
    for name in ['java17-comparisons.tsv','table-primary-sets.tsv']:
        target=fixture/(name if name=='java17-comparisons.tsv' else 'table-primary-sets-v2.tsv')
        if target.exists():need(sha(target)==sha(output/name),'Existing comparator fixture differs')
        else:shutil.copy2(output/name,target)
    result=dict(status='JAVA_COMPARATOR_AND_ORIGINAL_TABLE_PRIMARY_SET_OBSERVED',out=str(output),work=str(work),
        parent=PARENT.relative_to(ROOT).as_posix(),parent_catalog_sha256=sha(PARENT/'catalog.json'),
        original_runtime_inputs=runtime,producer_source_sha256=sha(producer),
        jdk_runtime_inputs=jdk_runtime,
        producer_classes=[dict(path=str(path),sha256=sha(path)) for path in sorted((work/'classes').glob('*.class'))],
        JDK_src_zip_sha256=sha(jdk.parent/'lib/src.zip'),
        data_role='CHARACTER_MAP_IS_INPUT_NOT_COMPUTED_MATCH',full_core_complete=False)
    (output/'oracle-result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(out=str(output),origin=(output/'case-origin.tsv').read_text('utf-8'))),flush=True)


if __name__ == '__main__':run()
