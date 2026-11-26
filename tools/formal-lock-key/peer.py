"""Actual pinned BaseTransactionalExecutor/ConnectionContext versus Rust lock keys, including Java HashMap order."""
import datetime
import hashlib
import itertools
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import uuid
import zipfile

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
REFERENCE=ROOT/'docs/phase-2/evidence/undo-codec-refinement/run_20261115_235850_b36442a0'
sys.path.insert(0,str(ROOT/'tools/formal-undo-format'))
from runtime_binding import RuntimeBinding
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def corpus():
    result=[];rng=random.Random(20261116)
    def add(name,names,rows=None,**extra):
        result.append(dict(case=name,table='lock_fixture',names=names,rows=rows if rows is not None else [list(range(1,len(names)+1))],**extra))
    add('reverse-index',['b','a'],[[2,1]],expected='lock_fixture:1_2')
    for count in range(1,129):
        names=['p_'+str(i).zfill(3) for i in range(count)]
        for mode in ['normal','reverse','shuffle']:
            current=names.copy()
            if mode=='reverse':current.reverse()
            if mode=='shuffle':rng.shuffle(current)
            add(f'capacity-{count}-{mode}',current)
    collisions=[''.join(parts) for parts in itertools.product(['Aa','BB'],repeat=7)]
    for count in range(1,129):
        add(f'collision-{count}',collisions[:count]);add(f'collision-reverse-{count}',list(reversed(collisions[:count])))
    for count in [9,10,11,12,16,24,48,49,96,97,128]:
        for iteration in range(8):
            names=collisions[:count].copy();rng.shuffle(names);add(f'tree-{count}-{iteration}',names)
    for iteration in range(256):
        count=rng.randint(1,128);names=rng.sample(collisions,count//2)+[f'q_{iteration}_{index}' for index in range(count-count//2)]
        rng.shuffle(names);add(f'mixed-{iteration}',names)
    def spread(name):
        value=0
        for c in name:value=(31*value+ord(c))&0xffffffff
        return value^(value>>16)
    group=[];other=[];index=0
    while len(group)<64 or len(other)<128:
        name='key_'+str(index);index+=1
        (group if spread(name)&63==0 else other).append(name)
    for dense in [11,16,24,32,48,64]:
        for total in [49,97,128]:
            if total>=dense:add(f'tree-split-{dense}-{total}',group[:dense]+other[:total-dense])
    source=[
        dict(case='source-single',table='test_name',names=['id'],types=['text'],rows=[['1'],['2']],expected='test_name:1,2'),
        dict(case='source-composite',table='test_name',names=['id','userId'],types=['text','text'],rows=[['1','one'],['2','two']],expected='test_name:1_one,2_two'),
        dict(case='source-binary',table='test_binary_table',names=['binary_id'],types=['binary'],rows=[[dict(kind='binary',text='0102030fff')],[dict(kind='binary',text='0a141e')]],expected='test_binary_table:[1, 2, 3, 15, -1],[10, 20, 30]'),
        dict(case='source-mixed',table='test_mixed_table',names=['user_id','session_id'],types=['text','binary'],rows=[['user123',dict(kind='binary',text='10203040')]],expected='test_mixed_table:user123_[16, 32, 48, 64]'),
    ];result.extend(source)
    add('empty',['id'],[])
    for value in ['0','0.00','0.000000','0.0000000','0.0000001','-0.0000001','0.00000010','-0.000000000000000000000000000001','12345.1200']:
        add('decimal-'+str(len(result)),['id'],[[dict(kind='decimal',text=value)]],types=['decimal'])
    add('unsigned-maximum',['id'],[[dict(kind='bigint',text='18446744073709551615')]],types=['unsigned'])
    for value in ['false','true']:
        add('tiny-default-'+value,['id'],[[dict(kind='boolean',text=value)]],types=['tiny'],profile='default')
    for value in [0,1,2,-128,127]:add('tiny-numeric-'+str(value),['id'],[[value]],types=['tiny'])
    for value in ['', ' ', 'x;y:z','中文🦀']:
        add('text-source-'+str(len(result)),['id'],[[value]],types=['text'])
    add('text-comma',['id'],[['a,b']],types=['text'])
    return result

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/lock-key-peer'/('run_'+stamp)
    work=Path('D:/codex-builds')/('seata_lock_key_peer_'+stamp)
    out.mkdir(parents=True);(work/'classes').mkdir(parents=True);(work/'lib').mkdir()
    print('LOCK_KEY_PEER_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','SEATA_CONFIG_NAME','SEATA_ENV']:env.pop(name,None)
    def put(name,data):
        path=out/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        value=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(name,value.stdout);need(value.returncode==0,name+' failed');return value.stdout
    for row in read(BUILD/'catalog.json')['members']:need(sha(BUILD/row['path'])==row['sha256'],'Original build archive drift')
    binding=RuntimeBinding(BUILD,JAVA);old=read(BUILD/'build.json')
    extra=[]
    for name in ['seata-config-core-2.8.0-SNAPSHOT.jar','config-1.2.1.jar','json-common-core-2.8.0-SNAPSHOT.jar','netty-common-4.2.12.Final.jar']:
        source=JAVA/'server/target/lib'/name;target=work/'lib'/name;target.write_bytes(source.read_bytes());put('lib/'+name,target.read_bytes())
        binding.files[target.resolve()]=dict(sha256=sha(target),archive=str(out/'lib'/name),role='lock_oracle_library');extra.append(source)
    sources=[JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource/exec/BaseTransactionalExecutor.java',
        JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource/ConnectionContext.java',JAVA/'common/src/main/java/org/apache/seata/common/util/ArrayUtils.java']
    for source in sources:
        relative=source.relative_to(JAVA).as_posix();blob=subprocess.check_output(['git','-C',str(JAVA),'show','cba73e180fd8896a99df6415b859d275122feaa7:'+relative],creationflags=subprocess.CREATE_NO_WINDOW)
        need(source.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Selected original source differs')
    paths={ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'tools/formal-undo-format/runtime_binding.py',ROOT/'tools/formal-lock-key/LockKeyOracle.java',ROOT/'docs/phase-2/global-lock-key-contract.md'}|set(extra+sources)
    paths.update(p for p in (ROOT/'crates').rglob('*') if p.is_file())
    dependencies=[]
    for row in read(REFERENCE/'source-inputs.json'):
        if 'registry' in Path(row['path']).parts:
            need(sha(Path(row['path']))==row['sha256'],'Locked dependency input drift');dependencies.append(row)
    save('dependency-binding.json',dict(reference=str(REFERENCE),catalog_sha256=sha(REFERENCE/'catalog.json'),unchanged_registry_inputs=len(dependencies)))
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(snapshot,path.read_bytes());frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    save('source-inputs.json',frozen)
    with zipfile.ZipFile('D:/Java/jdk-17.0.12/lib/src.zip') as archive:put('JDK-reference/HashMap.java',archive.read('java.base/java/util/HashMap.java'))
    save('JDK-reference/origin.json',dict(source_zip='D:/Java/jdk-17.0.12/lib/src.zip',source_zip_sha256=sha(Path('D:/Java/jdk-17.0.12/lib/src.zip')),member='java.base/java/util/HashMap.java',member_sha256=sha(out/'JDK-reference/HashMap.java')))
    raw=command(['cargo','build','--locked','--offline','-p','seata-at','--example','lock_key_peer','--message-format=json'],'rust-build.jsonl')
    artifacts=[]
    for line in raw.splitlines():
        try:row=json.loads(line)
        except ValueError:continue
        if row.get('reason')=='compiler-artifact' and row['target']['name']=='lock_key_peer' and row.get('executable'):artifacts.append(Path(row['executable']))
    need(len(artifacts)==1,'Rust peer missing');binary=artifacts[0];digest=sha(binary);put('artifacts/'+binary.name,binary.read_bytes());save('rust-artifact.json',dict(path=str(binary),sha256=digest))
    cases=corpus();(work/'input.json').write_text(json.dumps(cases),encoding='utf-8');save('inputs.json',cases)
    command([binary,work/'input.json',work/'candidates.json'],'rust-peer.log');put('candidates.json',(work/'candidates.json').read_bytes())
    cp=old['classpath']+os.pathsep+os.pathsep.join(str(work/'lib'/p.name) for p in extra)
    save('runtime.json',dict(runtime=str(work),classpath=str(work/'classes')+os.pathsep+cp,original_build=str(BUILD)))
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-J-Duser.language=en','-encoding','UTF-8','-cp',cp,'-d',work/'classes']+sources+[ROOT/'tools/formal-lock-key/LockKeyOracle.java'],'java-build.log')
    classes=[]
    for path in (work/'classes').rglob('*.class'):
        snapshot='classes/'+path.relative_to(work/'classes').as_posix();put(snapshot,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/snapshot),role='lock_selected_class');classes.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    for name,data in [('classes/registry.conf','config { type = "file"\n file { name = "'+(work/'file.conf').as_posix()+'" } }\n'),('file.conf','client { undo { onlyCareUpdateColumns = false } }\n')]:
        path=work/name;path.write_text(data,encoding='utf-8');put('runtime-config/'+path.name,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/'runtime-config'/path.name),role='private_file_configuration')
    binding.roots.append((work/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest());save('compiled-classes.json',classes)
    loaded=work/'classes.loaded.log'
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-Duser.language=en','-Dseata.config.name=registry','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),
        '-Dlogback.configurationFile='+str(Path(old['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loaded.as_posix(),'-cp',str(work/'classes')+os.pathsep+cp,
        'org.apache.seata.rm.datasource.LockKeyOracle',work/'candidates.json',work/'comparison.json'],'java-peer.log')
    put('classes.loaded.log',loaded.read_bytes());origins=binding.loaded(loaded,None,True,'lock')
    for name in ['org.apache.seata.rm.datasource.exec.BaseTransactionalExecutor','org.apache.seata.rm.datasource.ConnectionContext','org.apache.seata.common.util.ArrayUtils']:
        need(Path(origins['bound_origins'][name]['actual_file'])==work/'classes'/(name.replace('.','/')+'.class'),'New selected original source was shadowed')
    save('origin-binding.json',origins);binding.verify();put('comparison.json',(work/'comparison.json').read_bytes())
    need(len(read(work/'comparison.json'))==len(cases),'Missing comparison rows')
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Peer source drift')
    need(sha(binary)==digest,'Executed Rust binary drift')
    save('result.json',dict(status='SELECTED_ORIGINAL_LOCK_KEY_DIFFERENTIAL_PASSED',cases=len(cases),Java_Rust_MATCH=len(cases),original_source_value_probes=4,
        original_JUnit_tests_executed=False,original_newly_compiled_sources=3,Java_source_case_parity_complete=False,
        actual_TC_lock_coverage_verified=False,SQL_Undo_atomicity_verified=False,TC_registration_verified=False,public_COMMIT_available=False,WP05_complete=False))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)

if __name__=='__main__':run()
