"""Fresh actual FILE records and full pinned TC restarts with the isolated three-class candidate."""
import datetime,hashlib,importlib.util,json,os,subprocess,uuid,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
def quoted(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def has_reload_exception(raw):
 return b'org.apache.seata.common.exception.ShouldNeverHappenException:' in raw or b'NOT properly handled CommitRetryTimeout' in raw or b'NOT properly handled RollbackRetryTimeout' in raw
def dependency_inventory(classpath):
 rows=[]
 for entry in classpath.split(os.pathsep):
  if not entry:raise RuntimeError('Compile dependencies must be explicit; empty classpath entry rejected')
  path=Path(entry).resolve()
  if path.drive.upper() not in ['D:','E:']:raise RuntimeError('Compile dependency outside owned D/E cohort')
  if path.is_file():rows.append(dict(path=str(path),kind='file',sha256=sha(path.read_bytes())))
  elif path.is_dir():rows.append(dict(path=str(path),kind='directory',files=[dict(path=p.relative_to(path).as_posix(),sha256=sha(p.read_bytes())) for p in sorted(path.rglob('*')) if p.is_file()]))
  else:raise RuntimeError('Missing actual compile dependency: '+str(path))
 return rows

def run():
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-0/evidence/local-java-retry-timeout-candidate'/('run_'+stamp);out.mkdir(parents=True);runtime=Path('D:/codex-builds')/('jrf_'+stamp);runtime.mkdir()
 java_parent=ROOT/'docs/phase-0/evidence/local-java-preface-candidate/run_20261112_221159_a16eafb1'
 build=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3');patchdir=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7';manifest=json.loads((patchdir/'retry-timeout-manifest.json').read_bytes());patch=patchdir/manifest['patch']
 if sha(patch.read_bytes())!=manifest['patch_sha256']:raise RuntimeError('Retention patch differs')
 frozen={p:p.read_bytes() for p in HERE.iterdir() if p.is_file() and p.suffix in ['.java','.py']};plan=ROOT/'docs/phase-0/java-retry-timeout-retention-candidate-plan.md';frozen[plan]=plan.read_bytes()
 helper=ROOT/'tools/java-v1-golden/FileRetryTimeoutStoreProbe.java'
 for path in [patch,patchdir/'retry-timeout-manifest.json',helper,ROOT/'tools/java-tc-integration/backend_recovery/run.py',ROOT/'tools/go-java-v1-real-at/overlay.py',java_parent.parent/'manifest.json',java_parent/'overlay-manifest.json']+list((java_parent/'overlay').rglob('*.class')):frozen[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(frozen.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';put(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 save(out/'source-input-audit.json',rows)
 audit_path=Path('D:/codex-builds/retry-retention-red_20261113_010005_fe3b7eb3/production-audit.json');report=build/'test-suite/test-new-version/target/surefire-reports/TEST-org.apache.seata.server.session.RetryTimeoutRetentionCandidateTest.xml';external={audit_path:audit_path.read_bytes(),report:report.read_bytes()};external_rows=[]
 for path,data in external.items():
  name='external-inputs/'+sha(str(path).encode())[:20]+'.bin';put(out/name,data);external_rows.append(dict(path=str(path),snapshot=name,sha256=sha(data)))
 save(out/'external-input-audit.json',external_rows);suite=ET.fromstring(external[report]);raw_classpath=next(p.get('value') for p in suite.findall('properties/property') if p.get('name')=='surefire.test.class.path');classpath=os.pathsep.join(entry for entry in raw_classpath.split(os.pathsep) if entry);put(out/'effective-classpath.txt',classpath+'\n');dependencies=dependency_inventory(classpath);save(out/'compile-dependency-audit.json',dependencies)
 h=module('retention_file_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py');observer=module('retention_native_observer',HERE/'native_observer.py');overlay_audit=module('retention_preface_audit',ROOT/'tools/go-java-v1-real-at/overlay.py');overlay_audit.verify(java_parent)
 jar=ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR changed')
 original=json.loads(external[audit_path]);expected={row['path']:row['patched_lf_sha256'] for row in manifest['files']};audit=[]
 for row in original:
  path=build/row['path'];actual=sha(path.read_bytes().replace(b'\r\n',b'\n'))
  if actual!=expected.get(row['path'],row['lf_sha256']):raise RuntimeError('Unexpected Java production drift: '+row['path'])
  audit.append(dict(path=row['path'],original_lf_sha256=row['lf_sha256'],actual_lf_sha256=actual,patched=row['path'] in expected))
 if len(audit)!=2122:raise RuntimeError('Incomplete original production closure')
 actual_paths={p.relative_to(build).as_posix() for p in build.rglob('*') if p.is_file() and 'target' not in p.relative_to(build).parts and ('/src/main/' in '/'+p.relative_to(build).as_posix() or p.name=='pom.xml')}
 if actual_paths!={row['path'] for row in original}:raise RuntimeError('Unexpected Java main/resource/POM path')
 save(out/'production-source-audit.json',audit)
 put(out/'compiler-version.txt',h.command([h.JAVA.parent/'javac.exe','-version']).stdout)
 compiled=out/'compiled';compiled.mkdir();sources=[HERE/Path(row['path']).name for row in manifest['files']]+[helper,HERE/'FileRetentionSeedBootstrap.java']
 for source,row in zip(sources,manifest['files']):
  if sha(source.read_bytes().replace(b'\r\n',b'\n'))!=row['patched_lf_sha256']:raise RuntimeError('Candidate template does not match patch')
 args=runtime/'compile.args';put(args,'\n'.join(quoted(x) for x in ['-encoding','UTF-8','-cp',classpath,'-d',compiled]+sources)+'\n')
 result=h.command([h.JAVA.parent/'javac.exe','@'+str(args)],timeout=90,check=False);put(out/'compile.log',result.stdout)
 if result.returncode:raise RuntimeError('Runtime candidate compilation failed')
 overlay=out/'overlay';inventory=[]
 for source in sorted((java_parent/'overlay').rglob('*.class')):
  target=overlay/source.relative_to(java_parent/'overlay');put(target,source.read_bytes());inventory.append(dict(path=target.relative_to(overlay).as_posix(),sha256=h.sha(target),origin='certified-preface'))
 for row in manifest['files']:
  relative=Path(row['path']).relative_to('server/src/main/java');base=compiled/relative.with_suffix('');files=[base.with_suffix('.class')]+list(base.parent.glob(base.name+'$*.class'))
  for source in files:
   target=overlay/source.relative_to(compiled);put(target,source.read_bytes());inventory.append(dict(path=target.relative_to(overlay).as_posix(),sha256=h.sha(target),origin=row['path']))
 for leaf in ['FileRetryTimeoutStoreProbe','FileRetentionSeedBootstrap']:
  fixture=runtime/'fixture/org/apache/seata/server/session'/(leaf+'.class');put(fixture,(compiled/'org/apache/seata/server/session'/(leaf+'.class')).read_bytes())
 save(out/'overlay-manifest.json',inventory)
 print('RUNTIME_OVERLAY_READY '+str(out),flush=True)
 cases=[];tc=None;success=False
 try:
  for name,status,timeout_unlock,failed_unlock in [('begin',1,False,False),('commit16',16,False,False),('rollback17_locked',17,False,False),('rollback17_timeout_unlock',17,True,False),('rollback17_failed_unlock',17,False,True),('rollback17_both_unlock',17,True,True)]:
   if h.port_open(28091):raise RuntimeError('Owned runtime port already occupied')
   directory=out/name;directory.mkdir();work=runtime/name;work.mkdir();store=work/'store/28091';xid='127.0.0.1:28091:'+str(900000+status)
   seed_config=work/'fixture.conf';save(seed_config,dict(store=dict(mode='file',session=dict(mode='file'),lock=dict(mode='file'),file=dict(dir=store.as_posix(),flushDiskMode='sync'))));registry=work/'registry.conf';save(registry,dict(config=dict(type='file',file=dict(name='file:'+seed_config.as_posix()))))
   seed_args=work/'seed.args';put(seed_args,'\n'.join(quoted(x) for x in ['-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dfile.listener.enabled=false','-Dseata.config.name=file:'+registry.as_posix(),'-Dseata.it.fixture.config='+seed_config.as_posix(),'-Dloader.path='+(runtime/'fixture').as_posix(),'-Dloader.main=org.apache.seata.server.session.FileRetentionSeedBootstrap','-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','seed',store,status])+'\n');result=h.command([h.JAVA,'@'+str(seed_args)],timeout=30,check=False);put(directory/'seed.log',result.stdout)
   if result.returncode or ('SEEDED status='+str(status)+' branches=1 xid='+xid).encode() not in result.stdout:raise RuntimeError('Original writer did not persist exact target')
   put(directory/'original-root.data',(store/'root.data').read_bytes())
   config=f'''server:
  port: 28091
spring:
  main:
    web-application-type: none
logging:
  file:
    path: {work.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: 28091
    enable-check-auth: false
    rollback-retry-timeout-unlock-enable: {str(timeout_unlock).lower()}
    rollback-failed-unlock-enable: {str(failed_unlock).lower()}
  store:
    mode: file
    session:
      mode: file
    lock:
      mode: file
    file:
      dir: {(work/'store').as_posix()}
      flush-disk-mode: sync
'''
   put(work/'application.yml',config);put(directory/'application.yml',config);observations=[];pids=[]
   for restart in [1,2]:
    stage=directory/('restart_'+str(restart));stage.mkdir()
    for row in inventory:
     if h.sha(overlay/row['path'])!=row['sha256']:raise RuntimeError('Candidate runtime class changed')
    env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=work.as_posix();tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),'-verbose:class','-Dloader.path='+overlay.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],stage,'tc',work,env);pids.append(tc.pid);h.wait_until(lambda:h.port_open(28091),60,'Actual candidate FILE TC startup',tc)
    lockable=status==16 or (status==17 and (timeout_unlock or failed_unlock));native=observer.observe(xid,'mysql://timeout-file','orders:1',status,lockable,stage/'native-frames.json');save(stage/'native-query.json',native)
    console=h.query(xid,stage,'console',1,status,1);branch=console['data'][0]['branchSessionVOs'][0]
    if str(branch['branchId'])!=str(910000+status) or branch['resourceId']!='mysql://timeout-file':raise RuntimeError('Actual restored branch identity differs')
    h.stop(tc);tc=None;raw=(stage/'tc-stdout.log').read_bytes()
    if has_reload_exception(raw):raise RuntimeError('Candidate full startup retained a reload exception')
    origins=[]
    for row in manifest['files']:
     name_class='.'.join(Path(row['path']).relative_to('server/src/main/java').with_suffix('').parts);matches=[line for line in raw.splitlines() if (name_class+' source: ').encode() in line]
     if len(matches)!=1 or ('file:/'+overlay.as_posix()+'/').encode() not in matches[0]:raise RuntimeError('Actual candidate class source differs: '+name_class)
     origins.append(dict(class_name=name_class,line=matches[0].decode('utf-8')))
    save(stage/'class-origins.json',origins);put(stage/'restored-root.data',(store/'root.data').read_bytes());observations.append(dict(pid=pids[-1],status=status,lockable=lockable,branch_id=910000+status))
    h.wait_until(lambda:not h.port_open(28091),10,'Owned TC stopped')
   if len(set(pids))!=2:raise RuntimeError('Actual fresh TC process was not demonstrated')
   cases.append(dict(case=name,status=status,timeout_unlock=timeout_unlock,failed_unlock=failed_unlock,xid=xid,observations=observations));print('PASS_FILE_RETENTION '+name,flush=True)
  for path,data in frozen.items():
   if path.read_bytes()!=data:raise RuntimeError('Frozen runtime input changed: '+str(path))
  for path,data in external.items():
   if path.read_bytes()!=data:raise RuntimeError('External compile metadata changed during execution: '+str(path))
  if dependency_inventory(classpath)!=dependencies:raise RuntimeError('Actual compile dependency cohort changed during execution')
  for row in inventory:
   if h.sha(overlay/row['path'])!=row['sha256']:raise RuntimeError('Runtime class changed after execution')
  if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR changed after runtime')
  success=True
 finally:
  h.stop(tc);h.wait_until(lambda:not h.port_open(28091),10,'Owned candidate TC resources stopped');save(out/'result.json',dict(succeeded=success,cases=cases,runtime=str(runtime),ports_released=True,original_jar_sha256=h.sha(jar),patch_sha256=h.sha(patch)))
 if success:
  save(out/'summary.json',dict(succeeded=True,cases=len(cases),tc_startups=12,frozen_inputs=len(frozen),external_metadata_inputs=len(external),compile_classpath_entries=len(dependencies),compile_classpath_files=sum(1 if item['kind']=='file' else len(item['files']) for item in dependencies),overlay_classes=len(inventory),patch_sha256=h.sha(patch),original_jar_sha256=h.sha(jar),scope='Seeded real FILE and full TC candidate restarts for Begin/16/17, default and both unlock policies. No operator callback/business compensation, actual RAFT election, or complete WP00/Rust certification.'));print(json.dumps(dict(succeeded=True,evidence_directory=str(out),cases=6,tc_startups=12)),flush=True)
 return out

if __name__=='__main__':run()
