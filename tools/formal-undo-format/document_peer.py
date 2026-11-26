"""Compare trusted Java-generated corpus through the real Rust codec and original Java parser."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid
from runtime_binding import RuntimeBinding

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)


def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/undo-document-peer'/('run_'+stamp)
    runtime=Path('D:/codex-builds')/('seata_undo_document_peer_'+stamp)
    out.mkdir(parents=True);runtime.mkdir();(runtime/'classes').mkdir();(runtime/'inputs').mkdir()
    print('UNDO_DOCUMENT_PEER_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1')
    for name in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS']:env.pop(name,None)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        result=subprocess.run([str(x) for x in args],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(name,result.stdout);need(result.returncode==0,name+' failed');return result.stdout
    for x in read(BUILD/'catalog.json')['members']:need(sha(BUILD/x['path'])==x['sha256'],'Original Java build archive differs')
    binding=RuntimeBinding(BUILD,JAVA);build=read(BUILD/'build.json')
    pairs=[]
    for profile in ['default','numeric_tinyint']:
        for source in sorted((ROOT/'tests/fixtures/undo/java_8_0_27'/profile).glob('undo_*.json')):
            name=profile+'-'+source.name;target=runtime/'inputs'/name;target.write_bytes(source.read_bytes());put('inputs/'+name,source.read_bytes())
            pairs.append(dict(case=name,reference=str(target),candidate=str(runtime/'rust'/name),source=str(source),source_sha256=sha(source)))
    synthetic={
        'default.json':b'{}','null.json':b'null',
        'nullable.json':b'{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":null,"branchId":-9223372036854775808,"sqlUndoLogs":["java.util.ArrayList",[null,{"@class":"org.apache.seata.rm.datasource.undo.SQLUndoLog","sqlType":null,"tableName":null,"beforeImage":null,"afterImage":null}]]}',
        'linked.json':b'{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","sqlUndoLogs":["java.util.LinkedList",[]]}',
    }
    for name,data in synthetic.items():
        target=runtime/'inputs'/name;target.write_bytes(data);put('inputs/'+name,data)
        pairs.append(dict(case=name,reference=str(target),candidate=str(runtime/'rust'/name),kind='Rust_obligation_original_Java_acceptance_probe'))
    java_extra=[JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource/sql/serial/SerialArray.java']
    source_tests=[JAVA/'rm-datasource/src/test/java/org/apache/seata/rm/datasource/undo'/name for name in
        ['BranchUndoLogTest.java','BaseUndoLogParserTest.java','parser/JacksonUndoLogParserTest.java']]
    for path in java_extra+source_tests:
        blob=subprocess.check_output(['git','-C',str(JAVA),'show','cba73e180fd8896a99df6415b859d275122feaa7:'+path.relative_to(JAVA).as_posix()],creationflags=subprocess.CREATE_NO_WINDOW)
        need(path.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Original probe source differs from pin')
    paths={Path(__file__),Path(__file__).with_name('runtime_binding.py'),Path(__file__).with_name('UndoDocumentPeer.java'),Path(__file__).with_name('UndoCodecSamples.java'),Path(__file__).with_name('UndoNumberBounds.java'),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/undo-codec-document-contract.md'}|set(java_extra+source_tests)
    paths.update(p for p in (ROOT/'crates').rglob('*') if p.is_file())
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin'
        put(snapshot,path.read_bytes());frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    save('source-inputs.json',frozen)
    raw=command(['cargo','build','--locked','--offline','-p','seata-at','--example','undo_codec_peer','--message-format=json'],'rust-build.jsonl')
    artifacts=[]
    for line in raw.splitlines():
        try:row=json.loads(line)
        except ValueError:continue
        if row.get('reason')=='compiler-artifact' and row['target']['name']=='undo_codec_peer' and row.get('executable'):artifacts.append(Path(row['executable']))
    need(len(artifacts)==1,'Expected actual Rust peer artifact')
    binary=artifacts[0];digest=sha(binary);put('artifacts/'+binary.name,binary.read_bytes());save('rust-artifact.json',dict(path=str(binary),sha256=digest))
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-cp',build['classpath'],'-d',runtime/'classes',
        Path(__file__).with_name('UndoDocumentPeer.java'),Path(__file__).with_name('UndoCodecSamples.java'),Path(__file__).with_name('UndoNumberBounds.java')]+java_extra,'java-build.log')
    classes=[]
    for path in (runtime/'classes').rglob('*.class'):
        snapshot='classes/'+path.relative_to(runtime/'classes').as_posix();put(snapshot,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/snapshot),role='peer_compiled_class');classes.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    binding.roots.append((runtime/'classes').resolve());binding.verify()
    save('runtime-inputs.json',binding.manifest());save('peer-classes.json',classes)
    cp=str(runtime/'classes')+os.pathsep+build['classpath']
    def java(mode,name,entry='UndoDocumentPeer'):
        loading=runtime/(name+'.loaded.log');binding.verify()
        args=['D:/Java/jdk-17.0.12/bin/java.exe','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),
            '-Duser.timezone=UTC','-Dlogback.configurationFile='+str(Path(build['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loading.as_posix(),'-cp',cp,entry]
        command(args+mode,name+'.stdout.log');put(name+'.loaded.log',loading.read_bytes())
        origins=binding.loaded(loading,None,True)
        if entry=='UndoCodecSamples':
            need(Path(origins['bound_origins']['org.apache.seata.rm.datasource.sql.serial.SerialArray']['actual_file'])==
                runtime/'classes/org/apache/seata/rm/datasource/sql/serial/SerialArray.class','Pinned SerialArray source was not the loaded class')
        save(name+'.origin-binding.json',origins);binding.verify()
    java([runtime/'inputs'],'java-source-samples','UndoCodecSamples')
    java([runtime/'inputs'],'java-number-bounds','UndoNumberBounds')
    number_report=read(runtime/'inputs/number-bounds.report')
    expected_accepted={'integer-1000','decimal-integer-1000','negative-1000','float-999','float-1000','negative-float-1000','exponent-1000'}
    need(len(number_report)==14 and {x['case'] for x in number_report if x['original_accepted']}==expected_accepted,'Original numeric constraint boundary differs')
    number_cases=sorted((runtime/'inputs').glob('number-*.json'));need(len(number_cases)==7,'Expected seven Java-accepted numeric boundaries')
    generated=sorted((runtime/'inputs').glob('probe-*.json'))
    need(len(generated)==17,'Expected 13 source-value/object probes and four Java scale boundaries')
    for path in generated:
        put('inputs/'+path.name,path.read_bytes());pairs.append(dict(case=path.name,reference=str(path),candidate=str(runtime/'rust'/path.name),kind='source_value_probe_NOT_original_JUnit_execution'))
    for path in number_cases:
        put('inputs/'+path.name,path.read_bytes());pairs.append(dict(case=path.name,reference=str(path),candidate=str(runtime/'rust'/path.name),kind='original_Jackson_number_constraint_probe'))
    for path in (runtime/'inputs').glob('*.report'):put('inputs/'+path.name,path.read_bytes())
    count=len(pairs)
    (runtime/'pairs.json').write_text(json.dumps(pairs,indent=2),encoding='utf-8');save('pairs.json',pairs)
    command([binary,runtime/'inputs',runtime/'rust'],'rust-codec.log')
    for path in (runtime/'rust').iterdir():put('rust/'+path.name,path.read_bytes())
    java(['compare',runtime/'inputs',runtime/'java',runtime/'pairs.json'],'java-compare')
    matches=read(runtime/'java/comparison.json');need(len(matches)==count and all(x['status']=='MATCH' for x in matches),'Missing Java runtime differential')
    put('comparison.json',(runtime/'java/comparison.json').read_bytes())
    java(['rewrite',runtime/'rust',runtime/'java-reencoded'],'java-reencode')
    for path in (runtime/'java-reencoded').iterdir():put('java-reencoded/'+path.name,path.read_bytes())
    # Java重新编码后的对象再由Rust解析，不能只做单方向Java读取。
    command([binary,runtime/'java-reencoded',runtime/'rust-second'],'rust-second.log')
    for path in (runtime/'rust-second').iterdir():put('rust-second/'+path.name,path.read_bytes())
    second=[row|dict(candidate=str(runtime/'rust-second'/Path(row['candidate']).name)) for row in pairs]
    (runtime/'second-pairs.json').write_text(json.dumps(second,indent=2),encoding='utf-8');save('second-pairs.json',second)
    java(['compare',runtime/'inputs',runtime/'java-second',runtime/'second-pairs.json'],'java-second-compare')
    second_matches=read(runtime/'java-second/comparison.json');need(len(second_matches)==count and all(x['status']=='MATCH' for x in second_matches),'Missing second Java runtime differential')
    put('second-comparison.json',(runtime/'java-second/comparison.json').read_bytes())
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Peer source drift')
    need(sha(binary)==digest,'Rust executed artifact drift');binding.verify()
    save('result.json',dict(status='selected_document_runtime_differential_passed',Java_generated_native_cases=12,
        Rust_obligation_original_Java_probe_cases=4,source_value_and_scale_probes=17,original_number_constraint_probes=14,
        original_accepted_numeric_documents=7,original_JUnit_tests_executed=False,
        Java_runtime_MATCH=count,Java_reencode_Rust_parse_cases=count,second_Java_runtime_MATCH=count,
        true_prepared_business_logs_verified=False,source_case_parity_complete=False,SQL_Undo_atomicity_verified=False,WP05_complete=False,
        original_build=str(BUILD),original_build_catalog_sha256=sha(BUILD/'catalog.json')))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),members=len(members))),flush=True)


if __name__=='__main__':run()
