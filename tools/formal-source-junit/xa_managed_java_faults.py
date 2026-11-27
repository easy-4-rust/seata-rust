"""Owned pinned original Java TC + managed Rust XA registration and normal two-phase end."""
import itertools
import argparse,datetime,json,os,socket,subprocess,sys,time,uuid,zipfile,shutil
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
sys.path.insert(0,str(ROOT/'tools/formal-tc-lock-coverage'))
import live_guard as guard

def port():
    with socket.socket() as probe:probe.bind(('127.0.0.1',0));return probe.getsockname()[1]

def run(versions,modes):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-4/evidence/xa-managed'/('java-fault_'+stamp);out.mkdir(parents=True);work=Path('D:/codex-builds')/('seata-xa-java-fault-'+stamp);(work/'data').mkdir(parents=True);print('XA_MANAGED_ORIGINAL_JAVA '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','RUSTC','RUSTDOC','SEATA_CONFIG_NAME','SEATA_ENV','SEATA_XA_JAVA_FAULT']:env.pop(key,None)
    commands=[]
    def command(label,argv,expected=0,input=None):
        value=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env,input=input,capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW);(out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr);commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-2000:].decode(errors='replace'))
        return value
    need(command('java-pin',['git','-C',guard.reg.ORACLE,'rev-parse','HEAD']).stdout.decode().strip()==guard.reg.PIN,'Original Java source pin differs');jar=guard.reg.ORACLE/'server/target/seata-server.jar';need(sha(jar)==guard.reg.JAR_SHA and sha(guard.DRIVER)==guard.DRIVER_SHA,'Original Java server/driver changed')
    owner=uuid.uuid4().hex;database='xa_java_'+owner[:12];db_port=port();container=None;processes=[];catalog_sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED)as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__)]):name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());catalog_sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',catalog_sources)
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-xa-java-'+stamp.replace('_','-'),'--label','seata.xa.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql','-p','127.0.0.1:'+str(db_port)+':3306','-e','MYSQL_ROOT_PASSWORD='+guard.PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,guard.IMAGE]).stdout.decode().strip();command('start',['docker','start',container]);mysql=['docker','exec','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e'];limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<limit,'Owned XA Java MySQL startup timeout');time.sleep(.3)
        server=command('original-server-schema',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':script/server/db/mysql.sql']).stdout;(out/'original-server.sql').write_bytes(server);command('install-server-schema',['docker','exec','-i','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database],input=server)
        resource=f'jdbc:mysql://127.0.0.1:{db_port}/{database}'
        (out/'original-DefaultCore.java').write_bytes(command('original-terminal-source',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java']).stdout)
        overlay=guard.compile_auth(jar,work,out,env,False)
        for version in versions:
            env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','xa','--test','live_xa_java','--offline','--locked','--no-run','--message-format=json']);artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_xa_java'];need(len(artifacts)==1,'Unique original Java XA client missing')
            for mode,direction in itertools.product(modes,['commit','rollback']):
                prefix=version+'-'+mode+'-'+direction;case_work=work/prefix;case_work.mkdir();case_out=out/prefix;case_out.mkdir();tc,rm,tm=port(),port(),port();need(len({tc,rm,tm,db_port})==4,'Owned ports overlap');stop=case_work/'stop';processes=[]
                config=f'''spring:
  main:
    web-application-type: none
logging:
  file:
    path: {case_work.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: {tc}
    enable-check-auth: true
    retry-dead-threshold: 1000
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
      url: {resource}?useSSL=false&allowPublicKeyRetrieval=true
      user: root
      password: {guard.PASSWORD}
      min-conn: 1
      max-conn: 4
'''
                (case_work/'application.yml').write_text(config,encoding='utf-8');(case_out/'application.yml').write_text(config,encoding='utf-8')
                try:
                    args=[guard.reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(case_work),'-Dspring.config.location=file:'+(case_work/'application.yml').as_posix(),'-Dseata.callback.fixture.resource='+resource,'-Dloader.path='+overlay.as_posix()+','+guard.DRIVER.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
                    processes.append(guard.support.spawn(args,case_work,env,case_out,'tc'));guard.support.wait_owned(processes[-1],tc,env)
                    for role,listen in [('RM',rm),('TM',tm)]:
                        args=[sys.executable,'-B',ROOT/'tools/formal-rm-java/rm_proxy.py','--listen-port',listen,'--upstream-port',tc,'--events',case_out/(role+'-frames.jsonl'),'--stop-file',stop,'--role',role,'--mode',mode if role=='RM' and mode in ['duplicate','reconnect'] else 'basic'];processes.append(guard.support.spawn(args,case_work,env,case_out,role));guard.support.wait_owned(processes[-1],listen,env)
                    command(prefix+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                    observations=case_out/'observations';observations.mkdir();extra=dict(SEATA_XA_JAVA_FAULT=mode,SEATA_XA_OUT=str(observations),SEATA_MYSQL_PORT=str(db_port),SEATA_MYSQL_PASSWORD=guard.PASSWORD,SEATA_MYSQL_DATABASE=database,SEATA_RM_ENDPOINT=f'127.0.0.1:{rm}',SEATA_TM_ENDPOINT=f'127.0.0.1:{tm}')
                    client_args=[str(artifacts[0]),'original_java_tc_'+('commits' if direction=='commit' else 'rolls_back')+'_managed_xa_original_branch','--ignored','--exact','--nocapture']
                    if mode in ['restart','crash']:
                        client=guard.support.spawn(client_args,ROOT,env|extra,case_out,'database-client');processes.append(client)
                        checkpoint=observations/'proof/before-database-restart.json';limit=time.monotonic()+30
                        while not (observations/'proof/database-checkpoint-ready').exists():
                            need(client.poll() is None and time.monotonic()<limit,'Client did not pause at original Prepared checkpoint');time.sleep(.05)
                        identity=json.loads(checkpoint.read_bytes());need(identity['rust_process_id']==client.pid and identity['mode']==mode,'Prepared checkpoint not owned client')
                        state=json.loads(command(prefix+'-owned-before-restart',['docker','inspect',container]).stdout)[0]
                        need(state['Id']==container and state['Image']==guard.IMAGE and state['Config']['Labels'].get('seata.xa.owner')==owner and state['State']['Running'],'Restart target not owned fixture')
                        before=command(prefix+'-prepared-before-restart',mysql+['XA RECOVER CONVERT XID']).stdout
                        need(len(before.strip().splitlines())==1,'Original Prepared missing before database failure')
                        uuid_before=command(prefix+'-uuid-before-restart',mysql+['SELECT @@server_uuid']).stdout
                        command(prefix+'-native-before-restart',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type='Query' AND argument LIKE 'XA %' ORDER BY event_time,thread_id"])
                        command(prefix+'-tc-before-restart',mysql+['SELECT xid,branch_id FROM '+database+'.branch_table; SELECT xid FROM '+database+'.global_table'])
                        command(prefix+'-stop-for-restart',['docker','kill','--signal','KILL',container] if mode=='crash' else ['docker','stop','--time','20',container])
                        stopped=json.loads(command(prefix+'-owned-stopped',['docker','inspect',container]).stdout)[0]
                        need(not stopped['State']['Running'] and (mode!='crash' or stopped['State']['ExitCode']==137),'Owned database did not stop in requested mode')
                        command(prefix+'-start-for-restart',['docker','start',container]);limit=time.monotonic()+120
                        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:
                            need(time.monotonic()<limit,'Owned XA database restart timeout');time.sleep(.3)
                        after=command(prefix+'-prepared-after-restart',mysql+['XA RECOVER CONVERT XID']).stdout
                        need(before==after and uuid_before==command(prefix+'-uuid-after-restart',mysql+['SELECT @@server_uuid']).stdout,'Original Prepared/instance changed after restart')
                        command(prefix+'-tc-after-restart',mysql+['SELECT xid,branch_id FROM '+database+'.branch_table; SELECT xid FROM '+database+'.global_table'])
                        command(prefix+'-reenable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                        (observations/'proof/database-resumed').write_bytes(b'resume-original-branch\n')
                        try:client.wait(timeout=90)
                        except subprocess.TimeoutExpired:guard.support.stop(client);raise
                        value=subprocess.CompletedProcess(client_args,client.returncode,(case_out/'database-client-stdout.log').read_bytes(),(case_out/'database-client-stderr.log').read_bytes())
                    else:value=subprocess.run(client_args,cwd=ROOT,env=env|extra,capture_output=True,timeout=90,creationflags=subprocess.CREATE_NO_WINDOW)
                    (case_out/'client.stdout').write_bytes(value.stdout);(case_out/'client.stderr').write_bytes(value.stderr);save(case_out/'client-command.json',dict(path=str(artifacts[0]),sha256=sha(artifacts[0]),exit_code=value.returncode));need(value.returncode==0,'Original Java XA case failed: '+(value.stdout+value.stderr)[-2200:].decode(errors='replace'));need(b'test result: ok. 1 passed;' in value.stdout,'Original Java XA case did not execute')
                    proof=json.loads((observations/'proof/original-java-xa-fault.json').read_bytes());xid=proof['xid'];branch=proof['branch_id']
                    success=(('Committing global transaction is successfully done, xid = '+xid+'.') if direction=='commit' else ('Rollback global transaction successfully, xid = '+xid+'.')).encode()
                    branch_success=(('Commit branch transaction successfully, xid = ' if direction=='commit' else 'Rollback branch transaction successfully, xid = ')+xid+' branchId = '+branch).encode()
                    limit=time.monotonic()+5
                    while success not in (case_out/'tc-stdout.log').read_bytes() or branch_success not in (case_out/'tc-stdout.log').read_bytes():
                        need(time.monotonic()<limit,'Original TC terminal log missing; Finished cannot prove outcome');time.sleep(.05)
                    save(case_out/'original-tc-terminal.json',dict(xid=xid,branch_id=branch,direction=direction,original_branch_success_log=branch_success.decode(),original_global_success_log=success.decode(),Finished_used_as_success=False))
                    command(prefix+'-independent-state',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_branch_v1']);command(prefix+'-independent-business',mysql+['SELECT id,v FROM '+database+'.xa_managed ORDER BY id']);command(prefix+'-independent-prepared',mysql+['XA RECOVER CONVERT XID']);command(prefix+'-independent-tc',mysql+['SELECT (SELECT COUNT(*) FROM '+database+'.global_table),(SELECT COUNT(*) FROM '+database+'.branch_table),(SELECT COUNT(*) FROM '+database+'.lock_table)']);command(prefix+'-native-commands',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type='Query' AND argument LIKE 'XA %' ORDER BY event_time,thread_id"])
                finally:
                    stop.write_text('stop\n',encoding='utf-8')
                    for process in processes[1:]:
                        try:process.wait(timeout=10)
                        except subprocess.TimeoutExpired:guard.support.stop(process)
                    for process in reversed(processes):guard.support.stop(process)
                    save(case_out/'process-cleanup.json',dict(stopped=all(p.poll() is not None for p in processes),pids=[p.pid for p in processes]));processes=[]
                    for role in ['RM','TM']:
                        summaries=[json.loads(line) for line in (case_out/(role+'-frames.jsonl')).read_bytes().splitlines() if json.loads(line).get('event')=='summary'];need(len(summaries)==1 and summaries[0]['succeeded'],'Original Java fault observer incomplete')
    finally:
        for process in reversed(processes):guard.support.stop(process)
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==guard.IMAGE and state['Config']['Labels'].get('seata.xa.owner')==owner,'Owned XA Java identity changed');command('mysql-log',['docker','logs',container]);command('stop',['docker','stop','--time','10',container]);command('remove',['docker','rm',container]);need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned XA Java container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),work=str(work)))
    for row in catalog_sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed original Java XA input drift')
    save(out/'result.json',dict(status='ORIGINAL_JAVA_TC_MANAGED_XA_CALLBACK_FAULTS_VERIFIED',Rust_versions=versions,modes=modes,Java_pin=guard.reg.PIN,Java_jar_sha256=guard.reg.JAR_SHA,XA_full_fault_gate_complete=False,whole_plan_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--mode',choices=['duplicate','reconnect','restart','crash']);parser.add_argument('--database',action='store_true');args=parser.parse_args();run([args.rust] if args.rust else ['1.99','1.85'],[args.mode] if args.mode else ['restart','crash'] if args.database else ['duplicate','reconnect'])
