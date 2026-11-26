"""Owned real Java and Go AT actors sharing one physical row: original namespace diagnosis."""
from pathlib import Path
import datetime,hashlib,importlib.util,json,os,socket,time,urllib.parse,urllib.request,uuid,xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;OPENER=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def save(path,data):put(path,json.dumps(data,indent=2)+'\n')
def run():
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-0/evidence/local-java-go-at-shared-lock'/('run_'+stamp);out.mkdir(parents=True);runtime=Path('D:/codex-builds')/('slock_'+stamp);runtime.mkdir()
 a=module('shared_lock_sdk_audit',ROOT/'tools/go-java-v1-inflight/audit.py');h=a.sdk.h;deps=module('shared_lock_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py');proxy=module('shared_lock_recorder',HERE/'proxy.py');wire=module('shared_lock_wire',HERE/'wire.py');observer=module('shared_lock_native',ROOT/'tools/java-raft-reload-candidate/native_observer.py')
 parent=ROOT/'docs/phase-0/evidence/local-java-preface-candidate/run_20261112_221159_a16eafb1';catalog=parent.parent/'manifest.json'
 if sha(catalog.read_bytes())!='743a16b3541fe5b618f939628dfb2bf806f60c6bf0e4dc784e82341bca6fed8a':raise RuntimeError('Certified preface parent changed')
 allowed={r['path']:r['sha256'] for r in json.loads(catalog.read_bytes())['files']}
 def parent_check(path):
  if sha(path.read_bytes())!=allowed[path.relative_to(catalog.parent).as_posix()]:raise RuntimeError('Certified overlay input changed')
 original=ROOT.parent/'seata-java-phase09-oracle';jar=original/'server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR changed')
 unit_xml=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3/test-suite/test-new-version/target/surefire-reports/TEST-org.apache.seata.server.session.RetryTimeoutRetentionCandidateTest.xml');cp=next(row.attrib['value'] for row in ET.parse(unit_xml).findall('./properties/property') if row.attrib['name']=='java.class.path').rstrip(os.pathsep);inventory=deps.dependency_inventory(cp)
 if len(inventory)!=340:raise RuntimeError('Unexpected actual Java client dependency cohort')
 cp+=os.pathsep+str(h.DRIVER)+os.pathsep+'E:/Java/.m2/repository/org/projectlombok/lombok/1.18.36/lombok-1.18.36.jar';inventory=deps.dependency_inventory(cp)
 if h.sha(h.DRIVER)!='e2a3b2fc726a1ac64e998585db86b30fa8bf3f706195b78bb77c5f99bf877bd9':raise RuntimeError('Pinned MySQL driver changed')
 source={p:p.read_bytes() for p in HERE.iterdir() if p.is_file()}
 admission=ROOT/'compatibility/seata-java/writer-admission/src/main/java/io/seatarust/compat/mysql/JavaAtWriterAdmission.java'
 source[admission]=admission.read_bytes()
 paths=[ROOT/'docs/phase-0/java-go-at-shared-lock-reference-plan.md',ROOT/'docs/phase-0/go-at-resource-identity-divergence.md',ROOT/'tools/go-java-v1-real-at/schema.sql',ROOT/'tools/go-java-v1-real-at/config.yml',ROOT/'tools/java-tc-integration/config/application.yml',ROOT/'tools/java-tc-integration/backend_recovery/run.py',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py',ROOT/'tools/go-java-v1-inflight/audit.py',ROOT/'tools/go-java-v1-real-at/verify.py',ROOT/'tools/java-raft-reload-candidate/native_observer.py',unit_xml,catalog]
 for name in ['rm-datasource/src/main/java/org/apache/seata/rm/datasource/DataSourceProxy.java','rm-datasource/src/main/java/org/apache/seata/rm/datasource/ConnectionProxy.java','rm-datasource/src/main/java/org/apache/seata/rm/datasource/initializer/db/MysqlResourceIdInitializer.java']:paths.append(original/name)
 for subdir in ['go-java-v1-inflight','go-java-v1-sdk-profile','go-java-v1-getty-profile','go-java-v1-merge-profile','go-java-v1-exception-profile','go-java-v1-length-patch','go-java-v1-file-at-timeout','go-java-v1-real-at','java-tc-integration/backend_recovery','java-raft-reload-candidate']:
  paths.extend(p for p in (ROOT/'tools'/subdir).iterdir() if p.is_file() and p.suffix in ['.py','.go','.java','.yml','.sql'])
 paths.extend([original/'core/src/main/java/org/apache/seata/core/rpc/netty/NettyClientBootstrap.java',original/'serializer/seata-serializer-seata/src/main/java/org/apache/seata/serializer/seata/protocol/v2/AbstractIdentifyResponseCodecV2.java',original/'common/src/main/java/org/apache/seata/server/console/entity/vo/BranchSessionVO.java'])
 for path in paths:source[path]=path.read_bytes()
 for path in [parent/'overlay-manifest.json']+list((parent/'overlay').rglob('*.class')):parent_check(path);source[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(source.items()):
  leaf='source-inputs/'+sha(str(path).encode())[:20]+'.bin';put(out/leaf,data);rows.append(dict(path=str(path),snapshot=leaf,sha256=sha(data)))
 save(out/'source-input-audit.json',rows);put(out/'effective-classpath.txt',cp);save(out/'compile-dependency-audit.json',inventory)
 cohort=Path('D:/codex-builds/sif_20261112_232026_e565657b');go_audit=a.run(cohort);save(out/'go-cohort-audit.json',go_audit);a.sdk.c.audit_go()
 binary=out/'shared-lock.exe';a.sdk.l.run_command(['go','build','-o',binary,HERE/'main.go'],cohort,a.sdk.c.GO_ENV,out/'go-build.log',timeout=240);a.sdk.l.run_command(['go','version','-m',binary],cohort,a.sdk.c.GO_ENV,out/'go-build-info.txt');binary_hash=h.sha(binary)
 compiled=out/'compiled';compiled.mkdir();args=runtime/'compile.args';put(args,'\n'.join('"'+str(v).replace('\\','\\\\').replace('"','\\"')+'"' for v in ['-encoding','UTF-8','-cp',cp,'-d',compiled,HERE/'SharedLockJavaActor.java',admission])+'\n');put(out/'compile.args',args.read_bytes());built=h.command([h.JAVA.parent/'javac.exe','@'+str(args)],timeout=90,check=False);put(out/'compile.log',built.stdout)
 if built.returncode:raise RuntimeError('Real Java actor compilation failed')
 classes=[dict(path=p.relative_to(compiled).as_posix(),sha256=h.sha(p)) for p in sorted(compiled.rglob('*.class'))];save(out/'compiled-manifest.json',classes)
 for port in [28091,28092,28093,28094]:
  if h.port_open(port):raise RuntimeError('Owned port occupied')
 password='seata_shared_lock_fixture_only';dbname='seata_shared_'+uuid.uuid4().hex[:8];schema=(ROOT/'tools/go-java-v1-real-at/schema.sql').read_bytes();put(runtime/'schema.sql',schema);container=None;tc=None;java=None;go=None;relays=[];success=False;proof=None
 try:
  data=runtime/'mysql';data.mkdir();container=h.command(['docker','create','--pull=never','--memory=512m','--cpus=1','--name','seata-'+runtime.name.lower(),'--label','seata-shared-lock='+runtime.name,'--mount','type=bind,source='+data.as_posix()+',target=/var/lib/mysql','--mount','type=bind,source='+(runtime/'schema.sql').as_posix()+',target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-p','127.0.0.1:28093:3306','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname,h.IMAGES['db']]).stdout.decode().strip();h.command(['docker','start',container])
  def owned_db():
   item=json.loads(h.command(['docker','inspect',container]).stdout)[0];mount=next(row for row in item['Mounts'] if row['Destination']=='/var/lib/mysql')
   if item['Image']!=h.IMAGES['db'] or item['Config']['Labels']['seata-shared-lock']!=runtime.name or mount['Source'].replace('\\','/').lower()!=data.as_posix().lower():raise RuntimeError('Owned MySQL identity changed')
   return item
  def sql(query):return h.command(['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-h','127.0.0.1','-N','-B',dbname,'-e',query]).stdout
  def ready():
   try:sql('SELECT COUNT(*) FROM accounts');return True
   except RuntimeError:return False
  h.wait_until(ready,90,'Owned shared MySQL ready');save(out/'container.json',owned_db());put(out/'initial.tsv',sql('SELECT id,balance FROM accounts; SELECT COUNT(*) FROM undo_log'));work=runtime/'tc';work.mkdir();tc_dir=out/'tc';tc_dir.mkdir();config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text().replace('18091','28091');put(work/'application.yml',config);put(tc_dir/'application.yml',config);env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=work.as_posix()
  command=[h.JAVA,'-Xlog:class+load=info','-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+work.as_posix(),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),'-Dloader.path='+(parent/'overlay').as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'];tc=h.spawn(command,tc_dir,'tc',work,env);save(tc_dir/'process.json',dict(pid=tc.pid,command=list(map(str,command))));h.wait_until(lambda:h.port_open(28091),90,'Owned TC ready',tc)
  def console(xid,label):
   url='http://127.0.0.1:28091/api/v1/console/globalSession/query?'+urllib.parse.urlencode(dict(xid=xid,pageNum=1,pageSize=20,withBranch='true'))
   with OPENER.open(url,timeout=5) as response:raw=response.read()
   if label:put(out/('console-'+label+'.json'),raw)
   return json.loads(raw)
  for label,port in [('java',28094),('go',28092)]:
   (runtime/label).mkdir();(out/label).mkdir();relay=proxy.WireProxy(out/label,port);relays.append(relay);relay.start()
  jw=runtime/'java';gw=runtime/'go';save(jw/'database.json',dict(jdbc_url='jdbc:mysql://127.0.0.1:28093/'+dbname+'?useSSL=false&allowPublicKeyRetrieval=true',user='root',password=password));save(gw/'database.json',dict(dsn='root:'+password+'@tcp(127.0.0.1:28093)/'+dbname+'?charset=utf8mb4&parseTime=true&loc=UTC'));put(gw/'seatago.yml',(ROOT/'tools/go-java-v1-real-at/config.yml').read_bytes())
  command=[h.JAVA,'-Xlog:class+load=info','-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+jw.as_posix(),'-cp',str(compiled)+os.pathsep+cp,'SharedLockJavaActor',jw];java=h.spawn(command,out/'java','sdk',jw,h.ENV);save(out/'java/process.json',dict(pid=java.pid,command=list(map(str,command))));h.wait_until(lambda:(jw/'phase-one.json').exists(),45,'Real Java phase one committed',java);jp=json.loads((jw/'phase-one.json').read_bytes());save(out/'java/phase-one.json',jp);put(out/'java-held.tsv',sql('SELECT id,balance FROM accounts; SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log'));console(jp['xid'],'java-held')
  go=h.spawn([binary,gw],out/'go','sdk',gw,h.ENV);save(out/'go/process.json',dict(pid=go.pid,command=[str(binary),str(gw)]));h.wait_until(lambda:(gw/'begun.json').exists(),30,'Real Go Begin before SQL',go);gb=json.loads((gw/'begun.json').read_bytes());save(out/'go/begun.json',gb)
  def registration(events):
   for event in events:
    if event['event']=='wire':
     value=wire.decode(event)
     if value['code']==11:return value
   raise RuntimeError('Original AT branch request missing')
  jreg=registration(list(relays[0].events));go_resources={wire.decode(event)['resource'] for event in list(relays[1].events) if event['event']=='wire' and event.get('payload_type')==103 and event['direction']=='sdk-to-tc'}
  if len(go_resources)!=1:raise RuntimeError('Original Go resource registration missing')
  gr=go_resources.pop();predicted_go_lock='ACCOUNTS:1;';comparisons=[('java-java',jreg['resource'],jreg['lock_key']),('go-java',gr,jreg['lock_key']),('java-go',jreg['resource'],predicted_go_lock),('go-go',gr,predicted_go_lock)]
  def query(stage,label,xid,resource,key,status):
   events=[]
   with socket.create_connection(('127.0.0.1',28091),timeout=5) as stream:
    identified=observer.exchange(stream,101,1,b''.join(observer.string(s) for s in ['1.5.2','owned-retention-observer','default_tx_group','']),102,events)
    if identified[0]!=1:raise RuntimeError('Read-only observer rejected')
    actual=observer.exchange(stream,15,2,observer.string(xid)+observer.string(''),16,events)
    if actual!=bytes([1,0,status]):raise RuntimeError('Original target global status differs')
    observer_xid='127.0.0.1:28091:999999999000000001';body=observer.string(observer_xid)+b'\0'+observer.string(resource)+observer.string(key,4)+observer.string('{}',4);result=observer.exchange(stream,21,3,body,22,events)
    if result[:3]!=b'\1\0\0' or len(result)!=4 or result[3] not in [0,1]:raise RuntimeError('Actual lock query response malformed')
   save(out/f'query-{stage}-{label}.json',dict(xid=xid,status=status,resource=resource,lock_key=key,observer_xid=observer_xid,lockable=bool(result[3]),target_mutations=0,frames=events))
  for label,res,key in comparisons:query('java-held',label,gb['xid'],res,key,1)
  put(gw/'start-update.ready','Owned Java-held query barrier complete\n');h.wait_until(lambda:(gw/'phase-one.json').exists(),30,'Actual Go same-row phase one outcome',go);gp=json.loads((gw/'phase-one.json').read_bytes());save(out/'go/phase-one.json',gp);greg=registration(list(relays[1].events))
  if greg['lock_key']!=predicted_go_lock or greg['resource']!=gr:raise RuntimeError('Original Go lock/resource differs from captured comparison inputs')
  put(out/'both-held.tsv',sql('SELECT id,balance FROM accounts; SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log'));console(jp['xid'],'both-java');console(gp['xid'],'both-go')
  for label,res,key in comparisons:query('both-held',label,gp['xid'],res,key,1)
  put(gw/'release.ready','Original Go TM commit authorized\n');h.wait_until(lambda:(gw/'finished.ready').exists(),30,'Original Go global commit',go);h.wait_until(lambda:sql("SELECT COUNT(*) FROM undo_log WHERE xid='"+gp['xid']+"'").strip()==b'0',30,'Actual Go Undo cleanup',go);h.wait_until(lambda:console(gp['xid'],None).get('total')==0,30,'Original Go TC final removal',tc)
  put(jw/'release.ready','Original Java TM commit authorized\n');h.wait_until(lambda:(jw/'finished.ready').exists(),30,'Original Java global commit',java);h.wait_until(lambda:sql('SELECT COUNT(*) FROM undo_log').strip()==b'0',30,'Actual Java Undo cleanup',java);h.wait_until(lambda:console(jp['xid'],None).get('total')==0,30,'Original Java TC final removal',tc)
  put(out/'final.tsv',sql('SELECT id,balance FROM accounts; SELECT COUNT(*) FROM undo_log'));console(jp['xid'],'final-java');console(gp['xid'],'final-go')
  for label,res,key in comparisons:query('final',label,gp['xid'],res,key,15)
  for relay in relays:relay.close()
  relays=[];put(jw/'stop.ready','Owned observation complete\n');put(gw/'stop.ready','Owned observation complete\n');java.wait(timeout=10);go.wait(timeout=10)
  if java.returncode or go.returncode:raise RuntimeError('Real actor failed after cleanup')
  save(out/'runtime.json',dict(tc_pid=tc.pid,java_pid=java.pid,go_pid=go.pid,container=container,runtime=str(runtime),physical_database=dbname,go_cohort=str(cohort),java_exit_code=java.returncode,go_exit_code=go.returncode));verifier=module('shared_lock_actual_evidence',HERE/'evidence.py');proof=verifier.derive(out);save(out/'proof.json',proof)
  if deps.dependency_inventory(cp)!=inventory or a.run(cohort)!=go_audit or h.sha(binary)!=binary_hash or h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original runtime cohort changed')
  for path,raw in source.items():
   if path.read_bytes()!=raw:raise RuntimeError('Frozen source changed: '+str(path))
  a.sdk.c.audit_go();save(out/'final-input-check.json',dict(source_inputs=True,java_dependencies=True,go_cohort=True,original_jar=True));success=True
 finally:
  for relay in relays:relay.close()
  for process in [go,java,tc]:h.stop(process)
  state=None
  if container:owned_db();h.command(['docker','stop','-t','10',container],timeout=30);state=owned_db()['State']['Status']
  h.wait_until(lambda:not any(h.port_open(port) for port in [28091,28092,28093,28094]),15,'Owned shared-lock ports released');save(out/'result.json',dict(succeeded=success,expected_defect_reproduced=success,recovery_certified=False,container=container,container_state=state,ports_released=True,runtime=str(runtime),proof=proof))
 if success:
  save(out/'summary.json',dict(succeeded=True,expected_defect_reproduced=True,shared_global_mutex_certified=False,frozen_inputs=len(source),go_binary_sha256=binary_hash,java_jar_sha256=h.JAR_HASH,java_classpath_entries=len(inventory),java_dependency_files=sum(1 if row['kind']=='file' else len(row['files']) for row in inventory),scope='Actual original Java and Go AT phase one on same MySQL row. Defect reproduction, not successful cross-language mutual exclusion or Undo interchange.'));print(json.dumps(dict(expected_defect_reproduced=True,evidence_directory=str(out))),flush=True)
if __name__=='__main__':run()
