"""Record real pinned JUnit discovery and execution without rewriting source tests."""
import argparse,datetime,hashlib,json,os,socket,subprocess,xml.etree.ElementTree as ET,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
JDK=Path('D:/Java/jdk-17.0.12/bin')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(v,why):
    if not v:raise ValueError(why)
def run(build,mode,evidence_root=None):
    result=read(build/'compile-result.json');need(result['status']=='SOURCE_TEST_COMPILE_PASSED','Original source tests did not compile')
    stage=Path(result['work']);source=stage/'source';need(stage.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Stage escaped D workspace')
    for row in read(build/'original-source-inputs.json'):need(sha(source/row['path'])==row['sha256'],'Original compiled input drift: '+row['path'])
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];work=stage/('junit_'+stamp);work.mkdir();(work/'classes').mkdir();(work/'home').mkdir()
    family='source-junit-literal' if read(build/'build-command.json').get('literal_Git_blobs_verified') else 'source-junit'
    out=(evidence_root or ROOT/('docs/phase-2/evidence/'+family))/('runtime_'+stamp);out.mkdir(parents=True);print('SOURCE_JUNIT_RUNTIME '+str(out),flush=True)
    (out/'run-executed.py').write_bytes(Path(__file__).read_bytes())
    def save(name,v):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    hosts=work/'hosts';hosts.write_text('127.0.0.1 localhost '+socket.gethostname()+'\n127.0.0.2 example.com\n',encoding='ascii')
    env=os.environ|dict(JAVA_HOME=str(JDK.parent),TEMP='D:/Temp',TMP='D:/Temp',JAVA_TOOL_OPTIONS='-Duser.home='+str(work/'home')+' -Djava.io.tmpdir=D:/Temp -Djdk.net.hosts.file='+str(hosts),MAVEN_SKIP_RC='1')
    for key in ['JDK_JAVA_OPTIONS','_JAVA_OPTIONS','MAVEN_ARGS']:env.pop(key,None)
    modules=['sqlparser/seata-sqlparser-core','rm-datasource']
    classpaths={m:[str(work/'classes'),str(source/m/'target/test-classes'),str(source/m/'target/classes')]+(source/m/'target/declared-test-classpath.txt').read_text('utf-8').strip().split(os.pathsep) for m in modules}
    inputs=[];seen=set()
    for m,paths in classpaths.items():
        for value in paths:
            p=Path(value);need(p.resolve().is_relative_to(Path('D:/codex-builds').resolve()) and p.exists(),'Classpath escaped or absent')
            for f in sorted(p.rglob('*')) if p.is_dir() else [p]:
                if f.is_file() and f not in seen:inputs.append(dict(path=str(f),sha256=sha(f)));seen.add(f)
    save('preexisting-runtime-inputs.json',inputs)
    helpers=[HERE/'SourceJUnitRunner.java',HERE/'HarnessGuard.java',HERE/'SourceJvmEnvironmentProbe.java']
    for p in helpers:(out/p.name).write_bytes(p.read_bytes())
    def invoke(args,leaf,cwd,timeout=600):
        with (out/leaf).open('wb') as stream:r=subprocess.run(list(map(str,args)),cwd=cwd,env=env,stdout=stream,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=timeout)
        return r.returncode
    cp=os.pathsep.join(classpaths[modules[0]])
    need(invoke([JDK/'javac.exe','-encoding','UTF-8','-cp',cp,'-d',work/'classes']+helpers,'helper-compile.log',work)==0,'JUnit observation helper compilation failed')
    manifest=work/'agent.mf';manifest.write_text('Manifest-Version: 1.0\nPremain-Class: SourceJvmEnvironmentProbe\nCan-Redefine-Classes: false\nCan-Retransform-Classes: false\n\n',encoding='ascii')
    agent=work/'environment-probe.jar'
    need(invoke([JDK/'jar.exe','cfm',agent,manifest,'-C',work/'classes','SourceJvmEnvironmentProbe.class'],'agent-build.log',work)==0,'JVM observation agent creation failed')
    (out/'environment-probe.jar').write_bytes(agent.read_bytes());(out/'agent.mf').write_bytes(manifest.read_bytes());save('agent-binding.json',dict(path=str(agent),sha256=sha(agent),class_transformation=False))
    base_java_options=env['JAVA_TOOL_OPTIONS']
    helper_artifacts=[dict(path=str(p),sha256=sha(p)) for p in sorted((work/'classes').glob('*.class'))];save('helper-artifacts.json',helper_artifacts)
    ns={'m':'http://maven.apache.org/POM/4.0.0'};pom=ET.parse(source/'build/pom.xml');profile=next(p for p in pom.findall('m:profiles/m:profile',ns) if p.findtext('m:id',namespaces=ns)=='args-for-test-by-jdk17-and-above');opens=profile.findtext('m:properties/m:maven.surefire.argLine',namespaces=ns).split()
    receipts=[]
    for module in modules:
        name=module.split('/')[-1];events=work/(name+'-events.jsonl');cwd=source/module;db=(cwd/'db_store').resolve();need(db.is_relative_to(stage.resolve()),'Source H2 deletion path escaped isolated stage')
        probe=work/(name+'-environments');probe.mkdir();env['JAVA_TOOL_OPTIONS']=base_java_options+' -javaagent:'+str(agent)+'='+str(probe)
        args=[JDK/'java.exe','-Dseata.junit.scope='+str(stage),'-Dseata.junit.work='+str(work),'-Djava.security.manager=HarnessGuard','-Xlog:class+load=info:file='+(work/(name+'-class-load.log')).as_posix()]+opens+['-cp',os.pathsep.join(classpaths[module]),'SourceJUnitRunner',source/module/'target/test-classes',events,mode]
        save(name+'-command.json',dict(argv=list(map(str,args)),cwd=str(cwd),JAVA_TOOL_OPTIONS=env['JAVA_TOOL_OPTIONS'],original_POM_JDK17_arguments=opens,original_pressure_overrides=False))
        code=invoke(args,name+'-stdout.log',cwd)
        for observation in probe.glob('*.xml'):
            dest=out/name/'environments'/observation.name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(observation.read_bytes())
        if events.exists():(out/events.name).write_bytes(events.read_bytes())
        loaded=work/(name+'-class-load.log')
        if loaded.exists():(out/loaded.name).write_bytes(loaded.read_bytes())
        guard=work/'guard-denials.jsonl'
        if guard.exists():(out/(name+'-guard-denials.jsonl')).write_bytes(guard.read_bytes())
        rows=[json.loads(line) for line in events.read_bytes().splitlines()] if events.exists() else []
        summary=rows[-1] if rows and rows[-1]['kind']=='summary' else None
        receipts.append(dict(module=module,exit_code=code,summary=summary));print(json.dumps(receipts[-1]),flush=True)
    for row in inputs+helper_artifacts:need(sha(Path(row['path']))==row['sha256'],'Runtime input drift')
    for row in read(build/'original-source-inputs.json'):need(sha(source/row['path'])==row['sha256'],'Source test or POM drift')
    (out/'hosts').write_bytes(hosts.read_bytes());save('result.json',dict(mode=mode,build=str(build),work=str(work),modules=receipts,source_pin=result['java_pin'],JUnit_executed=mode=='execute',Rust_parity_complete=False,WP05_complete=False,full_V1_complete=False))
    print(json.dumps(dict(runtime=str(out))),flush=True)
    return dict(runtime=str(out))
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--build',type=Path,required=True);parser.add_argument('--mode',choices=['discover','execute'],required=True);args=parser.parse_args();run(args.build,args.mode)
