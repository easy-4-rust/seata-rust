"""Compile pinned original source tests in an isolated D-drive Maven reactor."""
import datetime,hashlib,json,os,subprocess,uuid,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
ORACLE=ROOT.parent/'seata-java-phase09-oracle'
PIN='cba73e180fd8896a99df6415b859d275122feaa7'
JDK=Path('D:/Java/jdk-17.0.12')
MVN=Path('D:/Java/maven/apache-maven-3.9.16/bin/mvn.cmd')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def need(v,why):
    if not v:raise ValueError(why)
def run(evidence_root=None):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_source_junit_'+stamp);work.mkdir()
    out=(evidence_root or ROOT/'docs/phase-2/evidence/source-junit')/('run_'+stamp);out.mkdir(parents=True)
    print('SOURCE_JUNIT_BUILD '+str(out),flush=True)
    (out/'build-executed.py').write_bytes(Path(__file__).read_bytes())
    env=os.environ|dict(MAVEN_SKIP_RC='1',JAVA_HOME=str(JDK),TEMP='D:/Temp',TMP='D:/Temp',PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1',MAVEN_OPTS='-Xmx1024m -XX:MaxMetaspaceSize=512m -Duser.home='+str(work/'home')+' -Djava.io.tmpdir=D:/Temp',PATH=str(JDK/'bin')+os.pathsep+os.environ['PATH'])
    for key in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','MAVEN_ARGS']:env.pop(key,None)
    def command(args,leaf,cwd=ROOT,timeout=1800):
        with (out/leaf).open('wb') as f:
            v=subprocess.run(list(map(str,args)),cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=timeout)
        return v.returncode
    need(subprocess.check_output(['git','-C',str(ORACLE),'rev-parse','HEAD'],env=env).decode().strip()==PIN,'Original pin differs')
    source=work/'source';source.mkdir();archive=work/'source.zip'
    tree=subprocess.check_output(['git','-C',str(ORACLE),'ls-tree','-rz',PIN],env=env,creationflags=subprocess.CREATE_NO_WINDOW)
    objects=[]
    for entry in tree.split(b'\0'):
        if not entry:continue
        header,name=entry.split(b'\t',1);mode,kind,digest=header.decode('ascii').split();need(kind=='blob' and mode in ['100644','100755'],'Unsupported Git source object mode');objects.append((name.decode('utf-8'),digest,mode))
    batch=subprocess.run(['git','-C',str(ORACLE),'cat-file','--batch'],input=''.join(digest+'\n' for _,digest,_ in objects).encode('ascii'),env=env,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,check=True).stdout
    offset=0;git_inputs=[]
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED) as z:
        for name,digest,mode in objects:
            end=batch.index(b'\n',offset);header=batch[offset:end].decode('ascii').split();size=int(header[2]);need(header[:2]==[digest,'blob'],'Git batch header differs');data=batch[end+1:end+1+size];need(hashlib.sha1(('blob '+str(size)+'\0').encode('ascii')+data).hexdigest()==digest,'Git blob bytes differ');offset=end+size+2
            p=source/name;need(p.resolve().is_relative_to(source.resolve()),'Literal Git path escaped stage');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);z.writestr(name,data);git_inputs.append(dict(path=name,git_blob=digest,git_mode=mode,sha256=hashlib.sha256(data).hexdigest()))
    need(offset==len(batch),'Git batch trailing bytes')
    (out/'literal-git-inputs.json').write_text(json.dumps(git_inputs,indent=2)+'\n',encoding='utf-8')
    m2=Path('D:/codex-builds/seata-source-junit-m2');m2.mkdir(exist_ok=True)
    settings=work/'settings.xml';settings.write_text('''<settings xmlns="http://maven.apache.org/SETTINGS/1.2.0"><localRepository>D:/codex-builds/seata-source-junit-m2</localRepository><profiles><profile><id>readonly-cached-artifacts</id><repositories><repository><id>readonly-user-cache</id><url>file:///C:/Users/hnxyh/.m2/repository</url><releases><enabled>true</enabled><updatePolicy>never</updatePolicy></releases><snapshots><enabled>true</enabled><updatePolicy>never</updatePolicy></snapshots></repository></repositories><pluginRepositories><pluginRepository><id>readonly-user-cache-plugins</id><url>file:///C:/Users/hnxyh/.m2/repository</url><releases><enabled>true</enabled><updatePolicy>never</updatePolicy></releases><snapshots><enabled>true</enabled><updatePolicy>never</updatePolicy></snapshots></pluginRepository></pluginRepositories></profile></profiles><activeProfiles><activeProfile>readonly-cached-artifacts</activeProfile></activeProfiles></settings>''',encoding='utf-8')
    global_settings=work/'global-settings.xml';global_settings.write_text('<settings xmlns="http://maven.apache.org/SETTINGS/1.2.0"/>',encoding='utf-8')
    command([MVN,'-v'],'maven-version.log',source)
    command([JDK/'bin/java.exe','-version'],'java-version.log',source)
    args=[MVN,'-B','-ntp','-gs',global_settings,'-s',settings,'-Dmaven.repo.local='+str(m2),'-pl','rm-datasource,sqlparser/seata-sqlparser-core','-am','-Dspotless.apply.skip=true','test-compile','org.apache.maven.plugins:maven-dependency-plugin:3.0.2:build-classpath','-Dmdep.includeScope=test','-Dmdep.outputFile=target/declared-test-classpath.txt']
    (out/'build-command.json').write_text(json.dumps(dict(argv=list(map(str,args)),cwd=str(source),source_pin=PIN,archive_sha256=sha(archive),literal_Git_blobs_verified=True,formatter_apply_disabled_to_preserve_source_bytes=True,work=str(work),C_cache_read_only=True,D_repo=str(m2)),indent=2)+'\n',encoding='utf-8')
    inputs=[]
    for p in sorted(source.rglob('*')):
        if p.is_file():inputs.append(dict(path=p.relative_to(source).as_posix(),sha256=sha(p)))
    (out/'original-source-inputs.json').write_text(json.dumps(inputs,indent=2)+'\n',encoding='utf-8')
    code=command(args,'maven-test-compile.log',source)
    for row in inputs:need(sha(source/row['path'])==row['sha256'],'Original POM or source changed during compile: '+row['path'])
    (out/'post-compile-source-integrity.json').write_text(json.dumps(dict(verified_original_files=len(inputs),all_unchanged=True,files=inputs),indent=2)+'\n',encoding='utf-8')
    result=dict(status='SOURCE_TEST_COMPILE_PASSED' if code==0 else 'SOURCE_TEST_COMPILE_FAILED',exit_code=code,work=str(work),java_pin=PIN,source_files=len(inputs),JUnit_executed=False,Rust_parity_complete=False,WP05_complete=False,full_V1_complete=False)
    (out/'compile-result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result),flush=True)
    if code:raise SystemExit(code)
    return out
if __name__=='__main__':run()
