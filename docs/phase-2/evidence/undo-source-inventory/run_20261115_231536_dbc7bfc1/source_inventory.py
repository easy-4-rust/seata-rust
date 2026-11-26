"""JDK AST declaration inventory only: inherited/parameterized/dynamic runtime cases remain unresolved."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'
PIN='cba73e180fd8896a99df6415b859d275122feaa7'


def sha(data):return hashlib.sha256(data).hexdigest()


def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/undo-source-inventory'/('run_'+stamp)
    work=Path('D:/codex-builds')/('seata_source_inventory_'+stamp)
    out.mkdir(parents=True);(work/'classes').mkdir(parents=True)
    print('UNDO_SOURCE_INVENTORY_RUN '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp')
    for key in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(key,None)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,value):put(name,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        result=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=120)
        put(name,result.stdout)
        if result.returncode:raise ValueError(name+' failed')
        return result.stdout
    if command(['git','-C',JAVA,'rev-parse','HEAD'],'java-pin.txt').decode().strip()!=PIN:raise ValueError('Original source pin differs')
    source=Path(__file__).with_name('SourceCaseInventory.java')
    put('SourceCaseInventory.java',source.read_bytes());put('source_inventory.py',Path(__file__).read_bytes())
    directories=[JAVA/'rm-datasource/src/test',JAVA/'sqlparser/seata-sqlparser-core/src/test']
    inputs=[];assets=[]
    for directory in directories:
        for path in sorted(directory.rglob('*')):
            if not path.is_file():continue
            raw=path.read_bytes();relative=path.relative_to(JAVA).as_posix()
            blob=subprocess.check_output(['git','-C',str(JAVA),'show',PIN+':'+relative],creationflags=subprocess.CREATE_NO_WINDOW)
            if path.suffix=='.java':
                if raw.replace(b'\r\n',b'\n')!=blob.replace(b'\r\n',b'\n'):raise ValueError('Source content differs: '+relative)
            snapshot='inputs/'+sha(relative.encode())[:24]+'.bin';put(snapshot,raw)
            row=dict(source=relative,checkout_sha256=sha(raw),git_blob_sha256=sha(blob),snapshot=snapshot,
                checkout_line_endings_only=path.suffix=='.java' and raw!=blob)
            inputs.append(row)
            if path.suffix!='.java':
                target=ROOT/'tests/fixtures/mysql/java_source_assets'/relative
                if target.read_bytes()!=raw:raise ValueError('Existing copied source asset differs')
                canonical=ROOT/'tests/fixtures/mysql/java_git_assets'/relative
                canonical.parent.mkdir(parents=True,exist_ok=True)
                if canonical.exists() and canonical.read_bytes()!=blob:raise ValueError('Existing pinned asset differs')
                if not canonical.exists():canonical.write_bytes(blob)
                git_snapshot='git-assets/'+sha(relative.encode())[:24]+'.bin';put(git_snapshot,blob)
                assets.append(row|dict(checkout_copy_target=target.relative_to(ROOT).as_posix(),checkout_copy_byte_identical=True,
                    canonical_Git_target=canonical.relative_to(ROOT).as_posix(),canonical_Git_snapshot=git_snapshot,
                    canonical_Git_byte_identical=True,checkout_differs_from_Git=raw!=blob,
                    disposition='PINNED_SOURCE_ASSET_COPIED_ONLY_NOT_CASE_PARITY'))
    save('source-inputs.json',inputs);save('source-assets.json',assets)
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-version'],'java-version.log')
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-d',work/'classes',source],'compile.log')
    artifact=[]
    for p in sorted((work/'classes').rglob('*.class')):
        name='classes/'+p.relative_to(work/'classes').as_posix();put(name,p.read_bytes());artifact.append(dict(path=str(p),snapshot=name,sha256=sha(p.read_bytes())))
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),
        '-cp',work/'classes','SourceCaseInventory',work/'declared-methods.jsonl']+directories,'inventory.log')
    data=(work/'declared-methods.jsonl').read_bytes();put('declared-methods.jsonl',data)
    methods=[json.loads(line) for line in data.splitlines()]
    if len(methods)!=1135:raise ValueError('Recognized declaration denominator changed')
    known={str(JAVA/x['source']):x['checkout_sha256'] for x in inputs}
    for method in methods:method['source_checkout_sha256']=known[method['source']]
    save('declared-methods.json',methods);save('test-artifacts.json',artifact)
    for row in inputs:
        if sha((JAVA/row['source']).read_bytes())!=row['checkout_sha256']:raise ValueError('Inventory source drift')
    save('result.json',dict(status='V0_STATIC_DECLARATIONS_ONLY',java_pin=PIN,source_files=189,recognized_declared_test_methods=len(methods),
        source_assets=7,concrete_runtime_cases_expanded=False,Java_tests_executed=False,Rust_cases_migrated_by_this_tool=0,
        declarations_MISSING=len(methods),known_limitations=['Annotation simple names parsed, composed annotation semantics unresolved',
            'Superclass syntax recorded, inherited concrete runner identities unresolved','Parameterized repeated template dynamic invocation expansion unresolved'],
        source_case_parity_complete=False,WP05_complete=False))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p.read_bytes())) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha((out/'catalog.json').read_bytes()))),flush=True)


if __name__=='__main__':run()
