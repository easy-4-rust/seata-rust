"""Owned real FILE17 rollback lifecycle metrics, actual non-reset registry baselines and unchanged AT/TCC recovery."""
import argparse,datetime,hashlib,importlib.util,json,re,socket,subprocess,time,urllib.parse,urllib.request,urllib.error,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
def operator(xid,path):
 url='http://127.0.0.1:28091/api/v1/console/globalSession/changeGlobalStatus?'+urllib.parse.urlencode(dict(xid=xid));request=urllib.request.Request(url,method='PUT')
 try:
  with urllib.request.urlopen(request,timeout=15) as response:code=response.status;raw=response.read()
 except urllib.error.HTTPError as error:code=error.code;raw=error.read()
 result=dict(http_status=code,body=raw.decode('utf-8','replace'));save(path,result);return result
def console(xid,path):
 url='http://127.0.0.1:28091/api/v1/console/globalSession/query?'+urllib.parse.urlencode(dict(xid=xid,pageNum=1,pageSize=20,withBranch='true'))
 with urllib.request.urlopen(url,timeout=5) as response:
  raw=response.read();body=json.loads(raw)
  if response.status!=200 or str(body.get('code'))!='200':raise RuntimeError('Actual console query rejected')
 put(path,raw);return body

def poll_console(xid):
 url='http://127.0.0.1:28091/api/v1/console/globalSession/query?'+urllib.parse.urlencode(dict(xid=xid,pageNum=1,pageSize=20,withBranch='true'))
 with urllib.request.urlopen(url,timeout=5) as response:
  value=json.load(response)
  if response.status!=200 or str(value.get('code'))!='200':raise RuntimeError('Actual FILE console polling rejected')
  return value

