"""Compare actual Rust eight-log lock keys to original JDBC/metadata/BaseTransactionalExecutor."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[2]
JAVA=Path(os.environ.get('SEATA_JAVA_ORACLE_ROOT',ROOT.parent/'seata-java-phase09-oracle'))
BUILD=Path(os.environ.get('SEATA_UNDO_ORACLE_BUILD',ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'))
LOCK_BUILD=Path(os.environ.get('SEATA_LOCK_ORACLE_BUILD',ROOT/'docs/phase-2/evidence/lock-key-peer/run_20261116_004741_eaf663fb'))
sys.path.insert(0,str(ROOT/'tools/formal-undo-format'))
from runtime_binding import RuntimeBinding
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def extra_inputs():
    paths={Path(__file__),ROOT/'tools/formal-lock-key/LockNativeOracle.java',ROOT/'tools/formal-lock-key/LockKeyOracle.java',
        ROOT/'tools/formal-undo-format/UndoPreparedOracle.java',ROOT/'tools/formal-undo-format/UndoDocumentPeer.java',ROOT/'tools/formal-undo-format/runtime_binding.py',
        ROOT/'docs/phase-2/global-lock-key-contract.md',ROOT/'tools/formal-lock-key/schema-extra.sql',ROOT/'tools/formal-lock-key/collision-insert.sql',LOCK_BUILD/'catalog.json',LOCK_BUILD/'runtime.json'}
    paths.update((ROOT/'tests/fixtures/undo/business_sql').glob('*.sql'))
    return paths

def run_native_peer(out,runtime,environment):
    destination=out/'lock-peer';destination.mkdir();work=runtime/'lock-peer';(work/'classes').mkdir(parents=True)
    env=environment.copy()
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','SEATA_CONFIG_NAME','SEATA_ENV']:env.pop(name,None)
    def put(name,data):
        path=destination/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        result=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=120)
        put(name,result.stdout);need(result.returncode==0,name+' failed')
    for packet in [BUILD,LOCK_BUILD]:
        for row in read(packet/'catalog.json')['members']:need(sha(packet/row['path'])==row['sha256'],'Original source/build archive drift')
    binding=RuntimeBinding(BUILD,JAVA);original=read(LOCK_BUILD/'runtime.json')
    for row in read(LOCK_BUILD/'runtime-inputs.json'):
        path=Path(row['path']).resolve()
        if path not in binding.files:
            need(sha(Path(row['archive']))==row['sha256'],'Lock build archived runtime differs')
            binding.files[path]={key:row[key] for key in ['sha256','archive','role']}
    binding.roots.append((Path(original['runtime'])/'classes').resolve());binding.verify()
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-J-Duser.language=en','-encoding','UTF-8','-cp',original['classpath'],'-d',work/'classes',
        ROOT/'tools/formal-lock-key/LockNativeOracle.java',ROOT/'tools/formal-undo-format/UndoPreparedOracle.java',ROOT/'tools/formal-undo-format/UndoDocumentPeer.java'],'compile.log')
    classes=[]
    for path in (work/'classes').rglob('*.class'):
        snapshot='classes/'+path.relative_to(work/'classes').as_posix();put(snapshot,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(destination/snapshot),role='native_lock_oracle_class');classes.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    binding.roots.append((work/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest());save('compiled-classes.json',classes)
    cp=str(work/'classes')+os.pathsep+original['classpath']+os.pathsep+str(binding.drivers['8.0.27'])
    profiles=[]
    for profile in ['default','numeric_tinyint']:
        binding.verify();loaded=work/(profile+'.loaded.log')
        command(['D:/Java/jdk-17.0.12/bin/java.exe','-Duser.language=en','-Dseata.config.name=registry','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),
            '-Dlogback.configurationFile='+str(Path(read(BUILD/'build.json')['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loaded.as_posix(),'-cp',cp,
            'LockNativeOracle',ROOT/'tests/fixtures/undo/business_sql',work/'reference',profile,ROOT/'tools/formal-lock-key/collision-insert.sql'],profile+'.stdout.log')
        put(profile+'.loaded.log',loaded.read_bytes());origins=binding.loaded(loaded,'8.0.27',True,'lock')
        need('org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser' in origins['bound_origins'],'Original native document parser missing')
        for name in ['org.apache.seata.rm.datasource.exec.BaseTransactionalExecutor','org.apache.seata.rm.datasource.ConnectionContext','org.apache.seata.common.util.ArrayUtils']:
            need(Path(origins['bound_origins'][name]['actual_file'])==Path(original['runtime'])/'classes'/(name.replace('.','/')+'.class'),'Selected original lock class was shadowed')
        save(profile+'.origin-binding.json',origins);binding.verify()
        source=work/'reference'/('locks-'+profile+'.json');candidate=Path(env['SEATA_MYSQL_LOCK_KEYS_OUT'])/(profile+'.json')
        put('reference/original-prepared-'+profile+'.json',(work/'reference'/(profile+'.json')).read_bytes())
        put('reference/original-codec-comparison-'+profile+'.json',(work/'reference'/('codec-compare-'+profile)/'comparison.json').read_bytes())
        put('reference/'+source.name,source.read_bytes());put('rust/'+candidate.name,candidate.read_bytes())
        need(read(source)==read(candidate),'Actual Java/Rust lock key mismatch')
        profiles.append(dict(profile=profile,logs=9,Java_Rust_MATCH=True,prepared_codec_logs=4,actual_prepared_codec_MATCH=True))
    save('result.json',dict(status='ACTUAL_JDBC_RUST_LOCK_KEYS_MATCH',profiles=profiles,
        lock_build=str(LOCK_BUILD),lock_build_catalog_sha256=sha(LOCK_BUILD/'catalog.json'),
        actual_TC_lock_coverage_verified=False,TC_registration_verified=False,SQL_Undo_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False))
