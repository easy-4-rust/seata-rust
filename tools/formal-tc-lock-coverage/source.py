"""Freeze and invoke original TC source methods with a recording backend; never a network/storage safety claim."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
LOCK=ROOT/'docs/phase-2/evidence/lock-key-peer/run_20261116_004741_eaf663fb'
sys.path.insert(0,str(ROOT/'tools/formal-undo-format'))
from runtime_binding import RuntimeBinding
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def cases():
    def row(table,pk):return dict(table=table,pk=pk)
    values=[
        ('normal','t:1',[row('t','1')]),('empty','t:',[]),('blank','t: ',[]),
        ('unicode-blank','t:\u2003',[]),('nonbreaking-space','t:\u00a0',[row('t','\u00a0')]),
        ('broken-middle','t:x;broken;later:2',[row('t','x')]),
        ('broken-first','broken;t:1',[]),('empty-first','t:;later:2',[]),
        ('injected-valid-group','t:x;fake:y;later:2',[row('t','x'),row('fake','y'),row('later','2')]),
        ('binary-a','t:[0, -1]',[row('t','[0'),row('t',' -1]')]),
        ('binary-b','t:[0, -2]',[row('t','[0'),row('t',' -2]')]),
        ('partial-empty-row','t:,1',[row('t','1')]),('partial-blank-row','t:1, ',[row('t','1')]),
        ('trailing-delimiters','t:1,,',[row('t','1')]),('normal-multi-table','t:x;later:2',[row('t','x'),row('later','2')]),
    ]
    return [dict(case=name,key=key,expected_rows=rows,expected_backend_calls=int(bool(rows))) for name,key,rows in values]

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/tc-lock-source'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata_tc_lock_source_'+stamp);(work/'classes').mkdir(parents=True)
    print('TC_LOCK_SOURCE_RUN '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','SEATA_CONFIG_NAME','SEATA_ENV']:env.pop(name,None)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        value=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(name,value.stdout);need(value.returncode==0,name+' failed');return value.stdout
    for packet in [BUILD,LOCK]:
        for row in read(packet/'catalog.json')['members']:need(sha(packet/row['path'])==row['sha256'],'Inherited classpath archive differs')
    binding=RuntimeBinding(BUILD,JAVA);lock=read(LOCK/'runtime.json')
    for row in read(LOCK/'runtime-inputs.json'):
        path=Path(row['path']).resolve()
        if path not in binding.files:
            need(sha(Path(row['archive']))==row['sha256'],'Lock library archive differs')
            binding.files[path]={name:row[name] for name in ['sha256','archive','role']}
    binding.roots.append((Path(lock['runtime'])/'classes').resolve());binding.verify()
    source=JAVA/'server/src/main/java/org/apache/seata/server/lock/AbstractLockManager.java'
    original=source.relative_to(JAVA).as_posix()
    need(command(['git','-C',JAVA,'rev-parse','HEAD'],'java-pin.txt').decode().strip()=='cba73e180fd8896a99df6415b859d275122feaa7','Java pin differs')
    blob=subprocess.check_output(['git','-C',str(JAVA),'show','cba73e180fd8896a99df6415b859d275122feaa7:'+original],creationflags=subprocess.CREATE_NO_WINDOW)
    need(source.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Original TC source differs');put('original-Git-blob.java',blob)
    fallback=JAVA/'server/target/classes';shutil.copytree(fallback,work/'server-fallback')
    paths={Path(__file__),ROOT/'tools/formal-tc-lock-coverage/LockCoverageOracle.java',ROOT/'tools/formal-undo-format/runtime_binding.py',source,
        ROOT/'docs/phase-2/tc-lock-coverage-contract.md',LOCK/'catalog.json',LOCK/'runtime.json'}
    frozen=[]
    for path in sorted(paths|{p for p in fallback.rglob('*') if p.is_file()}):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(snapshot,path.read_bytes());frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
        if path.is_relative_to(fallback):
            target=work/'server-fallback'/path.relative_to(fallback)
            binding.files[target.resolve()]=dict(sha256=sha(target),archive=str(out/snapshot),role='frozen_server_fallback')
    save('source-inputs.json',frozen);binding.roots.append((work/'server-fallback').resolve());binding.verify()
    cp=lock['classpath']+os.pathsep+str(work/'server-fallback')
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-version'],'java-version.txt')
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-J-Duser.language=en','-encoding','UTF-8','-cp',cp,'-d',work/'classes',source,ROOT/'tools/formal-tc-lock-coverage/LockCoverageOracle.java'],'compile.log')
    compiled=[]
    for path in (work/'classes').rglob('*.class'):
        snapshot='classes/'+path.relative_to(work/'classes').as_posix();put(snapshot,path.read_bytes());compiled.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)));binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/snapshot),role='compiled_TC_source_oracle')
    binding.roots.append((work/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest());save('compiled-classes.json',compiled)
    values=cases();(work/'cases.json').write_text(json.dumps(values),encoding='utf-8');save('cases.json',values)
    loaded=work/'class-load.log'
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-Duser.language=en','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),
        '-Dlogback.configurationFile='+str(Path(read(BUILD/'build.json')['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loaded.as_posix(),'-cp',str(work/'classes')+os.pathsep+cp,
        'LockCoverageOracle',work/'cases.json',work/'observations.json'],'run.log')
    put('class-load.log',loaded.read_bytes());observed=read(work/'observations.json');need(len(observed)==15,'Missing original source observations');put('observations.json',(work/'observations.json').read_bytes())
    origins=binding.loaded(loaded,None,False,'tc_lock')
    name='org.apache.seata.server.lock.AbstractLockManager';need(Path(origins['bound_origins'][name]['actual_file'])==work/'classes'/(name.replace('.','/')+'.class'),'Original TC source was shadowed');save('origin-binding.json',origins);binding.verify()
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'TC source input drift')
    save('result.json',dict(status='ORIGINAL_TC_SOURCE_OBSERVATIONS_VERIFIED',source_scenarios=15,recording_backend=True,
        original_selected_sources_compiled=1,all_file_class_origins_bound=True,runtime_files=len(binding.files),frozen_server_fallback_files=sum('server/target/classes' in row['path'].replace('\\','/') for row in frozen),
        full_original_Maven_environment_recreated=False,actual_network_TC_requests=0,real_backend_lock_storage_verified=False,
        TC_safety_gate_passed=False,TC_registration_gate_passed=False,SQL_Undo_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False,external_tc_risk_accepted=False))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)

if __name__=='__main__':run()