def run(output_root):
 if output_root.resolve().drive.upper() not in ['D:','E:']:raise ValueError('Owned metrics evidence must stay on D/E')
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=output_root/('run_'+stamp);out.mkdir(parents=True);runtime=Path('D:/codex-builds')/('mrt_'+stamp);runtime.mkdir()
 parent=ROOT/'docs/phase-0/evidence/local-go-java-file-at-timeout/run_20261113_032718_6712fd5e';catalog=parent.parent/'manifest.json'
 if sha(catalog.read_bytes())!='4a8bb3abfcbfde159ecff41e3354b0495d12152761e1880bf7b78cb1f465fc1b':raise RuntimeError('Certified FILE rollback parent changed')
 catalog_rows=json.loads(catalog.read_bytes())['files']
 for path in [parent/'summary.json',parent/'source-input-audit.json']:
  expected=next(row['sha256'] for row in catalog_rows if row['path']==path.relative_to(catalog.parent).as_posix())
  if sha(path.read_bytes())!=expected:raise RuntimeError('Certified FILE rollback metadata changed')
 previous=json.loads((parent/'summary.json').read_bytes())
 metrics_parent=ROOT/'docs/phase-0/evidence/local-java-metrics-lifecycle-candidate/run_20261113_043739_9fbdad8c';metrics_catalog=metrics_parent.parent/'manifest.json'
 if sha(metrics_catalog.read_bytes())!='ddb4ded1ccd8740d2bc603722f1808caa6d61feed9b652470663bbd885cbe666':raise RuntimeError('Certified metrics parent catalog changed')
 metrics_catalog_rows=json.loads(metrics_catalog.read_bytes())['files']
 def authenticate_metrics(path):
  expected=next(row['sha256'] for row in metrics_catalog_rows if row['path']==path.relative_to(metrics_catalog.parent).as_posix())
  if sha(path.read_bytes())!=expected:raise RuntimeError('Certified metrics parent file changed: '+str(path))
 for path in [metrics_parent/'summary.json',metrics_parent/'source-input-audit.json',metrics_parent/'external-input-audit.json',metrics_parent/'metrics-overlay-manifest.json']:authenticate_metrics(path)
 real_summary=json.loads((metrics_parent/'summary.json').read_bytes());metrics_here=ROOT/'tools/java-metrics-lifecycle-candidate' 
 java_parent=Path(previous['java_candidate_parent']);source={p:p.read_bytes() for p in HERE.iterdir() if p.is_file() and p.suffix in ['.py','.md','.java']}
 for path in [ROOT/'docs/phase-0/java-metrics-rollback-candidate-plan.md',catalog,parent/'summary.json',parent/'source-input-audit.json',ROOT/'tools/java-raft-reload-candidate/ObservationJUnit.java']:source[path]=path.read_bytes()
 patchdir=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7'
 for path in [patchdir/'0004-honor-metrics-event-lifecycle.patch',patchdir/'metrics-lifecycle-manifest.json']:source[path]=path.read_bytes()
 for row in json.loads((parent/'source-input-audit.json').read_bytes()):
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256']:raise RuntimeError('Certified inherited input changed: '+row['path'])
  source[path]=data
 for path in [metrics_catalog,metrics_parent/'summary.json',metrics_parent/'source-input-audit.json',metrics_parent/'external-input-audit.json',metrics_parent/'metrics-overlay-manifest.json']+list((metrics_parent/'metrics-overlay').rglob('*.class')):
  authenticate_metrics(path) if path!=metrics_catalog else None
  source[path]=path.read_bytes()
 for row in json.loads((metrics_parent/'source-input-audit.json').read_bytes()):
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256']:raise RuntimeError('Certified metrics source changed: '+row['path'])
  source[path]=data
 for path in (ROOT/'tools/go-java-v1-file-at-timeout').glob('*.py'):source[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(source.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';put(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 save(out/'source-input-audit.json',rows)
 a=module('timeout_audit',ROOT/'tools/go-java-v1-inflight/audit.py');p=module('timeout_proxy',ROOT/'tools/go-java-v1-real-at-timeout/proxy.py');v=module('timeout_verify',ROOT/'tools/go-java-v1-file-at-timeout/verify.py');e=module('timeout_evidence',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py');metrics=module('runtime_metrics_verify',HERE/'metrics_verify.py');overlay=module('timeout_overlay',ROOT/'tools/go-java-v1-file-at-timeout/candidate.py');observer=module('file_timeout_native',ROOT/'tools/java-raft-reload-candidate/native_observer.py');java=module('timeout_java',ROOT/'tools/java-preface-candidate/runtime.py');prepare=module('timeout_java_prepare',ROOT/'tools/java-preface-candidate/prepare_candidate.py')
 h=a.sdk.h;cohort=Path(real_summary['cohort']);audit=a.run(cohort);save(out/'go-cohort-audit.json',audit)
 driver_hash='e2a3b2fc726a1ac64e998585db86b30fa8bf3f706195b78bb77c5f99bf877bd9'
 def check_driver():
  if h.sha(h.DRIVER)!=driver_hash:raise RuntimeError('Pinned JDBC dependency bytes changed')
 check_driver();put(out/'dependencies/mysql-connector-j-8.0.33.jar',h.DRIVER.read_bytes());save(out/'dependencies/manifest.json',dict(path=str(h.DRIVER),sha256=driver_hash))
 overlay.verify(java_parent)
 binary=out/'real-timeout.exe';a.sdk.l.run_command(['go','build','-o',binary,ROOT/'tools/go-java-v1-real-at-timeout/main.go'],cohort,a.sdk.c.GO_ENV,out/'go-build.log',timeout=240);a.sdk.l.run_command(['go','version','-m',binary],cohort,a.sdk.c.GO_ENV,out/'go-build-info.txt')
 schema=(ROOT/'tools/go-java-v1-real-at/schema.sql').read_bytes();put(runtime/'schema.sql',schema);put(out/'schema.sql',schema)
 for port in [28091,28092,28093,28094,28095]:
  if h.port_open(port):raise RuntimeError('Owned timeout port is occupied')
 if h.command(['docker','context','show']).stdout.strip()!=b'desktop-linux':raise RuntimeError('Wrong Docker context')
 h.command(['docker','image','inspect',h.IMAGES['db']]);password='seata_timeout_fixture_only';dbname='seata_timeout_'+uuid.uuid4().hex[:8];container=None;tc=None;sdk=None;relay=None;success=False;proof=None;starts=[];pids=[];tc_pids=[];stages=[]
 try:
  mysql_data=runtime/'mysql';mysql_data.mkdir();container=h.command(['docker','create','--pull=never','--memory=512m','--cpus=1','--name','seata-'+runtime.name.lower(),'--label','seata-metrics-rollback='+runtime.name,'--mount','type=bind,source='+mysql_data.as_posix()+',target=/var/lib/mysql','--mount','type=bind,source='+(runtime/'schema.sql').as_posix()+',target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-p','127.0.0.1:28093:3306','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname,h.IMAGES['db']]).stdout.decode().strip();h.command(['docker','start',container])
  def sql(query):
   result=h.command(['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-h','127.0.0.1','-N','-B',dbname,'-e',query],check=False)
   if result.returncode:raise RuntimeError('Owned independent SQL failed: '+result.stdout.decode('utf-8','replace'))
   return result.stdout
  def ready():
   try:sql('SELECT COUNT(*) FROM accounts');return True
   except RuntimeError:return False
  def info():
   item=json.loads(h.command(['docker','inspect',container]).stdout)[0]
   if item['Image']!=h.IMAGES['db'] or item['Config']['Labels']['seata-metrics-rollback']!=runtime.name:raise RuntimeError('Owned image/label changed')
   mount=next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')
   if mount['Source'].replace('\\','/').lower()!=mysql_data.as_posix().lower():raise RuntimeError('Unexpected MySQL data path')
   return item
  h.wait_until(ready,90,'Owned timeout MySQL readiness');item=info();save(out/'container.json',dict(id=container,image=item['Image'],mounts=item['Mounts'],label=runtime.name));starts.append(item['State']['StartedAt'])
  jar=a.sdk.c.ORACLE/'server/target/seata-server.jar'
  if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR changed')
  configuration=f'''server:
  port: 28091
spring:
  main:
    web-application-type: none
logging:
  level:
    org.apache.seata.server.metrics.MetricsSubscriber: DEBUG
  file:
    path: {runtime.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: 28091
    enable-check-auth: false
    max-rollback-retry-timeout: 5000
    enable-parallel-handle-branch: false
    rollback-retry-timeout-unlock-enable: false
    rollback-failed-unlock-enable: false
    retry-dead-threshold: 500
    recovery:
      rollbacking-retry-period: 200
  store:
    mode: file
    session:
      mode: file
    lock:
      mode: file
    file:
      dir: {runtime.as_posix()}/store
      flush-disk-mode: sync
  metrics:
    enabled: true
    registry-type: compact
    exporter-list: prometheus
    exporter-prometheus-port: 28094
  transport:
    rpc-tc-request-timeout: 750
'''
  unit=Path('D:/codex-builds/metrics-lifecycle-20261113');compile_cp=(unit/'effective-classpath.txt').read_text().strip();dep_audit=module('metrics_compile_dependency_audit',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py');dependencies=dep_audit.dependency_inventory(compile_cp)
  if dependencies!=json.loads((unit/'frozen-compile-dependencies.json').read_bytes()):raise RuntimeError('Certified unit compile dependency bytes changed')
  save(out/'compile-dependency-audit.json',dependencies);put(out/'effective-classpath.txt',compile_cp+'\n')
  unit_sources=json.loads((unit/'frozen-source-audit.json').read_bytes())
  for row in unit_sources:
   if h.sha(Path(row['path']))!=row['sha256'] or h.sha(Path(row['snapshot']))!=row['sha256']:raise RuntimeError('Frozen Java unit source changed')
  unit_binaries=json.loads((unit/'frozen-binary-audit.json').read_bytes())
  if h.sha(unit/'frozen-candidate/org/apache/seata/server/metrics/MetricsSubscriber.class')!=unit_binaries['candidate_class_sha256']:raise RuntimeError('Certified unit candidate class changed')
  external={path:path.read_bytes() for path in unit.rglob('*') if path.is_file()};freeze_script=Path('D:/codex-builds/freeze_metrics_unit_inputs_20261113.py');external[freeze_script]=freeze_script.read_bytes()
  external_rows=[]
  for path,data in sorted(external.items()):
   name='external-inputs/'+sha(str(path).encode())[:20]+'.bin';put(out/name,data);external_rows.append(dict(path=str(path),snapshot=name,sha256=sha(data)))
  save(out/'external-input-audit.json',external_rows)
  certified_external=json.loads((metrics_parent/'external-input-audit.json').read_bytes())
  if [(r['path'],r['sha256']) for r in external_rows]!=[(r['path'],r['sha256']) for r in certified_external]:raise RuntimeError('Frozen unit evidence no longer matches certified metrics parent')
  compiled=out/'metrics-overlay';compiled.mkdir();arguments=runtime/'metrics-compile.args'
  def quoted(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
  put(arguments,'\n'.join(quoted(x) for x in ['-encoding','UTF-8','-cp',compile_cp,'-d',compiled,metrics_here/'MetricsSubscriber.java',HERE/'MetricsRollbackControl.java'])+'\n')
  put(out/'metrics-compile.args',arguments.read_bytes());put(out/'compiler-version.txt',h.command([h.JAVA.parent/'javac.exe','-version']).stdout)
  result=h.command([h.JAVA.parent/'javac.exe','@'+str(arguments)],check=False,timeout=90);put(out/'metrics-compile.log',result.stdout)
  if result.returncode:raise RuntimeError('Actual metrics/controller compilation failed')
  candidate_class=compiled/'org/apache/seata/server/metrics/MetricsSubscriber.class'
  authenticate_metrics(metrics_parent/'metrics-overlay/org/apache/seata/server/metrics/MetricsSubscriber.class')
  if h.sha(candidate_class)!=h.sha(metrics_parent/'metrics-overlay/org/apache/seata/server/metrics/MetricsSubscriber.class'):raise RuntimeError('Rollback production metrics class differs from certified FILE16 bytes')
  if h.sha(candidate_class)!=h.sha(unit/'frozen-candidate/org/apache/seata/server/metrics/MetricsSubscriber.class'):raise RuntimeError('Runtime metrics candidate does not match tested class bytes')
  metadata=json.loads((patchdir/'metrics-lifecycle-manifest.json').read_bytes())
  if sha((patchdir/metadata['patch']).read_bytes())!=metadata['patch_sha256'] or sha((metrics_here/'MetricsSubscriber.java').read_bytes().replace(b'\r\n',b'\n'))!=metadata['files'][0]['patched_lf_sha256']:raise RuntimeError('Metrics patch/source manifest differs')
  generated=[dict(path=p.relative_to(compiled).as_posix(),sha256=h.sha(p)) for p in sorted(compiled.rglob('*.class'))];save(out/'metrics-overlay-manifest.json',generated)
  token='phase0-metrics-'+stamp.rsplit('_',1)[1];put(runtime/'.phase0-owned-metrics',token+'\n');opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
  def metrics_state():
   request=urllib.request.Request('http://127.0.0.1:28095/state',headers={'X-Phase0-Owner':token})
   with opener.open(request,timeout=5) as response:
    value=json.load(response)
    if value.get('pid')!=tc.pid:raise RuntimeError('Metrics control belongs to another process')
    return value
  def metrics_snapshot(name,pid,xid=None):
   latest=None;barrier=None
   def observed():
    nonlocal latest,barrier
    try:
     barrier=None
     if name in ['lost','final']:
      marker='ignore unfinished completion metrics, transactionId='+xid.rsplit(':',1)[1]+', status=RollbackRetryTimeout';lines=[line for line in (stages[-1][0]/'tc-stdout.log').read_text(encoding='utf-8',errors='replace').splitlines() if marker in line]
      if len(lines)!=1:return False
      barrier=dict(marker=marker,marker_line_sha256=sha(lines[0].encode()))
     latest=metrics_state();metrics.validate(latest,name,pid,'file:/'+compiled.as_posix()+'/',xid);return True
    except (OSError,RuntimeError):return False
   h.wait_until(observed,12,'Actual non-reset lifecycle metrics '+name,tc)
   path=out/('metrics-'+name+'-'+str(pid)+'.json');save(path,latest);save(out/f'metrics-observation-{name}-{pid}.json',dict(pid=pid,kind=name,marker=barrier['marker'] if barrier else None,marker_line_sha256=barrier['marker_line_sha256'] if barrier else None,markers_before_request=1 if barrier else 0,snapshot_sha256=sha(path.read_bytes())))
   return latest
  put(runtime/'application.yml',configuration);put(out/'application.yml',configuration);env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=runtime.as_posix()
  binary_sha=h.sha(binary)
  def runtime_artifacts():
   if h.sha(jar)!=h.JAR_HASH or h.sha(binary)!=binary_sha or (runtime/'application.yml').read_bytes()!=(out/'application.yml').read_bytes():raise RuntimeError('Actual runtime JAR/binary/configuration changed')
   return dict(java_jar_sha256=h.sha(jar),sdk_binary_sha256=h.sha(binary),configuration_sha256=h.sha(runtime/'application.yml'))
  def journal(name,when):
   store=runtime/'store';files=[]
   if store.exists():
    for path in sorted(store.rglob('*')):
     if path.is_file():
      data=path.read_bytes();relative=path.relative_to(store).as_posix();put(out/'journals'/(name+'-'+when)/relative,data);files.append(dict(path=relative,sha256=sha(data),bytes=len(data)))
   save(out/('journal-'+name+'-'+when+'.json'),dict(stage=name,files=files))
  def start(name,mode):
   nonlocal tc,sdk,relay
   directory=out/name;directory.mkdir();work=runtime/name;work.mkdir();save(directory/'overlay-before.json',overlay.verify(java_parent));check_driver();save(directory/'jdbc-before.json',dict(sha256=h.sha(h.DRIVER)))
   save(directory/'runtime-artifacts-before.json',runtime_artifacts())
   journal(name,'before')
   tc=h.spawn([h.JAVA,'-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),'-verbose:class','-Dloader.main=org.apache.seata.server.metrics.MetricsRollbackControl','-Dseata.it.metrics.dir='+str(runtime),'-Dseata.it.metrics.port=28095','-Dseata.it.metrics.token='+token,'-Dloader.path='+compiled.as_posix()+','+(java_parent/'overlay').as_posix()+','+h.DRIVER.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],directory,'tc',runtime,env);tc_pids.append(tc.pid);save(directory/'tc-process.json',dict(pid=tc.pid,command=tc.args));h.wait_until(lambda:h.port_open(28091),90,'Fresh candidate FILE TC',tc)
   metrics_snapshot('baseline',tc.pid)
   h.wait_until(lambda:h.port_open(28094),10,'Owned metrics exporter bound',tc)
   relay=p.TimeoutProxy(work,directory,mode);relay.start();put(work/'seatago.yml',(ROOT/'tools/go-java-v1-real-at/config.yml').read_bytes());save(work/'database.json',dict(dsn='root:'+password+'@tcp(127.0.0.1:28093)/'+dbname+'?charset=utf8mb4&parseTime=true&loc=UTC',mode='prepare' if name=='initial' else 'serve'))
   sdk=h.spawn([binary,work],directory,'sdk',work,env);pids.append(sdk.pid);stages.append((directory,work))
   h.wait_until(lambda:(work/'rm-identified.ready').exists(),20,'Actual full SDK RM identification',sdk)
   return directory,work
  def stop():
   nonlocal sdk,tc,relay
   if relay:relay.close();relay=None
   h.stop(sdk);sdk=None;h.stop(tc);tc=None
   directory,work=stages[-1];journal(directory.name,'after');save(directory/'retention-class-origins.json',overlay.class_sources(directory/'tc-stdout.log',java_parent/'overlay'));save(directory/'class-origins.json',java.class_sources(directory/'tc-stdout.log',java_parent/'overlay',prepare));save(directory/'overlay-after.json',overlay.verify(java_parent));check_driver();save(directory/'jdbc-after.json',dict(sha256=h.sha(h.DRIVER)));h.wait_until(lambda:not h.port_open(28091) and not h.port_open(28092),10,'TC/SDK ports released')
   save(directory/'runtime-artifacts-after.json',runtime_artifacts())
   text=(directory/'tc-stdout.log').read_text(encoding='utf-8',errors='replace')
   for name in ['org.apache.seata.server.metrics.MetricsSubscriber','org.apache.seata.server.metrics.MetricsRollbackControl']:
    actual=re.findall(r'\[class,load\s*\]\s+'+re.escape(name)+r'\s+source:\s+(\S+)',text)
    if actual!=['file:/'+compiled.as_posix()+'/']:raise RuntimeError('Actual metrics/control class source differs')
   if 'Exception thrown by subscriber method recordGlobalTransactionEventForMetrics' in text:raise RuntimeError('Metrics subscriber still throws')
  def restart_db():
   h.command(['docker','restart',container],timeout=45);h.wait_until(ready,90,'Actual owned MySQL restart');item=info();starts.append(item['State']['StartedAt']);save(out/('database-restart-'+str(len(starts)-1)+'.json'),dict(id=container,image=item['Image'],started_at=item['State']['StartedAt'],pid=item['State']['Pid']))
  query="SELECT 'business',id,balance FROM accounts; SELECT 'undo',xid,branch_id,log_status,HEX(rollback_info) FROM undo_log"
  put(out/'business-tables.txt',sql('SHOW TABLES'))
  def snapshot(name):
   put(out/(name+'.tsv'),sql(query));console(xid,out/('console-'+name.replace('_','-')+'.json'))
   events=[]
   try:
    with socket.create_connection(('127.0.0.1',28091),timeout=5) as stream:observed=observer.query(stream,xid,phase['resource_id'],'ACCOUNTS:1;',15 if name=='final' else 17,name=='final',events)
    save(out/('native-'+name+'.json'),observed)
   finally:save(out/('native-'+name+'-frames.json'),events)
   metrics_snapshot(name,tc.pid,xid)
  directory,work=start('initial','deny-rollback');h.wait_until(lambda:(work/'recovery.json').exists(),20,'Actual local business/Undo phase one',sdk);phase=json.loads((work/'recovery.json').read_bytes());xid=phase['xid']
  registrations=[e.wire.decode(event) for event in list(relay.events) if event.get('event')=='wire' and event.get('payload_type')==11 and event.get('direction')=='sdk-to-tc'];resources={row['resource'] for row in registrations}
  if len(resources)!=1:raise RuntimeError('Actual original branch resource is not unique')
  phase['resource_id']=next(iter(resources));save(out/'phase-one.json',phase)
  def timed_out():
   try:return poll_console(xid)['data'][0]['status']==17
   except (IndexError,KeyError):return False
  h.wait_until(timed_out,30,'Natural persisted RollbackRetryTimeout17',tc);snapshot('timeout');h.wait_until(lambda:(work/'sdk-serving.ready').exists(),20,'Original TM single rollback returned',sdk);stop()
  restart_db();directory,work=start('operator_loss','lose-ack');snapshot('restart')
  relay.record(event='operator-command-start',xid=xid);first=operator(xid,directory/'operator.json');h.wait_until(lambda:any(row['event']=='successful-rollback-response-lost' for row in relay.events),5,'Actual completed compensation response loss',sdk);snapshot('lost');stop()
  restart_db();directory,work=start('operator_replay','pass');snapshot('second_restart');relay.record(event='operator-command-start',xid=xid);second=operator(xid,directory/'operator.json')
  h.wait_until(lambda:poll_console(xid).get('total')==0,10,'Original TC final session removal',tc);snapshot('final');stop()
  save(out/'runtime.json',dict(sdk_pids=pids,tc_pids=tc_pids,database_starts=starts,storage_mode='file'));proof=v.validate(e.derive(out));save(out/'proof.json',proof);save(out/'metrics-proof.json',metrics.derive(out))
  tests_env=h.ENV.copy();tests_env['SEATA_FILE_TIMEOUT_EVIDENCE']=str(out);a.sdk.l.run_command(['python','-m','unittest','discover','-s',ROOT/'tools/go-java-v1-file-at-timeout','-p','test_*.py','-v'],ROOT,tests_env,out/'python-tests.log')
  tests_env['SEATA_METRICS_ROLLBACK_EVIDENCE']=str(out);a.sdk.l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,tests_env,out/'metrics-python-tests.log')
  if a.run(cohort)!=audit:raise RuntimeError('Certified Go cohort changed')
  for path,data in source.items():
   if path.read_bytes()!=data:raise RuntimeError('Frozen input changed: '+str(path))
  a.sdk.c.audit_go();check_driver()
  if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR changed during run')
  for path,data in external.items():
   if path.read_bytes()!=data:raise RuntimeError('External certified unit input changed')
  if dep_audit.dependency_inventory(compile_cp)!=dependencies:raise RuntimeError('Actual metrics compile dependencies changed')
  if generated!=[dict(path=p.relative_to(compiled).as_posix(),sha256=h.sha(p)) for p in sorted(compiled.rglob('*.class'))]:raise RuntimeError('Runtime metrics overlay changed')
  runtime_artifacts();save(out/'final-input-check.json',dict(frozen_sources_unchanged=True,go_cohort_unchanged=True,original_jar_unchanged=True,sdk_binary_unchanged=True,configuration_unchanged=True));success=True
 finally:
  if relay:relay.close()
  h.stop(sdk);h.stop(tc)
  state=None
  if container:
   item=info();h.command(['docker','stop','-t','10',container],timeout=30);state=info()['State']['Status']
  h.wait_until(lambda:not any(h.port_open(p) for p in [28091,28092,28093,28094,28095]),10,'Owned timeout resources released')
  save(out/'result.json',dict(succeeded=success,container=container,container_state=state,ports_released=True,runtime=str(runtime),proof=proof))
 if success:
  save(out/'summary.json',dict(succeeded=True,parent_evidence=str(parent),cohort=str(cohort),parent_summary_sha256=h.sha(parent/'summary.json'),java_candidate_parent=str(java_parent),java_jar_sha256=h.sha(jar),binary_sha256=h.sha(binary),driver_sha256=h.sha(h.DRIVER),mysql_image=h.IMAGES['db'],frozen_inputs=len(source),business_python_tests=int(re.search(rb'Ran (\d+) tests',(out/'python-tests.log').read_bytes()).group(1)),metrics_python_tests=int(re.search(rb'Ran (\d+) tests',(out/'metrics-python-tests.log').read_bytes()).group(1)),inherited_go_tests=254,inherited_rust_tests=117,inherited_java_tests=617,new_go_unit_execution=False,new_rust_execution=False,new_java_unit_execution=False,inherited_candidate_java_tests=77,scope='Independent metrics lifecycle candidate with certified retention/preface; real FILE17 rollback recovery plus actual per-process non-reset metrics/events. Not full restart metric reconstruction,16+17 concurrent,RAFT,allSQL/crash,productionRust or completeWP00.'))
  print(json.dumps(dict(succeeded=True,evidence_directory=str(out),runtime=str(runtime))),flush=True)

if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output-root',type=Path,default=ROOT/'docs/phase-0/evidence/local-java-metrics-rollback-candidate');run(parser.parse_args().output_root)
