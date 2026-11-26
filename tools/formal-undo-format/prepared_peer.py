"""Within an owned live fixture compare real Rust prepared logs against original JDBC/parser output."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
from runtime_binding import RuntimeBinding

ROOT=Path(__file__).resolve().parents[2]
JAVA=Path(os.environ.get('SEATA_JAVA_ORACLE_ROOT',ROOT.parent/'seata-java-phase09-oracle'))
BUILD=Path(os.environ.get('SEATA_UNDO_ORACLE_BUILD',ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'))


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)


def extra_inputs():
    paths={Path(__file__),Path(__file__).with_name('runtime_binding.py'),Path(__file__).with_name('UndoPreparedOracle.java'),
        Path(__file__).with_name('UndoDocumentPeer.java'),ROOT/'docs/phase-2/undo-codec-document-contract.md',BUILD/'build.json',BUILD/'catalog.json'}
    paths.update((ROOT/'tests/fixtures/undo/business_sql').glob('*.sql'))
    source_dir=Path('D:/codex-builds/seata-connector8-source-20261115')
    paths.update(p for p in source_dir.iterdir() if p.is_file())
    return paths


def run_prepared_peer(out,runtime,environment):
    destination=out/'prepared-peer';destination.mkdir()
    work=runtime/'prepared-peer';(work/'classes').mkdir(parents=True)
    env=environment.copy()
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    def put(name,data):
        path=destination/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        value=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=90)
        put(name,value.stdout);need(value.returncode==0,name+' failed');return value.stdout
    for row in read(BUILD/'catalog.json')['members']:need(sha(BUILD/row['path'])==row['sha256'],'Original Java build archive drift')
    build=read(BUILD/'build.json');binding=RuntimeBinding(BUILD,JAVA)
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-cp',build['classpath'],'-d',work/'classes',
        Path(__file__).with_name('UndoPreparedOracle.java'),Path(__file__).with_name('UndoDocumentPeer.java')],'java-build.log')
    classes=[]
    for path in (work/'classes').rglob('*.class'):
        snapshot='classes/'+path.relative_to(work/'classes').as_posix();put(snapshot,path.read_bytes())
        binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(destination/snapshot),role='peer_compiled_class')
        classes.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    binding.roots.append((work/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest());save('peer-classes.json',classes)
    def java(main,args,name,driver):
        binding.verify();loaded=work/(name+'.loaded.log')
        cp=str(work/'classes')+os.pathsep+build['classpath']+(os.pathsep+str(binding.drivers[driver]) if driver else '')
        command(['D:/Java/jdk-17.0.12/bin/java.exe','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),
            '-Dlogback.configurationFile='+str(Path(build['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loaded.as_posix(),
            '-cp',cp,main]+args,name+'.stdout.log')
        put(name+'.loaded.log',loaded.read_bytes());save(name+'.origin-binding.json',binding.loaded(loaded,driver,True));binding.verify()
    pairs=[]
    for profile in ['default','numeric_tinyint']:
        reference=work/'references';java('UndoPreparedOracle',[ROOT/'tests/fixtures/undo/business_sql',reference,profile],profile,'8.0.27')
        data=reference/(profile+'.json');put('reference/'+data.name,data.read_bytes())
        candidate=Path(env['SEATA_MYSQL_CODEC_OUT'])/(profile+'.json');need(candidate.is_file(),'Actual Rust prepared output missing');put('rust/'+candidate.name,candidate.read_bytes())
        pairs.append(dict(case=profile,reference=str(data),candidate=str(candidate),reference_sha256=sha(data),candidate_sha256=sha(candidate)))
    manifest=work/'pairs.json';manifest.write_text(json.dumps(pairs,indent=2),encoding='utf-8');save('pairs.json',pairs)
    java('UndoDocumentPeer',['compare',work/'references',work/'comparison',manifest],'actual-JDBC-Rust-comparison',None)
    results=read(work/'comparison/comparison.json');need(len(results)==2 and all(x['status']=='MATCH' for x in results),'Actual JDBC/prepared differential missing')
    put('comparison.json',(work/'comparison/comparison.json').read_bytes())
    binding.verify();save('result.json',dict(actual_Rust_local_DML_prepare=True,original_JDBC_prepare=True,
        selected_profiles=2,logs_per_profile=4,Java_runtime_type_value_container_MATCH=2,
        source_case_parity_complete=False,TC_registration_verified=False,SQL_Undo_atomicity_verified=False,public_COMMIT_available=False,
        original_build=str(BUILD),original_build_catalog_sha256=sha(BUILD/'catalog.json')))
