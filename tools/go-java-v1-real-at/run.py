"""Own MySQL, original TC+certified overlay and full SDK: real normal/reconnect/persistent AT."""
import datetime,hashlib,importlib.util,json,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def case(label,out,runtime,binary,a,recorder,verifier,java,parent,overlay_audit):
 h=a.sdk.h;directory=out/label;directory.mkdir();work=runtime/label;work.mkdir();data=work/'mysql';data.mkdir();run=runtime.name+'-'+label
 for port in [28091,28092,28093]:
  if h.port_open(port):raise RuntimeError('Owned AT port already occupied')
 schema=work/'schema.sql';put(schema,(HERE/'schema.sql').read_bytes());password='seata_real_at_fixture_only';dbname='seata_at_'+uuid.uuid4().hex[:8];container=None;tc=None;relay=None;success=False;proof=None;programs=[]
 try:
  container=h.command(['docker','create','--pull=never','--name','seata-'+run.lower(),'--label','seata-real-at='+run,'--mount','type=bind,source='+data.as_posix()+',target=/var/lib/mysql','--mount','type=bind,source='+schema.as_posix()+',target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-p','127.0.0.1:28093:3306','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname,h.IMAGES['db']]).stdout.decode().strip();h.command(['docker','start',container])
  def sql(query):return h.command(['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-N','-B',dbname,'-e',query],check=False)
  h.wait_until(lambda:sql('SELECT COUNT(*) FROM accounts').returncode==0,90,'Owned AT MySQL startup')
  info=json.loads(h.command(['docker','inspect',container]).stdout)[0]
  if info['Image']!=h.IMAGES['db'] or info['Config']['Labels']['seata-real-at']!=run:raise RuntimeError('Owned MySQL image/label differs')
  mount=next(r for r in info['Mounts'] if r['Destination']=='/var/lib/mysql')
  if mount['Source'].replace('\\','/').lower()!=data.as_posix().lower():raise RuntimeError('Owned MySQL data escaped D: bind')
  put(directory/'container.json',json.dumps(dict(id=container,image=info['Image'],mounts=info['Mounts'],label=run),indent=2)+'\n')
  database=dict(dsn='root:'+password+'@tcp(127.0.0.1:28093)/'+dbname+'?charset=utf8mb4&parseTime=true&loc=UTC',reconnect_replay=label=='reconnect',recovery_mode='prepare' if label=='recovery' else '')
  put(work/'database.json',json.dumps(database)+'\n');put(work/'seatago.yml',(HERE/'config.yml').read_bytes());put(directory/'seatago.yml',(HERE/'config.yml').read_bytes())
  config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091');put(work/'application.yml',config);put(directory/'application.yml',config)
  env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=work.as_posix();jar=a.sdk.c.ORACLE/'server/target/seata-server.jar';overlay=parent/'overlay'
  if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR drift')
  put(directory/'overlay-before.json',json.dumps(overlay_audit.verify(parent),indent=2)+'\n')
  tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),'-verbose:class','-Dloader.path='+overlay.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],directory,'tc',work,env)
  h.wait_until(lambda:h.port_open(28091),90,'Owned AT TC startup',tc);relay=recorder.RealAtProxy(work,directory);relay.start()
  result=subprocess.run([str(binary),str(work)],cwd=work,env=env,capture_output=True,creationflags=h.FLAGS,timeout=120);put(directory/('prepare.log' if label=='recovery' else 'client.log'),result.stdout+result.stderr)
  if label=='recovery':h.wait_until(lambda:any(e['event']=='expected-phase-one-process-exit' for e in relay.events),3,'Explicit prepared SDK exit observation')
  relay.close();relay=None
  if result.returncode:raise RuntimeError('Real AT SDK process failed: '+label)
  if label=='recovery':
   if b'REAL_AT_PREPARED ' not in result.stdout:raise RuntimeError('Real phase-one process did not stop before global end')
   query='SELECT id,balance FROM accounts; SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log'
   snapshot=sql(query)
   if snapshot.returncode:raise RuntimeError('Independent pre-restart SQL failed')
   put(directory/'database-before-restart.tsv',snapshot.stdout);before=json.loads(h.command(['docker','inspect',container]).stdout)[0]['State']['StartedAt']
   h.command(['docker','restart',container],timeout=45);h.wait_until(lambda:sql('SELECT COUNT(*) FROM accounts').returncode==0,90,'Owned AT database restart')
   after=json.loads(h.command(['docker','inspect',container]).stdout)[0]['State']['StartedAt'];snapshot=sql(query)
   if snapshot.returncode:raise RuntimeError('Independent post-restart SQL failed')
   put(directory/'database-after-restart.tsv',snapshot.stdout)
   put(directory/'restart.json',json.dumps(dict(container=container,started_before=before,started_after=after,scope='SDK process and MySQL restart; original TC remains live'))+'\n')
   database['recovery_mode']='resume';(work/'database.json').write_text(json.dumps(database)+'\n',encoding='utf-8');(work/'phase-one-exit.ready').unlink()
   resumed=directory/'resumed';resumed.mkdir();relay=recorder.RealAtProxy(work,resumed);relay.start()
   result=subprocess.run([str(binary),str(work)],cwd=work,env=env,capture_output=True,creationflags=h.FLAGS,timeout=120);put(resumed/'client.log',result.stdout+result.stderr);relay.close();relay=None
   if result.returncode:raise RuntimeError('Actual restarted RM did not recover')
  final=sql('SELECT id,balance FROM accounts; SELECT xid,branch_id,log_status,LENGTH(rollback_info) FROM undo_log')
  if final.returncode:raise RuntimeError('Independent final SQL failed')
  put(directory/'database-final.tsv',final.stdout)
  proof=verifier.recovery(directory) if label=='recovery' else verifier.normal(directory,reconnect=label=='reconnect')
  origins=java.class_sources(directory/'tc-stdout.log',overlay,module('real_at_java_prepare',ROOT/'tools/java-preface-candidate/prepare_candidate.py'));put(directory/'class-origins.json',json.dumps(origins,indent=2)+'\n');put(directory/'overlay-after.json',json.dumps(overlay_audit.verify(parent),indent=2)+'\n');success=True
 finally:
  if relay:relay.close()
  h.stop(tc)
  if container:
   info=json.loads(h.command(['docker','inspect',container]).stdout)[0]
   if info['Config']['Labels']['seata-real-at']!=run:raise RuntimeError('Cannot stop non-owned MySQL')
   put(directory/'mysql.log',h.command(['docker','logs',container],check=False).stdout);h.command(['docker','stop','-t','10',container],timeout=30)
  h.wait_until(lambda:not any(h.port_open(p) for p in [28091,28092,28093]),10,'Owned AT resources cleanup')
  put(directory/'result.json',json.dumps(dict(succeeded=success,proof=proof,container=container,runtime=str(work),ports_released=not any(h.port_open(p) for p in [28091,28092,28093]),original_jar_sha256=h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')),indent=2)+'\n')
 return dict(case=label,succeeded=success,proof=proof)
def run():
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-0/evidence/local-go-java-real-at'/('run_'+stamp);out.mkdir(parents=True,exist_ok=False);runtime=Path('D:/codex-builds')/('rat_'+stamp);runtime.mkdir()
 source={p:p.read_bytes() for p in HERE.iterdir() if p.is_file() and p.suffix in ['.go','.py','.sql','.yml']};source[ROOT/'docs/phase-0/go-java-v1-real-at-plan.md']=(ROOT/'docs/phase-0/go-java-v1-real-at-plan.md').read_bytes()
 parent=Path(json.loads((ROOT/'docs/phase-0/evidence/local-go-java-inflight/latest_run.json').read_bytes())['evidence_directory']);previous=json.loads((parent/'summary.json').read_bytes());java_parent=Path(previous['inherited_java_evidence'])
 for path in [java_parent.parent/'manifest.json',java_parent/'overlay-manifest.json']+list((java_parent/'overlay').rglob('*.class')):source[path]=path.read_bytes()
 for row in json.loads((parent/'source-input-audit.json').read_bytes()):
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256']:raise RuntimeError('Inherited certified input changed: '+row['path'])
  source[path]=data
 rows=[]
 for path,data in sorted(source.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';put(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 put(out/'source-input-audit.json',json.dumps(rows,indent=2)+'\n')
 a=module('real_at_run_audit',ROOT/'tools/go-java-v1-inflight/audit.py');recorder=module('real_at_run_proxy',HERE/'proxy.py');verifier=module('real_at_run_verify',HERE/'verify.py');java=module('real_at_java_runtime',ROOT/'tools/java-preface-candidate/runtime.py');h=a.sdk.h;l=a.sdk.l;cohort=Path(previous['cohort']);audit=a.run(cohort)
 if h.command(['docker','context','show']).stdout.strip()!=b'desktop-linux':raise RuntimeError('Unexpected Docker context')
 h.command(['docker','image','inspect',h.IMAGES['db']]);put(out/'source-cohort-audit.json',json.dumps(audit,indent=2)+'\n')
 binary=out/'real-at.exe';l.run_command(['go','build','-o',binary,HERE/'main.go'],cohort,a.sdk.c.GO_ENV,out/'go-build.log',timeout=240)
 l.run_command(['go','version','-m',binary],cohort,a.sdk.c.GO_ENV,out/'go-build-info.txt')
 overlay_audit=module('real_at_overlay_audit',HERE/'overlay.py');overlay_audit.verify(java_parent)
 cases=[case(label,out,runtime,binary,a,recorder,verifier,java,java_parent,overlay_audit) for label in ['normal','reconnect','recovery']]
 env=h.ENV.copy();env.update(SEATA_REAL_AT_NORMAL=str(out/'normal'),SEATA_REAL_AT_RECONNECT=str(out/'reconnect'),SEATA_REAL_AT_RECOVERY=str(out/'recovery'))
 l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,env,out/'python-tests.log')
 if a.run(cohort)!=audit:raise RuntimeError('Go candidate changed during full SDK run')
 for path,data in source.items():
  if path.read_bytes()!=data:raise RuntimeError('Frozen input changed: '+str(path))
 a.sdk.c.audit_go()
 summary=dict(succeeded=True,completed_at=datetime.datetime.now().astimezone().isoformat(),parent_evidence=str(parent),parent_summary_sha256=h.sha(parent/'summary.json'),inherited_go_tests=254,inherited_rust_tests=117,inherited_java_tests=617,new_go_unit_execution=False,new_rust_execution=False,new_java_unit_execution=False,python_tests=16,cohort=str(cohort),binary_sha256=h.sha(binary),mysql_image=h.IMAGES['db'],cases=cases,scope='Actual full SDK, real MySQL normal AT/reconnect/duplicate and normal persistent replay. Not retry-timeout16/17 recovery, fault atomicity matrix, mixed-language resource safety, production Rust, full client or WP00 completion.')
 put(out/'summary.json',json.dumps(summary,indent=2)+'\n');(out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(evidence_directory=str(out),cases=[r['case'] for r in cases],python_tests=16)),flush=True)
if __name__=='__main__':run()
