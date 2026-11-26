"""Owned real MySQL AT -> natural17 -> restart -> operator loss -> restart -> durable replay."""
import argparse,datetime,hashlib,importlib.util,json,subprocess,time,urllib.parse,urllib.request,urllib.error,uuid
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

def run(output_root):
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=output_root/('run_'+stamp);out.mkdir(parents=True);runtime=Path('D:/codex-builds')/('rto_'+stamp);runtime.mkdir()
 parent=Path(json.loads((ROOT/'docs/phase-0/evidence/local-go-java-real-at/latest_run.json').read_bytes())['evidence_directory']);previous=json.loads((parent/'summary.json').read_bytes())
 catalog=parent.parent/'manifest.json';catalog_data=catalog.read_bytes()
 if sha(catalog_data)!='7a9efa62d8313dbbc61613b91d61d654ee28205208a24db5ea461e257e5e7943':raise RuntimeError('Certified real AT parent catalog changed')
 inventory=json.loads(catalog_data)
 if inventory['primary']!=parent.name:raise RuntimeError('Parent pointer is not the certified primary')
 for path in [parent/'summary.json',parent/'source-input-audit.json']:
  expected=next(row['sha256'] for row in inventory['files'] if row['path']==path.relative_to(catalog.parent).as_posix())
  if sha(path.read_bytes())!=expected:raise RuntimeError('Certified real AT parent metadata changed')
 source={p:p.read_bytes() for p in HERE.iterdir() if p.is_file() and p.suffix in ['.py','.go']};plan=ROOT/'docs/phase-0/go-java-v1-real-at-timeout-plan.md';source[plan]=plan.read_bytes()
 for path in [catalog,parent/'summary.json',parent/'source-input-audit.json']:source[path]=path.read_bytes()
 for row in json.loads((parent/'source-input-audit.json').read_bytes()):
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256']:raise RuntimeError('Inherited certified input changed: '+row['path'])
  source[path]=data
 rows=[]
 for path,data in sorted(source.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';put(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 save(out/'source-input-audit.json',rows)
 a=module('timeout_audit',ROOT/'tools/go-java-v1-inflight/audit.py');p=module('timeout_proxy',HERE/'proxy.py');v=module('timeout_verify',HERE/'verify.py');e=module('timeout_evidence',HERE/'evidence.py');overlay=module('timeout_overlay',ROOT/'tools/go-java-v1-real-at/overlay.py');java=module('timeout_java',ROOT/'tools/java-preface-candidate/runtime.py');prepare=module('timeout_java_prepare',ROOT/'tools/java-preface-candidate/prepare_candidate.py')
 h=a.sdk.h;cohort=Path(previous['cohort']);audit=a.run(cohort);save(out/'go-cohort-audit.json',audit)
 driver_hash='e2a3b2fc726a1ac64e998585db86b30fa8bf3f706195b78bb77c5f99bf877bd9'
 def check_driver():
  if h.sha(h.DRIVER)!=driver_hash:raise RuntimeError('Pinned JDBC dependency bytes changed')
 check_driver();put(out/'dependencies/mysql-connector-j-8.0.33.jar',h.DRIVER.read_bytes());save(out/'dependencies/manifest.json',dict(path=str(h.DRIVER),sha256=driver_hash))
 go_parent=Path(previous['parent_evidence']);java_parent=Path(json.loads((go_parent/'summary.json').read_bytes())['inherited_java_evidence']);overlay.verify(java_parent)
 binary=out/'real-timeout.exe';a.sdk.l.run_command(['go','build','-o',binary,HERE/'main.go'],cohort,a.sdk.c.GO_ENV,out/'go-build.log',timeout=240);a.sdk.l.run_command(['go','version','-m',binary],cohort,a.sdk.c.GO_ENV,out/'go-build-info.txt')
 schema=h.command(['git','-C',a.sdk.c.ORACLE,'show',h.PIN+':script/server/db/mysql.sql']).stdout+(ROOT/'tools/go-java-v1-real-at/schema.sql').read_bytes();put(runtime/'schema.sql',schema);put(out/'schema.sql',schema)
 for port in [28091,28092,28093]:
  if h.port_open(port):raise RuntimeError('Owned timeout port is occupied')
 if h.command(['docker','context','show']).stdout.strip()!=b'desktop-linux':raise RuntimeError('Wrong Docker context')
 h.command(['docker','image','inspect',h.IMAGES['db']]);password='seata_timeout_fixture_only';dbname='seata_timeout_'+uuid.uuid4().hex[:8];container=None;tc=None;sdk=None;relay=None;success=False;proof=None;starts=[];pids=[];tc_pids=[];stages=[]
 try:
  mysql_data=runtime/'mysql';mysql_data.mkdir();container=h.command(['docker','create','--pull=never','--name','seata-'+runtime.name.lower(),'--label','seata-real-at-timeout='+runtime.name,'--mount','type=bind,source='+mysql_data.as_posix()+',target=/var/lib/mysql','--mount','type=bind,source='+(runtime/'schema.sql').as_posix()+',target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-p','127.0.0.1:28093:3306','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname,h.IMAGES['db']]).stdout.decode().strip();h.command(['docker','start',container])
  def sql(query):
   result=h.command(['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-N','-B',dbname,'-e',query],check=False)
   if result.returncode:raise RuntimeError('Owned independent SQL failed: '+result.stdout.decode('utf-8','replace'))
   return result.stdout
  def ready():
   try:sql('SELECT COUNT(*) FROM accounts');return True
   except RuntimeError:return False
  def info():
   item=json.loads(h.command(['docker','inspect',container]).stdout)[0]
   if item['Image']!=h.IMAGES['db'] or item['Config']['Labels']['seata-real-at-timeout']!=runtime.name:raise RuntimeError('Owned image/label changed')
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
    rollback-retry-timeout-unlock-enable: false
    rollback-failed-unlock-enable: false
    retry-dead-threshold: 500
    recovery:
      rollbacking-retry-period: 200
  store:
    mode: db
    session:
      mode: db
    lock:
      mode: db
    db:
      datasource: druid
      db-type: mysql
      driver-class-name: com.mysql.cj.jdbc.Driver
      url: jdbc:mysql://127.0.0.1:28093/{dbname}?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC
      user: root
      password: {password}
      min-conn: 1
      max-conn: 4
  transport:
    rpc-tc-request-timeout: 750
'''
  put(runtime/'application.yml',configuration);put(out/'application.yml',configuration);env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=runtime.as_posix()
  def start(name,mode):
   nonlocal tc,sdk,relay
   directory=out/name;directory.mkdir();work=runtime/name;work.mkdir();save(directory/'overlay-before.json',overlay.verify(java_parent));check_driver();save(directory/'jdbc-before.json',dict(sha256=h.sha(h.DRIVER)))
   tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),'-verbose:class','-Dloader.path='+(java_parent/'overlay').as_posix()+','+h.DRIVER.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],directory,'tc',runtime,env);tc_pids.append(tc.pid);h.wait_until(lambda:h.port_open(28091),90,'Fresh original DB TC',tc)
   relay=p.TimeoutProxy(work,directory,mode);relay.start();put(work/'seatago.yml',(ROOT/'tools/go-java-v1-real-at/config.yml').read_bytes());save(work/'database.json',dict(dsn='root:'+password+'@tcp(127.0.0.1:28093)/'+dbname+'?charset=utf8mb4&parseTime=true&loc=UTC',mode='prepare' if name=='initial' else 'serve'))
   sdk=h.spawn([binary,work],directory,'sdk',work,env);pids.append(sdk.pid);stages.append((directory,work))
   h.wait_until(lambda:(work/'rm-identified.ready').exists(),20,'Actual full SDK RM identification',sdk)
   return directory,work
  def stop():
   nonlocal sdk,tc,relay
   if relay:relay.close();relay=None
   h.stop(sdk);sdk=None;h.stop(tc);tc=None
   directory,work=stages[-1];save(directory/'class-origins.json',java.class_sources(directory/'tc-stdout.log',java_parent/'overlay',prepare));save(directory/'overlay-after.json',overlay.verify(java_parent));check_driver();save(directory/'jdbc-after.json',dict(sha256=h.sha(h.DRIVER)));h.wait_until(lambda:not h.port_open(28091) and not h.port_open(28092),10,'TC/SDK ports released')
  def restart_db():
   h.command(['docker','restart',container],timeout=45);h.wait_until(ready,90,'Actual owned MySQL restart');item=info();starts.append(item['State']['StartedAt']);save(out/('database-restart-'+str(len(starts)-1)+'.json'),dict(id=container,image=item['Image'],started_at=item['State']['StartedAt'],pid=item['State']['Pid']))
  query="SELECT 'business',id,balance FROM accounts; SELECT 'undo',xid,branch_id,log_status,HEX(rollback_info) FROM undo_log; SELECT 'global',xid,status,begin_time FROM global_table; SELECT 'branch',xid,branch_id,status,resource_id,branch_type FROM branch_table; SELECT 'lock',xid,branch_id,resource_id,table_name,pk,status FROM lock_table"
  directory,work=start('initial','deny-rollback');h.wait_until(lambda:(work/'recovery.json').exists(),20,'Actual local business/Undo phase one',sdk);phase=json.loads((work/'recovery.json').read_bytes());save(out/'phase-one.json',phase);xid=phase['xid']
  def timed_out():return sql('SELECT status FROM global_table WHERE xid=\''+xid+'\'').strip()==b'17'
  h.wait_until(timed_out,30,'Natural persisted RollbackRetryTimeout17',tc);put(out/'timeout.tsv',sql(query));console(xid,out/'console-timeout.json');h.wait_until(lambda:(work/'sdk-serving.ready').exists(),20,'Original TM single rollback returned',sdk);stop()
  restart_db();directory,work=start('operator_loss','lose-ack');put(out/'restart.tsv',sql(query));console(xid,out/'console-restart.json')
  relay.record(event='operator-command-start',xid=xid);first=operator(xid,directory/'operator.json');h.wait_until(lambda:any(row['event']=='successful-rollback-response-lost' for row in relay.events),5,'Actual completed compensation response loss',sdk);put(out/'lost.tsv',sql(query));console(xid,out/'console-lost.json');stop()
  restart_db();directory,work=start('operator_replay','pass');put(out/'second_restart.tsv',sql(query));console(xid,out/'console-second-restart.json');relay.record(event='operator-command-start',xid=xid);second=operator(xid,directory/'operator.json')
  h.wait_until(lambda:sql('SELECT COUNT(*) FROM global_table').strip()==b'0',10,'Original TC final session removal',tc);put(out/'final.tsv',sql(query));console(xid,out/'console-final.json');stop()
  save(out/'runtime.json',dict(sdk_pids=pids,tc_pids=tc_pids,database_starts=starts));proof=v.validate(e.derive(out));save(out/'proof.json',proof)
  tests_env=h.ENV.copy();tests_env['SEATA_AT_TIMEOUT_EVIDENCE']=str(out);a.sdk.l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,tests_env,out/'python-tests.log')
  if a.run(cohort)!=audit:raise RuntimeError('Certified Go cohort changed')
  for path,data in source.items():
   if path.read_bytes()!=data:raise RuntimeError('Frozen input changed: '+str(path))
  a.sdk.c.audit_go();check_driver();success=True
 finally:
  if relay:relay.close()
  h.stop(sdk);h.stop(tc)
  state=None
  if container:
   item=info();h.command(['docker','stop','-t','10',container],timeout=30);state=info()['State']['Status']
  h.wait_until(lambda:not any(h.port_open(p) for p in [28091,28092,28093]),10,'Owned timeout resources released')
  save(out/'result.json',dict(succeeded=success,container=container,container_state=state,ports_released=True,runtime=str(runtime),proof=proof))
 if success:
  save(out/'summary.json',dict(succeeded=True,parent_evidence=str(parent),parent_summary_sha256=h.sha(parent/'summary.json'),java_jar_sha256=h.sha(jar),binary_sha256=h.sha(binary),driver_sha256=h.sha(h.DRIVER),mysql_image=h.IMAGES['db'],frozen_inputs=len(source),python_tests=25,inherited_go_tests=254,inherited_rust_tests=117,inherited_java_tests=617,new_go_unit_execution=False,new_rust_execution=False,new_java_unit_execution=False,scope='One real MySQL AT naturally generated17; two TC/SDK/database restarts; actual operator rollback, lost real acknowledgement and persisted idempotent replay. Not16, FILE/RAFT, production Rust or complete WP00.'))
  print(json.dumps(dict(succeeded=True,evidence_directory=str(out),runtime=str(runtime))),flush=True)

if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output-root',type=Path,default=ROOT/'docs/phase-0/evidence/local-go-java-real-at-timeout');run(parser.parse_args().output_root)
