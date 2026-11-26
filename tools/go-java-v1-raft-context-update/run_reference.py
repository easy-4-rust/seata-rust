"""Owned real three-node RAFT, updated TCC context reference reproduction; not recovery certification and durable replay."""
import argparse,datetime,hashlib,importlib.util,json,os,re,socket,time,urllib.parse,urllib.request,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
HOST='192.168.1.10';NATIVE=[28291,28293,28295];RAFT=[28292,28294,28296];CONTROL=[28101,28102,28103];PORTS=NATIVE+RAFT+CONTROL+[28092,28093]
OPENER=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
def catalog(parent,expected):
 path=parent.parent/'manifest.json';data=path.read_bytes()
 if sha(data)!=expected:raise RuntimeError('Certified parent catalog changed')
 rows={r['path']:r['sha256'] for r in json.loads(data)['files']}
 def check(path):
  if sha(path.read_bytes())!=rows[path.relative_to(parent.parent).as_posix()]:raise RuntimeError('Certified parent input differs: '+str(path))
 return check

def run(output_root):
 if output_root.resolve().drive.upper() not in ['D:','E:']:raise ValueError('Evidence must stay on D/E')
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=output_root/('run_'+stamp);out.mkdir(parents=True);runtime=Path('D:/codex-builds')/('rcx_'+stamp);runtime.mkdir();token='phase0-raft-'+stamp.rsplit('_',1)[1]
 parent=ROOT/'docs/phase-0/evidence/local-go-java-raft-commit-timeout/run_20261113_055235_cb3164e4';check=catalog(parent,'c3bd4466f4d84adb0c367c8566350949dc4780262f653eeb9fa568b37f040c53')
 commit_parent=ROOT/'docs/phase-0/evidence/local-java-metrics-lifecycle-candidate/run_20261113_043739_9fbdad8c';check_commit=catalog(commit_parent,'ddb4ded1ccd8740d2bc603722f1808caa6d61feed9b652470663bbd885cbe666')
 raft_parent=ROOT/'docs/phase-0/evidence/local-java-raft-reload-candidate/run_20261113_030623_c4a999a3';check_raft=catalog(raft_parent,'e2a284c1cdf7f2b09821ec2daf363730ce9b115f445a907303760d80a5cb79a2')
 source={p:p.read_bytes() for p in HERE.iterdir() if p.is_file()};plan=ROOT/'docs/phase-0/java-raft-context-replication-candidate-plan.md';source[plan]=plan.read_bytes()
 for owner,verify in [(parent,check),(raft_parent,check_raft),(commit_parent,check_commit)]:
  for path in [owner.parent/'manifest.json',owner/'source-input-audit.json']:
   if path.name!='manifest.json':verify(path)
   source[path]=path.read_bytes()
  for row in json.loads((owner/'source-input-audit.json').read_bytes()):
   path=Path(row['path']);path=path if path.is_absolute() else ROOT/path;data=path.read_bytes()
   if sha(data)!=row['sha256']:raise RuntimeError('Inherited source changed: '+str(path))
   source[path]=data
 for path in [parent/'summary.json',parent/'real-commit.exe',raft_parent/'overlay-manifest.json',raft_parent/'effective-classpath.txt',raft_parent/'compile-dependency-audit.json']+list((raft_parent/'overlay').rglob('*.class')):
  (check if path.is_relative_to(parent) else check_raft)(path);source[path]=path.read_bytes()
 for path in [commit_parent/'summary.json',commit_parent/'real-commit.exe',parent/'compiled-manifest.json']+list((parent/'compiled').rglob('*.class')):
  (check_commit if path.is_relative_to(commit_parent) else check)(path);source[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(source.items()):
  leaf='source-inputs/'+sha(str(path).encode())[:20]+'.bin';put(out/leaf,data);rows.append(dict(path=str(path),snapshot=leaf,sha256=sha(data)))
 save(out/'source-input-audit.json',rows)
 a=module('raft_business_sdk_audit',ROOT/'tools/go-java-v1-inflight/audit.py');h=a.sdk.h;proxy=module('raft_business_proxy',HERE/'proxy.py');checks=module('raft_business_checks',HERE/'reference_checks.py');observer=module('raft_business_native',ROOT/'tools/java-raft-reload-candidate/native_observer.py');deps=module('raft_business_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
 for port in PORTS:
  with socket.socket() as listener:listener.bind((HOST if port in RAFT else '127.0.0.1',port))
 summary=json.loads((parent/'summary.json').read_bytes());cohort=Path(summary['cohort']);audit=a.run(cohort);save(out/'go-cohort-audit.json',audit)
 jar=a.sdk.c.ORACLE/'server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR differs')
 binary=out/'real-commit.exe';a.sdk.l.run_command(['go','build','-o',binary,HERE/'main.go',HERE/'owned_commit_action.go'],cohort,a.sdk.c.GO_ENV,out/'go-build.log',timeout=240)
 a.sdk.l.run_command(['go','version','-m',binary],cohort,a.sdk.c.GO_ENV,out/'go-build-info.txt')
 cp=(raft_parent/'effective-classpath.txt').read_text().strip()
 if deps.dependency_inventory(cp)!=json.loads((raft_parent/'compile-dependency-audit.json').read_bytes()):raise RuntimeError('Certified RAFT compile dependency cohort changed')
 cp=str(raft_parent/'overlay')+os.pathsep+cp;dependencies=deps.dependency_inventory(cp)
 save(out/'compile-dependency-audit.json',dependencies);put(out/'effective-classpath.txt',cp+'\n');compiled=out/'compiled';compiled.mkdir();argfile=runtime/'compile.args'
 def quote(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
 args=['-encoding','UTF-8','-cp',cp,'-d',compiled,HERE/'RaftBusinessControl.java',HERE/'SnapshotMetadataReader.java',ROOT/'tools/java-tc-integration/raft_recovery/ReadRaftExceptions.java'];put(argfile,'\n'.join(quote(x) for x in args)+'\n');put(out/'compile.args',argfile.read_bytes());result=h.command([h.JAVA.parent/'javac.exe','@'+str(argfile)],timeout=90,check=False);put(out/'compile.log',result.stdout)
 if result.returncode:raise RuntimeError('Read-only RAFT control compilation failed')
 for row in json.loads((parent/'compiled-manifest.json').read_bytes()):
  if h.sha(compiled/row['path'])!=row['sha256']:raise RuntimeError('Read-only controller/diagnostic class differs from certified RAFT17 parent')
 generated=[dict(path=p.relative_to(compiled).as_posix(),sha256=h.sha(p)) for p in sorted(compiled.rglob('*.class'))];save(out/'compiled-manifest.json',generated)
 overlay=out/'overlay'
 for row in json.loads((raft_parent/'overlay-manifest.json').read_bytes()):
  if 'RaftReloadObservationControl' not in row['path']:put(overlay/row['path'],(raft_parent/'overlay'/row['path']).read_bytes())
 for path in (compiled/'org/apache/seata/server/cluster/raft').glob('*.class'):put(overlay/path.relative_to(compiled),path.read_bytes())
 inventory=[dict(path=p.relative_to(overlay).as_posix(),sha256=h.sha(p)) for p in sorted(overlay.rglob('*.class'))];save(out/'overlay-manifest.json',inventory)
 binary_hash=h.sha(binary);driver_hash=h.sha(h.DRIVER)
 if driver_hash!='e2a3b2fc726a1ac64e998585db86b30fa8bf3f706195b78bb77c5f99bf877bd9':raise RuntimeError('JDBC fixed dependency changed')
 schema=(ROOT/'tools/go-java-v1-real-at/schema.sql').read_bytes()+(HERE/'schema.sql').read_bytes();put(runtime/'schema.sql',schema);put(out/'schema.sql',schema)
 if h.command(['docker','context','show']).stdout.strip()!=b'desktop-linux':raise RuntimeError('Wrong Docker context')
 password='seata_raft_fixture_only';dbname='seata_raft_'+uuid.uuid4().hex[:8];container=None;processes={};logs={};generations={i:0 for i in range(3)};stage_logs={};sdk=None;relay=None;phase=None;starts=[];sdk_pids=[];leader_pids=[];all_pids=[];success=False;proof=None;bundles={}
 try:
  data=runtime/'mysql';data.mkdir();container=h.command(['docker','create','--pull=never','--memory=512m','--cpus=1','--name','seata-'+runtime.name.lower(),'--label','seata-raft-context='+runtime.name,'--mount','type=bind,source='+data.as_posix()+',target=/var/lib/mysql','--mount','type=bind,source='+(runtime/'schema.sql').as_posix()+',target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-p','127.0.0.1:28093:3306','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname,h.IMAGES['db']]).stdout.decode().strip();h.command(['docker','start',container])
  def db_info():
   item=json.loads(h.command(['docker','inspect',container]).stdout)[0];mount=next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')
   if item['Image']!=h.IMAGES['db'] or item['Config']['Labels']['seata-raft-context']!=runtime.name or mount['Source'].replace('\\','/').lower()!=data.as_posix().lower():raise RuntimeError('Owned MySQL identity differs')
   return item
  def sql(query):return h.command(['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-h','127.0.0.1','-N','-B',dbname,'-e',query]).stdout
  def db_ready():
   try:sql('SELECT COUNT(*) FROM accounts');return True
   except RuntimeError:return False
  h.wait_until(db_ready,90,'Owned TCP MySQL ready');starts.append(db_info()['State']['StartedAt']);save(out/'container.json',db_info());put(out/'business-tables.txt',sql('SHOW TABLES'))
  def restart_db():
   h.command(['docker','restart',container],timeout=45);h.wait_until(db_ready,90,'Owned MySQL restarted');starts.append(db_info()['State']['StartedAt']);save(out/f'database-start-{len(starts)}.json',db_info())
  def start(index):
   work=runtime/f'node_{index}';work.mkdir(exist_ok=True)
   if not (work/'.phase0-owned-raft').exists():put(work/'.phase0-owned-raft',token+'\n')
   config=f'''server:
  port: {NATIVE[index]}
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
    service-port: {NATIVE[index]}
    enable-check-auth: false
    max-commit-retry-timeout: 5000
    enable-parallel-handle-branch: false
    rollback-retry-timeout-unlock-enable: false
    rollback-failed-unlock-enable: false
    retry-dead-threshold: 500
    recovery:
      committing-retry-period: 200
      async-committing-retry-period: 200
    raft:
      group: default
      server-addr: {','.join(HOST+':'+str(p) for p in RAFT)}
      sync: true
      snapshot-interval: 600
      election-timeout-ms: 3000
      reporter-enabled: false
      serialization: jackson
      compressor: none
  store:
    mode: raft
    session:
      mode: raft
    lock:
      mode: raft
    file:
      dir: {work.as_posix()}/store
  metrics:
    enabled: false
  transport:
    rpc-tc-request-timeout: 750
'''
   if not (work/'application.yml').exists():put(work/'application.yml',config)
   elif (work/'application.yml').read_bytes()!=config.encode():raise RuntimeError('Node configuration changed on restart')
   directory=out/f'node_{index}_generation_{generations[index]}';generations[index]+=1;directory.mkdir();put(directory/'application.yml',config);env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=work.as_posix()
   command=[h.JAVA,'-Xlog:class+load=info','-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+work.as_posix(),'-Dserver.raftPort='+str(RAFT[index]),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),'-Dseata.it.raft.dir='+work.as_posix(),'-Dseata.it.raft.controlPort='+str(CONTROL[index]),'-Dseata.it.raft.token='+token,'-Dloader.main=org.apache.seata.server.cluster.raft.RaftBusinessControl','-Dloader.path='+overlay.as_posix()+','+h.DRIVER.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1','-n',str(index+1)]
   command=[str(value) for value in command];command[command.index('-h')+1]=HOST;process=h.spawn(command,directory,'tc',work,env);processes[index]=process;logs[index]=directory;all_pids.append(process.pid);save(directory/'process.json',dict(pid=process.pid,index=index,command=command))
  def api(index,operation='/state',method='GET'):
   query=urllib.parse.urlencode(dict(resource=phase['resource_id'])) if phase else ''
   if operation.startswith('/dump?'):query=''
   url=f'http://127.0.0.1:{CONTROL[index]}'+operation+('?' + query if query else '');request=urllib.request.Request(url,method=method,headers={'X-Phase0-Owner':token})
   with OPENER.open(request,timeout=20) as response:value=json.load(response)
   if value.get('pid')!=processes[index].pid:raise RuntimeError('RAFT control process identity differs')
   return value
  def stable(name,live,final=False,verify=True,partial=False):
   latest=None;failure_reason=None
   def settled():
    nonlocal latest,failure_reason
    try:
     nodes={str(i):api(i) for i in live};leaders=[int(k) for k,v in nodes.items() if v['leader']]
     if len(leaders)!=1:
      latest=dict(nodes=nodes);failure_reason='Actual unique leader missing';return False
     index=leaders[0];checks.completion.verify_completion(nodes[str(index)]);confirm=api(index);checks.completion.verify_completion(confirm);latest=dict(leader=index,nodes=nodes,confirmation=confirm,processes={str(i):dict(pid=processes[i].pid,peer_id=HOST+':'+str(RAFT[i]),process_file=str(logs[i]/'process.json')) for i in live})
     if any(confirm[k]!=nodes[str(index)][k] for k in ['pid','term','node_id','leader_id','raft_peer_id','reload_complete_term']):return False
     checks.verify_stage(latest,phase,final,partial)
     return True
    except (OSError,RuntimeError,ValueError) as error:failure_reason=str(error);return False
   try:h.wait_until(settled,60,'Actual current-term RAFT '+name)
   except TimeoutError:
    save(out/('cluster-failed-'+name+'.json'),dict(stage=name,reason=failure_reason,last_observation=latest));raise
   save(out/('cluster-'+name+'.json'),latest);bundles[name]=latest;return latest['leader']
  def snapshot_cluster(name,live):
   for index in live:
    before=api(index);after=api(index,'/snapshot','POST');save(out/f'snapshot-{name}-node_{index}.json',dict(before=before,after=after));snapshot=after['snapshot'];directory=Path(snapshot['directory']);files=[]
    for path in sorted(directory.rglob('*')):
     if path.is_file():
      raw=path.read_bytes();relative=path.relative_to(directory).as_posix();put(out/f'snapshot-data-{name}-node_{index}'/relative,raw);files.append(dict(path=relative,bytes=len(raw),sha256=sha(raw)))
    save(out/f'snapshot-files-{name}-node_{index}.json',dict(node=index,pid=after['pid'],snapshot=snapshot,files=files))
  def resume(index,label,snapshot_stage):
   audit=json.loads((out/f'snapshot-files-{snapshot_stage}-node_{index}.json').read_bytes());directory=Path(audit['snapshot']['directory'])
   if directory.resolve().is_relative_to((runtime/f'node_{index}'/'store').resolve()) is not True:raise RuntimeError('Snapshot input is outside this owned node')
   actual=[dict(path=p.relative_to(directory).as_posix(),bytes=p.stat().st_size,sha256=h.sha(p)) for p in sorted(directory.rglob('*')) if p.is_file()]
   if actual!=audit['files']:raise RuntimeError('Restart snapshot bytes differ from preceding completed snapshot')
   save(out/('resume-'+label+'.json'),dict(node=index,snapshot_stage=snapshot_stage,snapshot=audit['snapshot'],files=actual,stopped_process=str(logs[index]/'process.json'),configuration_sha256=h.sha(runtime/f'node_{index}'/'application.yml')));start(index)
  def dump_reload(index,label,boundary):
   save(out/('dump-'+label+'.json'),api(index,'/dump?'+label,'POST'));path=runtime/f'node_{index}'/(label+'.jfr');put(out/(label+'.jfr'),path.read_bytes());raw=h.command([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Dfile.encoding=UTF-8','-cp',compiled,'ReadRaftExceptions',path]).stdout;put(out/(label+'-exceptions.json'),raw)
   found=checks.completion.reload_exceptions(json.loads(raw)['recording']['events'],16,boundary)
   if found:raise RuntimeError('Actual reload still throws after this election')
   save(out/('reload-'+label+'.json'),dict(node=index,pid=processes[index].pid,boundary=boundary,exceptions=found,jfr_sha256=h.sha(path),reader_sha256=h.sha(compiled/'ReadRaftExceptions.class')))
  def begin_sdk(name,mode,index):
   nonlocal sdk,relay
   live=[i for i,p in processes.items() if p.poll() is None];current=stable(name+'_sdk_start',live,partial=name=='operator_replay')
   if current!=index:raise RuntimeError('Selected leader changed before SDK start')
   state=bundles[name+'_sdk_start']['nodes'][str(index)]
   directory=out/name;directory.mkdir();work=runtime/name;work.mkdir();stage_logs[name]=logs[index];leader_pids.append(processes[index].pid);put(directory/'tc-process.json',(logs[index]/'process.json').read_bytes());put(directory/'application.yml',(logs[index]/'application.yml').read_bytes());save(directory/'route.json',dict(node=index,port=NATIVE[index],pid=processes[index].pid,peer=state['raft_peer_id'],term=state['term'],source_log=str(logs[index]/'tc-stdout.log')))
   relay=proxy.RaftCommitProxy(work,directory,mode,NATIVE[index]);relay.start();put(work/'seatago.yml',(ROOT/'tools/go-java-v1-real-at/config.yml').read_bytes());save(work/'database.json',dict(dsn='root:'+password+'@tcp(127.0.0.1:28093)/'+dbname+'?charset=utf8mb4&parseTime=true&loc=UTC',mode='prepare' if name=='initial' else 'serve'))
   sdk=h.spawn([binary,work],directory,'sdk',work,h.ENV);sdk_pids.append(sdk.pid);h.wait_until(lambda:(work/'rm-identified.ready').exists(),20,'Actual SDK RM registration',sdk);h.wait_until(lambda:(work/'sdk-serving.ready').exists() or name=='initial',20,'Actual initialized TCC service',sdk);return directory,work
  def stop_sdk():
   nonlocal sdk,relay
   if relay:relay.close();relay=None
   h.stop(sdk);sdk=None;h.wait_until(lambda:not h.port_open(28092),10,'SDK proxy released')
  def business_snapshot(name,index):
   leader_state=api(index);checks.completion.verify_completion(leader_state);save(out/('business-leader-'+name+'.json'),leader_state)
   put(out/('prepared-context-'+name+'.tsv'),sql('SELECT xid,branch_id,prepared_units,prepared_token FROM commit_reservation'));put(out/(name+'.tsv'),sql("SELECT 'business',id,balance FROM accounts; SELECT 'undo',xid,branch_id,log_status,HEX(rollback_info) FROM undo_log; SELECT 'commit_account',id,reserved,confirmed FROM commit_account; SELECT 'reservation',xid,branch_id,phase,effect_count FROM commit_reservation"));params=urllib.parse.urlencode(dict(xid=phase['xid'],pageNum=1,pageSize=20,withBranch='true',unit='default'))
   with OPENER.open(f'http://127.0.0.1:{NATIVE[index]}/api/v1/console/globalSession/query?'+params,timeout=10) as response:put(out/('console-'+name.replace('_','-')+'.json'),response.read())
   events=[]
   try:
    with socket.create_connection(('127.0.0.1',NATIVE[index]),timeout=5) as stream:value=observer.query(stream,phase['xid'],phase['resource_id'],'ACCOUNTS:1;',15 if name=='final' else 16,True,events)
    save(out/('native-'+name+'.json'),value)
   finally:save(out/('native-'+name+'-frames.json'),events)
   after=api(index);checks.completion.verify_completion(after)
   if any(after[key]!=leader_state[key] for key in ['pid','term','raft_peer_id']):raise RuntimeError('Leader changed during actual SQL/console/native observation')
   save(out/('business-leader-after-'+name+'.json'),after)
  def operator(directory,index):
   route=json.loads((directory/'route.json').read_bytes());before=api(index);checks.completion.verify_completion(before)
   if (before['pid'],before['term'],before['raft_peer_id'])!=(route['pid'],route['term'],route['peer']):raise RuntimeError('Actual operator leader differs from SDK route')
   save(directory/'operator-leader-before.json',before)
   relay.record(event='operator-command-start',xid=phase['xid']);params=urllib.parse.urlencode(dict(xid=phase['xid'],unit='default'));request=urllib.request.Request(f'http://127.0.0.1:{NATIVE[index]}/api/v1/console/globalSession/changeGlobalStatus?'+params,method='PUT')
   try:
    with OPENER.open(request,timeout=15) as response:value=dict(http_status=response.status,body=response.read().decode())
   except urllib.error.HTTPError as error:value=dict(http_status=error.code,body=error.read().decode())
   save(directory/'operator.json',value)
   after=api(index);checks.completion.verify_completion(after)
   if any(after[key]!=before[key] for key in ['pid','term','raft_peer_id']):raise RuntimeError('Leader changed during actual operator recovery')
   save(directory/'operator-leader-after.json',after)
  def stop_leader(index,label):
   before=api(index);checks.completion.verify_completion(before);boundary=datetime.datetime.now(datetime.timezone.utc).isoformat();h.stop(processes[index]);stopped=datetime.datetime.now(datetime.timezone.utc).isoformat()
   save(out/('leader-stop-'+label+'.json'),dict(node=index,pid=processes[index].pid,boundary=boundary,stopped_at=stopped,returncode=processes[index].poll(),before=before,process_file=str(logs[index]/'process.json')));return boundary
  for index in range(3):start(index)
  leader=stable('ready',[0,1,2],verify=False);directory,work=begin_sdk('initial','deny-commit',leader);h.wait_until(lambda:(work/'recovery.json').exists(),20,'Original real phase one',sdk);phase=json.loads((work/'recovery.json').read_bytes())
  wire=module('raft_business_wire',ROOT/'tools/go-java-v1-real-at/verify.py');resources={wire.decode(e)['resource'] for e in list(relay.events) if e.get('event')=='wire' and e.get('direction')=='sdk-to-tc' and e.get('payload_type')==11 and wire.decode(e)['branch_type']==0}
  if len(resources)!=1:raise RuntimeError('Unique original branch resource missing')
  phase['resource_id']=next(iter(resources));save(out/'phase-one.json',phase);leader=stable('timeout',[0,1,2]);business_snapshot('timeout',leader);h.wait_until(lambda:(work/'sdk-serving.ready').exists(),20,'Single original TM commit returned',sdk);snapshot_cluster('timeout',[0,1,2]);stop_sdk()
  old=leader;boundary=stop_leader(old,'election_one');live=[i for i in range(3) if i!=old];leader=stable('election_one',live);checks.verify_transition(bundles['timeout'],bundles['election_one']);dump_reload(leader,'election_one',boundary)
  restart_db();directory,work=begin_sdk('operator_rejected','pass',leader);business_snapshot('restart',leader);operator(directory,leader);h.wait_until(lambda:b'REAL_TCC_CONTEXT_REJECTED' in (directory/'sdk-stdout.log').read_bytes(),10,'Original callback rejected absent prepared context',sdk);business_snapshot('rejected',leader);stable('rejected',live);stop_sdk()
  resume(old,'first','timeout');stable('restored',[0,1,2])
  for process in processes.values():h.stop(process)
  for name,directory in stage_logs.items():put(out/name/'tc-stdout.log',(directory/'tc-stdout.log').read_bytes())
  save(out/'runtime.json',dict(tc_pids=leader_pids,sdk_pids=sdk_pids,database_starts=starts,all_tc_pids=all_pids,storage_mode='raft'));e=module('context_reference_evidence',HERE/'reference_evidence.py');proof=e.derive(out);save(out/'proof.json',proof)
  if a.run(cohort)!=audit or h.sha(jar)!=h.JAR_HASH or h.sha(binary)!=binary_hash or h.sha(h.DRIVER)!=driver_hash:raise RuntimeError('Original runtime artifacts changed')
  for path,frozen_bytes in source.items():
   if path.read_bytes()!=frozen_bytes:raise RuntimeError('Frozen run source changed: '+str(path))
  if deps.dependency_inventory(cp)!=dependencies or inventory!=[dict(path=p.relative_to(overlay).as_posix(),sha256=h.sha(p)) for p in sorted(overlay.rglob('*.class'))]:raise RuntimeError('Runtime dependency/overlay changed')
  if generated!=[dict(path=p.relative_to(compiled).as_posix(),sha256=h.sha(p)) for p in sorted(compiled.rglob('*.class'))]:raise RuntimeError('Actual diagnostic reader classes changed')
  a.sdk.c.audit_go();save(out/'final-input-check.json',dict(sources_unchanged=True,dependencies_unchanged=True,overlay_unchanged=True,original_runtime_unchanged=True));success=True
 finally:
  if relay:relay.close()
  h.stop(sdk)
  for process in processes.values():h.stop(process)
  state=None
  if container:db_info();h.command(['docker','stop','-t','10',container],timeout=30);state=db_info()['State']['Status']
  def listening(port):
   try:
    with socket.create_connection((HOST if port in RAFT else '127.0.0.1',port),timeout=.2):return True
   except OSError:return False
  h.wait_until(lambda:not any(listening(p) for p in PORTS),15,'Owned RAFT/SDK/MySQL ports released');save(out/'result.json',dict(succeeded=success,container=container,container_state=state,ports_released=True,runtime=str(runtime),proof=proof))
 if success:
  save(out/'summary.json',dict(succeeded=True,expected_defect_reproduced=True,recovery_certified=False,parent_evidence=str(parent),commit_parent=str(commit_parent),raft_parent=str(raft_parent),cohort=str(cohort),frozen_inputs=len(source),java_jar_sha256=h.sha(jar),binary_sha256=binary_hash,driver_sha256=driver_hash,election_timeout_ms=3000,scope='Reference original status-only RAFT replication loses actual updated Prepare context; real callback rejection and durable business non-confirmation observed. Not a recovery pass, not productionRust or completeWP00.'));print(json.dumps(dict(expected_defect_reproduced=True,evidence_directory=str(out))),flush=True)
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output-root',type=Path,default=ROOT/'docs/phase-0/evidence/local-go-java-raft-context-reference');run(parser.parse_args().output_root)
