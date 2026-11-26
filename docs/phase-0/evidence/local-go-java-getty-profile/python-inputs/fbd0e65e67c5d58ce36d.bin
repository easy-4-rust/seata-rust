"""Owned cached MySQL/Redis + unchanged pinned TC: seeded timeout restart and manual-retry evidence."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parents[3]
PIN='cba73e180fd8896a99df6415b859d275122feaa7'
JAR_HASH='aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5'
JAVA=Path('D:/Java/jdk-17.0.12/bin/java.exe')
MAVEN=Path('D:/Java/maven/apache-maven-3.9.9/bin/mvn.cmd')
SETTINGS=Path('D:/codex-builds/seata-phase09-live-20261111/maven-central-settings.xml')
DRIVER=Path('E:/Java/.m2/repository/com/mysql/mysql-connector-j/8.0.33/mysql-connector-j-8.0.33.jar')
IMAGES={'db':'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902',
        'redis':'sha256:8da19c7399dca900c6822a1cf30f9d2285585e2c4b457ff4095b0f4500054d3f'}
FLAGS=getattr(subprocess,'CREATE_NO_WINDOW',0)
ENV=os.environ.copy()
ENV.update(TEMP='D:/Temp',TMP='D:/Temp',LOCALAPPDATA='D:/Temp/seata-localappdata',JAVA_HOME='D:/Java/jdk-17.0.12',
           MAVEN_OPTS='-Djava.io.tmpdir=D:/Temp',CARGO_TARGET_DIR='D:/cargo-target/seata-phase09',PYTHONDONTWRITEBYTECODE='1')


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,data):
    with path.open('xb') as stream: stream.write(data.encode() if isinstance(data,str) else data)
def command(args,timeout=30,check=True):
    result=subprocess.run([str(x) for x in args],env=ENV,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout,creationflags=FLAGS)
    if check and result.returncode: raise RuntimeError(f'Command failed ({args[0]}), exit={result.returncode}: {result.stdout[-1500:]!r}')
    return result
def logged(args,directory,name,timeout=30):
    result=command(args,timeout,False);write(directory/name,result.stdout)
    if result.returncode: raise RuntimeError(f'{name} failed, exit={result.returncode}')
    return result.stdout.decode('utf-8-sig')
def spawn(args,directory,name,work,env=None):
    with (directory/(name+'-stdout.log')).open('xb') as out,(directory/(name+'-stderr.log')).open('xb') as err:
        return subprocess.Popen([str(x) for x in args],env=env or ENV,cwd=work,stdout=out,stderr=err,creationflags=FLAGS)
def stop(process):
    if process and process.poll() is None:
        process.kill();process.wait(timeout=10)
def quote_arg(value): return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def wait_until(test,timeout,label,process=None):
    deadline=time.monotonic()+timeout
    while True:
        if process and process.poll() is not None: raise RuntimeError(f'{label}: owned process exited {process.returncode}')
        if test(): return
        if time.monotonic()>=deadline: raise TimeoutError(label)
        time.sleep(.15)
def port_open(port):
    try:
        with socket.create_connection(('127.0.0.1',port),timeout=.2): return True
    except OSError:return False
def is_missing_session_rejection(status,raw,server_delta,xid):
    return (status==500 and raw==b''
            and ('manual operation to change the global session, xid: '+xid).encode() in server_delta
            and b'Caused by: java.lang.IllegalArgumentException: Global session is not exist, may be finished' in server_delta)

def contenders_for_cleanup(text,xid):
    rows=re.findall(r'BACKEND_LOCK targetXid=(\S+) contenderXid=(\S+) lockable=(?:true|false) expected=(?:true|false)',text)
    if len(rows)!=2 or any(target!=xid or contender==xid or not re.fullmatch(r'.+:28091:\d+',contender) for target,contender in rows):raise RuntimeError('Observer cleanup identity is not proved')
    contenders=[contender for _,contender in rows]
    if len(set(contenders))!=2:raise RuntimeError('Observer cleanup identities are not distinct')
    return contenders

def audit_build(oracle,build):
    rows=[]
    for entry in command(['git','-C',oracle,'ls-tree','-r','-z',PIN]).stdout.split(b'\0'):
        if not entry:continue
        header,path=entry.split(b'\t',1);relative=path.decode('utf-8')
        if '/src/main/' not in '/'+relative and relative!='pom.xml' and not relative.endswith('/pom.xml'):continue
        expected=header.split()[2].decode();current=(build/relative).read_bytes();normalized=current.replace(b'\r\n',b'\n')
        blob=lambda data:hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
        if blob(current)!=expected and blob(normalized)!=expected:
            pinned=command(['git','-C',oracle,'show',PIN+':'+relative]).stdout
            if normalized!=pinned.replace(b'\r\n',b'\n'):raise RuntimeError('Production source/build definition drift: '+relative)
        rows.append({'path':relative,'pinned_blob':expected,'raw_sha256':hashlib.sha256(current).hexdigest(),'lf_sha256':hashlib.sha256(normalized).hexdigest()})
    if len(rows)<1000:raise RuntimeError('Pinned production source closure incomplete')
    return rows
def api(path,directory,name,method='GET'):
    request=urllib.request.Request('http://127.0.0.1:28091/api/v1/console/globalSession/'+path,method=method)
    try:
        with urllib.request.urlopen(request,timeout=10) as response: status=response.status;raw=response.read()
    except urllib.error.HTTPError as error: status=error.code;raw=error.read()
    write(directory/(name+'.json'),json.dumps({'http_status':status,'body':raw.decode('utf-8',errors='replace')},indent=2)+'\n')
    try: body=json.loads(raw)
    except ValueError:body={}
    return status,body,raw
def query(xid,directory,name,expected,status=None,branches=None):
    code,body,_=api('query?'+urllib.parse.urlencode(dict(xid=xid,pageNum=1,pageSize=20,withBranch='true')),directory,name)
    if code!=200 or str(body.get('code'))!='200' or body.get('total')!=expected: raise RuntimeError(f'Console original-XID query differs: {name}')
    rows=body.get('data',[])
    if len(rows)!=expected: raise RuntimeError('Console row count differs')
    if expected:
        row=rows[0]
        if row.get('xid')!=xid or row.get('status')!=status or len(row.get('branchSessionVOs') or [])!=branches: raise RuntimeError('Console original status/branch fields differ')
    return body


def run(args):
    build=Path(args.java_build);oracle=Path(args.java_source);runtime_base=Path(args.runtime_base)
    if any(not re.match(r'^[DE]:[/\\]',str(p)) for p in [build,runtime_base]): raise ValueError('Development paths must remain on D/E')
    if command(['git','-C',oracle,'rev-parse','HEAD']).stdout.decode().strip()!=PIN: raise RuntimeError('Java pin mismatch')
    jar=oracle/'server/target/seata-server.jar'
    if sha(jar)!=JAR_HASH:raise RuntimeError('TC artifact mismatch')
    settings=json.loads((Path(os.environ['APPDATA'])/'Docker/settings-store.json').read_text(encoding='utf-8-sig'))
    if not re.match(r'^[DE]:',settings.get('DataFolder','')):raise RuntimeError('Docker data is not on D/E')
    if command(['docker','context','show']).stdout.strip()!=b'desktop-linux':raise RuntimeError('Unexpected Docker context')
    for image in IMAGES.values():command(['docker','image','inspect',image])
    for port in [28091,28093,28094]:
        with socket.socket() as listener:listener.bind(('127.0.0.1',port))
    run_id=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    runtime=runtime_base/run_id;runtime.mkdir(parents=True,exist_ok=False)
    evidence=ROOT/'docs/phase-0/evidence/local-backend-retry-timeout'/('run_'+run_id);evidence.mkdir(parents=True,exist_ok=False)
    test_root=build/'test-suite/test-new-version'
    source_before=audit_build(oracle,build)
    # Copy only these owned overlays, remove only their generated class leaves, then run fresh tests.
    for name in ['BackendRetryTimeoutStoreProbe','BackendRetryTimeoutProbeTest']:
        source=ROOT/f'tools/java-v1-golden/{name}.java';target=test_root/f'src/test/java/org/apache/seata/server/{name}.java'
        target.write_bytes(source.read_bytes())
        cls=test_root/f'target/test-classes/org/apache/seata/server/session/{name}.class'
        if cls.exists():cls.unlink()
    with (evidence/'java-tests.log').open('xb') as log:
        result=subprocess.run([str(MAVEN),'-B','-ntp','-s',str(SETTINGS),'-f',str(build/'pom.xml'),'-pl','test-suite/test-new-version','-am',
            '-Dtest=BackendRetryTimeoutProbeTest','-Dsurefire.failIfNoSpecifiedTests=false','-Djava.io.tmpdir=D:/Temp','test'],
            env=ENV,stdout=log,stderr=subprocess.STDOUT,creationflags=FLAGS,timeout=300)
    if result.returncode:raise RuntimeError('Fresh Java helper tests failed; no backend started')
    source_after=audit_build(oracle,build)
    write(evidence/'java-production-source-audit.json',json.dumps({'java_pin':PIN,'normalization':'CRLF to LF only','before_build':source_before,'after_build':source_after},indent=2)+'\n')
    report=test_root/'target/surefire-reports/TEST-org.apache.seata.server.session.BackendRetryTimeoutProbeTest.xml'
    suite=ET.parse(report).getroot()
    if int(suite.get('tests'))!=3 or any(int(suite.get(x,'0')) for x in ['failures','errors','skipped']):raise RuntimeError('Fresh test report differs')
    write(evidence/'java-report.xml',report.read_bytes())
    for name in ['BackendRetryTimeoutStoreProbe','BackendRetryTimeoutProbeTest']:
        if (ROOT/f'tools/java-v1-golden/{name}.java').read_bytes()!=(test_root/f'src/test/java/org/apache/seata/server/{name}.java').read_bytes():raise RuntimeError('Reconcile tested formatter output before live run')
    logged(['cargo','build','--manifest-path',ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'],evidence,'rust-build.log')
    probe=Path(ENV['CARGO_TARGET_DIR'])/'debug/seata-java-tc-it.exe'
    # Run the fixture with the original TC dependency cohort, not the test module's legacy compatibility jars.
    helper_classes=runtime/'fixture-classes'
    helper_class=helper_classes/'org/apache/seata/server/session/BackendRetryTimeoutStoreProbe.class'
    helper_class.parent.mkdir(parents=True)
    write(helper_class,(test_root/'target/test-classes/org/apache/seata/server/session/BackendRetryTimeoutStoreProbe.class').read_bytes())
    schema=oracle/'script/server/db/mysql.sql'
    if schema.read_bytes().replace(b'\r\n',b'\n')!=command(['git','-C',oracle,'show',PIN+':script/server/db/mysql.sql']).stdout.replace(b'\r\n',b'\n'):raise RuntimeError('Schema differs from pin')
    containers=[];tc=None;client=None;cases=[];success=False;cleanup_errors=[]
    try:
        for backend in ['db','redis']:
            work=runtime/backend;work.mkdir();directory=evidence/backend;directory.mkdir();data=work/'data';data.mkdir()
            dbname='seata_retry_'+run_id.lower();password='seata_retry_fixture_only'
            name='seata-rust-retry-'+backend+'-'+run_id.lower()
            creation=['docker','create','--pull=never','--name',name,'--label','seata-rust-retry='+run_id,'--mount',f'type=bind,source={data.as_posix()},target='+('/var/lib/mysql' if backend=='db' else '/data'),'-p',f'127.0.0.1:{28093 if backend=="db" else 28094}:'+('3306' if backend=='db' else '6379')]
            if backend=='db':creation+=['--mount',f'type=bind,source={schema.as_posix()},target=/docker-entrypoint-initdb.d/01-schema.sql,readonly','-e','MYSQL_ROOT_PASSWORD='+password,'-e','MYSQL_DATABASE='+dbname]
            creation+=[IMAGES[backend]]
            if backend=='redis':creation+=['redis-server','--appendonly','yes','--appendfsync','always']
            container=command(creation).stdout.decode().strip();containers.append(container)
            command(['docker','start',container])
            def db_command(sql):return ['docker','exec','-e','MYSQL_PWD='+password,container,'mysql','-uroot','-N','-B',dbname,'-e',sql]
            def ready():return command(db_command('SELECT COUNT(*) FROM global_table') if backend=='db' else ['docker','exec',container,'redis-cli','ping'],check=False).returncode==0
            wait_until(ready,90,'Owned backend readiness')
            before=json.loads(command(['docker','inspect',container]).stdout)[0]
            write(directory/'container-before.json',json.dumps({'id':container,'image':before['Image'],'pid':before['State']['Pid'],'started_at':before['State']['StartedAt'],'mounts':before['Mounts']},indent=2)+'\n')
            url=f'jdbc:mysql://127.0.0.1:28093/{dbname}?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC'
            storage={'datasource':'druid','dbType':'mysql','driverClassName':'com.mysql.cj.jdbc.Driver','url':url,'user':'root','password':password,'minConn':1,'maxConn':4} if backend=='db' else {'mode':'single','single':{'host':'127.0.0.1','port':28094},'database':0,'minConn':1,'maxConn':4,'type':'pipeline'}
            conf=work/'store.conf';write(conf,json.dumps({'store':{'mode':backend,'session':{'mode':backend},'lock':{'mode':backend},backend:storage}},indent=2)+'\n')
            registry=work/'registry.conf';write(registry,json.dumps({'config':{'type':'file','file':{'name':'file:'+conf.as_posix()}}},indent=2)+'\n')
            def fixture(mode):
                argfile=work/(mode+'.args')
                values=['-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dfile.listener.enabled=false','-Dseata.config.name=file:'+registry.as_posix(),'-Dseata.it.fixture.config='+conf.as_posix(),
                        '-Dloader.path='+helper_classes.as_posix()+','+DRIVER.as_posix(),'-Dloader.main=org.apache.seata.server.session.BackendRetryTimeoutStoreProbe',
                        '-cp',str(jar),'org.springframework.boot.loader.launch.PropertiesLauncher',mode,backend]
                write(argfile,'\n'.join(map(quote_arg,values))+'\n')
                return logged([JAVA,'@'+str(argfile)],directory,mode+'.log',30)
            seed=fixture('seed')
            profiles=re.findall(r'RECORD backend=(db|redis) index=(\d) xid=(\S+) status=(\d+) branches=(\d) branchId=(\d+) resource=(\S+) lockable=(true|false)',seed)
            if len(profiles)!=4:raise RuntimeError('No four actual persisted profiles')
            command(['docker','restart',container],timeout=40);wait_until(ready,60,'Owned backend restart readiness')
            after=json.loads(command(['docker','inspect',container]).stdout)[0]
            if before['State']['StartedAt']==after['State']['StartedAt'] or before['State']['Pid']==after['State']['Pid']:raise RuntimeError('Backend process restart not demonstrated')
            write(directory/'container-after.json',json.dumps({'id':container,'image':after['Image'],'pid':after['State']['Pid'],'started_at':after['State']['StartedAt']},indent=2)+'\n')
            inspected=fixture('inspect')
            if re.findall(r'RECORD .*',seed)!=re.findall(r'RECORD .*',inspected):raise RuntimeError('Restored backend fields/locks differ')
            yaml=f'''server:\n  port: 28091\nspring:\n  main:\n    web-application-type: none\nlogging:\n  file:\n    path: {work.as_posix()}/logs\nseata:\n  config:\n    type: file\n  registry:\n    type: file\n  server:\n    service-port: 28091\n    enable-check-auth: false\n  store:\n    mode: {backend}\n    session:\n      mode: {backend}\n    lock:\n      mode: {backend}\n'''
            if backend=='db':yaml+=f'    db:\n      datasource: druid\n      db-type: mysql\n      driver-class-name: com.mysql.cj.jdbc.Driver\n      url: {url}\n      user: root\n      password: {password}\n      min-conn: 1\n      max-conn: 4\n'
            else:yaml+='    redis:\n      mode: single\n      single:\n        host: 127.0.0.1\n        port: 28094\n      database: 0\n      type: pipeline\n'
            config=work/'application.yml';write(config,yaml);write(directory/'application.yml',yaml)
            tc=spawn([JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+config.as_posix(),'-Dloader.path='+str(DRIVER),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],directory,'tc',work)
            wait_until(lambda:port_open(28091),90,'Owned pinned TC readiness',tc)
            for _,index,xid,status,branches,branch_id,resource,_ in profiles:
                case_dir=directory/('case_'+index);case_dir.mkdir();status=int(status);branches=int(branches);branch_id=int(branch_id)
                query(xid,case_dir,'console-before',1,status,branches)
                readyfile=work/('ready_'+index);env=ENV.copy();env.update(SEATA_TC_HOST='127.0.0.1',SEATA_TC_PORT='28091',SEATA_IT_APP_ID='backend-retry-probe',SEATA_IT_HOLD_MS='0',SEATA_IT_RESOURCE_ID=resource,SEATA_IT_RETRY_XID=xid,SEATA_IT_RETRY_STATUS=str(status),SEATA_IT_RETRY_BRANCH_ID=str(branch_id),SEATA_IT_RETRY_READY=str(readyfile))
                client=spawn([probe,'--backend-retry-timeout'],case_dir,'client',work,env)
                def client_ready():
                    try:return readyfile.read_text(encoding='utf-8').strip()==str(client.pid)
                    except FileNotFoundError:return False
                wait_until(client_ready,10,'Native original-XID/lock observation with owned PID',client)
                http,answer,_=api('changeGlobalStatus?'+urllib.parse.urlencode({'xid':xid}),case_dir,'operator-first','PUT')
                if http!=200 or str(answer.get('code'))!='200':raise RuntimeError('Actual operator retry failed')
                if client.wait(timeout=15)!=0:raise RuntimeError('Native callback/final lock check failed')
                text=(case_dir/'client-stdout.log').read_text(encoding='utf-8')
                if f'PASS BACKEND_RETRY xid={xid} branchId={branch_id} initialStatus={status} observedStatus=15 actions={branches} lockReleased=true targetMutationCount=0' not in text:raise RuntimeError('No complete native evidence')
                query(xid,case_dir,'console-after',0)
                log_path=directory/'tc-stdout.log';log_offset=log_path.stat().st_size
                repeat_http,repeat,repeat_raw=api('changeGlobalStatus?'+urllib.parse.urlencode({'xid':xid}),case_dir,'operator-repeat','PUT')
                def rejected():return is_missing_session_rejection(repeat_http,repeat_raw,log_path.read_bytes()[log_offset:],xid)
                wait_until(rejected,5,'Exact missing-session rejection with current-XID server log',tc)
                write(case_dir/'operator-repeat-server.log',log_path.read_bytes()[log_offset:])
                query(xid,case_dir,'console-after-repeat',0)
                # DELAY_HANDLE_SESSION acknowledges Rollbacked11 but leaves ordinary DB/Redis records Rollbacking4.
                # Finish only our proven branch-empty observers through the TC's explicit operator retry API.
                observer_xids=contenders_for_cleanup(text,xid)
                for observer_index,observer in enumerate(observer_xids):
                    query(observer,case_dir,f'observer-{observer_index}-before',1,4,0)
                    deletion,deleted,_=api('sendCommitOrRollback?'+urllib.parse.urlencode({'xid':observer}),case_dir,f'observer-{observer_index}-finish','PUT')
                    if deletion!=200 or str(deleted.get('code'))!='200':raise RuntimeError('Deferred observer completion failed')
                    query(observer,case_dir,f'observer-{observer_index}-after',0)
                cases.append(dict(backend=backend,index=int(index),xid=xid,seeded_status=status,branch_id=branch_id,branches=branches,operator_code=answer.get('code'),repeat_http=repeat_http,repeat_code=repeat.get('code'),client_exit=client.returncode,simulated_actions=branches,original_mutations=0,observer_xids=observer_xids,observer_cleanup='initial Rollbacked11 RPC; verified persisted Rollbacking4/zero branches; explicit TC operator sendCommitOrRollback; then absent'))
                client=None
            if backend=='db':
                counts=logged(db_command('SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table)'),directory,'final-store-counts.txt').strip()
                if counts!='0\t0\t0':raise RuntimeError('Original DB sessions/branches/locks were not removed')
            else:
                for pattern in ['SEATA_GLOBAL_*','SEATA_BRANCH*']:
                    found=logged(['docker','exec',container,'redis-cli','--scan','--pattern',pattern],directory,'final-'+pattern.replace('*','all')+'.txt').strip()
                    if found:raise RuntimeError('Original Redis sessions/branches remain')
            stop(tc);tc=None
            logged(['docker','logs',container],directory,'backend.log')
            print('PASS backend restart/manual retry '+backend+' cases=4',flush=True)
        success=True
    finally:
        for process in [client,tc]:
            try:stop(process)
            except Exception as error:cleanup_errors.append(str(error))
        states=[]
        for container in containers:
            try:
                details=json.loads(command(['docker','inspect',container]).stdout)[0]
                if details['Config']['Labels'].get('seata-rust-retry')!=run_id:raise RuntimeError('Refusing to stop unowned container')
                command(['docker','stop','-t','10',container],timeout=30)
                details=json.loads(command(['docker','inspect',container]).stdout)[0]
                if details['State']['Running']:raise RuntimeError('Owned container remains running')
                states.append({'id':container,'state':details['State']['Status']})
            except Exception as error:cleanup_errors.append(str(error))
        result=dict(executed_at=datetime.datetime.now().astimezone().isoformat(),succeeded=success and not cleanup_errors,java_pin=PIN,jar_sha256=sha(jar),driver_sha256=sha(DRIVER),rust_sha256=sha(probe),helper_class_sha256=sha(helper_class),java_report_sha256=sha(evidence/'java-report.xml'),fixture_runtime='original pinned TC JAR dependency cohort plus one external helper class and existing JDBC jar',cases=cases,owned_containers=states,cleanup_errors=cleanup_errors,scope='Real cached stores/process restart and unchanged TC JAR with existing JDBC jar; injected statuses and simulated RM only, no SQL Undo compensation or durable RM replay proof')
        write(evidence/'result.json',json.dumps(result,indent=2)+'\n')
        pointer=evidence.parent/'latest_run.json';pointer.write_text(json.dumps({'evidence_directory':str(evidence),'runtime_directory':str(runtime),'succeeded':result['succeeded']},indent=2)+'\n',encoding='utf-8')
    if not result['succeeded']:raise RuntimeError('Backend recovery diagnostics incomplete')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--java-source',default='E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle');parser.add_argument('--java-build',default='D:/codex-builds/seata-java-exact-overlay-20261112');parser.add_argument('--runtime-base',default='D:/codex-builds/seata backend retry timeout');run(parser.parse_args())
