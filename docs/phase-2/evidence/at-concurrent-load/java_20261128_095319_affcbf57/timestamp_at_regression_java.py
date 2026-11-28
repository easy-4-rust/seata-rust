"""Owned original Java TC + formal Rust AT concurrent finite load."""
import argparse,datetime,json,os,socket,subprocess,sys,time,uuid,zipfile,shutil
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
sys.path.insert(0,str(ROOT/'tools/formal-tc-lock-coverage'))
import live_guard as guard

def port():
    with socket.socket() as probe:probe.bind(('127.0.0.1',0));return probe.getsockname()[1]

def run(versions, levels):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/at-concurrent-load'/('java_'+stamp);out.mkdir(parents=True);work=Path('D:/codex-builds')/('seata-at-load-'+stamp);(work/'data').mkdir(parents=True);print('AT_CONCURRENT_ORIGINAL_JAVA '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','RUSTC','RUSTDOC','SEATA_CONFIG_NAME','SEATA_ENV','SEATA_XA_JAVA_FAULT','SEATA_JAVA_MYSQL_TLS']:env.pop(key,None)
    commands=[]
    def command(label,argv,expected=0,input=None):
        value=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env,input=input,capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW);(out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr);commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-2000:].decode(errors='replace'))
        return value
    need(command('java-pin',['git','-C',guard.reg.ORACLE,'rev-parse','HEAD']).stdout.decode().strip()==guard.reg.PIN,'Original Java source pin differs');jar=guard.reg.ORACLE/'server/target/seata-server.jar';need(sha(jar)==guard.reg.JAR_SHA and sha(guard.DRIVER)==guard.DRIVER_SHA,'Original Java server/driver changed')
    owner=uuid.uuid4().hex;database='undo_load_'+owner[:12];db_port=port();container=None;processes=[];catalog_sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED)as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'tools/formal-rm-java/rm_proxy.py',Path(guard.support.__file__),Path(guard.reg.__file__),ROOT/'tools/formal-tc-lock-coverage/live.py',ROOT/'tools/formal-tc-lock-coverage/live_guard.py',ROOT/'tools/java-v1-golden/Phase0CredentialCheckAuthHandler.java',ROOT/'tools/formal-commit-direction/CallbackOwnershipAuthHandler.java']):name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());catalog_sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',catalog_sources)
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-at-load-'+stamp.replace('_','-'),'--label','seata.xa.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql','-p','127.0.0.1:'+str(db_port)+':3306','-e','MYSQL_ROOT_PASSWORD='+guard.PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,guard.IMAGE]).stdout.decode().strip();command('start',['docker','start',container]);mysql=['docker','exec','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e'];limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<limit,'Owned AT load MySQL startup timeout');time.sleep(.3)
        server=command('original-server-schema',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':script/server/db/mysql.sql']).stdout;(out/'original-server.sql').write_bytes(server);command('install-server-schema',['docker','exec','-i','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database],input=server)
        undo=command('original-undo-schema',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':script/client/at/db/mysql.sql']).stdout;(work/'undo.sql').write_bytes(undo);(out/'original-undo.sql').write_bytes(undo)
        resource=f'jdbc:mysql://127.0.0.1:{db_port}/{database}'
        overlay=guard.compile_auth(jar,work,out,env,False)
        for version in versions:
            env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','experimental_changed_commit','--test','live_at_load_java','--offline','--locked','--no-run','--message-format=json']);artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_at_load_java'];need(len(artifacts)==1,'Unique original Java XA client missing')
            for concurrency in levels:
                prefix=version+'-c'+str(concurrency);case_work=work/prefix;case_work.mkdir();case_out=out/prefix;case_out.mkdir();tc,rm,tm=port(),port(),port();need(len({tc,rm,tm,db_port})==4,'Owned ports overlap');stop=case_work/'stop';processes=[]
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
                        args=[sys.executable,'-B',ROOT/'tools/formal-rm-java/rm_proxy.py','--listen-port',listen,'--upstream-port',tc,'--events',case_out/(role+'-frames.jsonl'),'--stop-file',stop,'--role',role,'--mode','basic'];processes.append(guard.support.spawn(args,case_work,env,case_out,role));guard.support.wait_owned(processes[-1],listen,env)
                    command(prefix+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                    observations=case_out/'observations';observations.mkdir();extra=dict(SEATA_AT_LOAD_OUT=str(observations),SEATA_AT_LOAD_CONCURRENCY=str(concurrency),SEATA_UNDO_SCHEMA_PATH=str(work/'undo.sql'),SEATA_MYSQL_PORT=str(db_port),SEATA_MYSQL_PASSWORD=guard.PASSWORD,SEATA_MYSQL_DATABASE=database,SEATA_RM_ENDPOINT=f'127.0.0.1:{rm}',SEATA_TM_ENDPOINT=f'127.0.0.1:{tm}')
                    value=subprocess.run([str(artifacts[0]),'original_java_tc_at_concurrent_load','--ignored','--exact','--nocapture'],cwd=ROOT,env=env|extra,capture_output=True,timeout=210,creationflags=subprocess.CREATE_NO_WINDOW);(case_out/'client.stdout').write_bytes(value.stdout);(case_out/'client.stderr').write_bytes(value.stderr);save(case_out/'client-command.json',dict(path=str(artifacts[0]),sha256=sha(artifacts[0]),exit_code=value.returncode,argv=[str(artifacts[0]),'original_java_tc_at_concurrent_load','--ignored','--exact','--nocapture']));need(value.returncode==0,'Original Java AT load case failed: '+(value.stdout+value.stderr)[-2200:].decode(errors='replace'));need(b'test result: ok. 1 passed;' in value.stdout,'Original Java AT load case did not execute')
                    command(prefix+'-independent-state',mysql+['SELECT HEX(xid),branch_id,phase,terminal,commit_authorized,commit_terminal,first_phase_committed,HEX(undo_sha256) FROM '+database+'.seata_rs_at_branch_v1 ORDER BY branch_id']);command(prefix+'-independent-business',mysql+['SELECT id,v FROM '+database+'.undo_work ORDER BY id']);command(prefix+'-independent-undo',mysql+['SELECT HEX(xid),branch_id,log_status FROM '+database+'.undo_log ORDER BY branch_id']);command(prefix+'-independent-tc',mysql+['SELECT (SELECT COUNT(*) FROM '+database+'.global_table),(SELECT COUNT(*) FROM '+database+'.branch_table),(SELECT COUNT(*) FROM '+database+'.lock_table)']);command(prefix+'-native-commands',mysql+["SELECT event_time,thread_id,argument FROM mysql.general_log WHERE command_type='Query' AND (argument LIKE 'UPDATE undo_work%' OR argument LIKE 'INSERT INTO undo_work%' OR argument LIKE 'DELETE FROM undo_work%') ORDER BY event_time,thread_id"])

                finally:
                    stop.write_text('stop\n',encoding='utf-8')
                    for process in processes[1:]:
                        try:process.wait(timeout=10)
                        except subprocess.TimeoutExpired:guard.support.stop(process)
                    for process in reversed(processes):guard.support.stop(process)
                    if (case_work/'class-load.log').is_file():shutil.copyfile(case_work/'class-load.log',case_out/'class-load.log')
                    save(case_out/'process-cleanup.json',dict(stopped=all(p.poll() is not None for p in processes),pids=[p.pid for p in processes]));processes=[]
    finally:
        for process in reversed(processes):guard.support.stop(process)
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==guard.IMAGE and state['Config']['Labels'].get('seata.xa.owner')==owner,'Owned AT load identity changed');command('mysql-log',['docker','logs',container]);command('stop',['docker','stop','--time','10',container]);command('inspect-stopped',['docker','inspect',container]);command('remove',['docker','rm',container]);need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned AT load container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),work=str(work)))
    need(sha(jar)==guard.reg.JAR_SHA and sha(guard.DRIVER)==guard.DRIVER_SHA,'Original Java runtime drift')
    for row in catalog_sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed original Java AT input drift')
    save(out/'result.json',dict(status='ORIGINAL_JAVA_TC_AT_CONCURRENT_LOAD_VERIFIED',Rust_versions=versions,Java_pin=guard.reg.PIN,Java_jar_sha256=guard.reg.JAR_SHA,concurrency_levels=levels,rounds_per_level=8,actual_global_transactions=8*sum(levels)*len(versions),production_capacity_certified=False,whole_plan_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--concurrency',type=int,choices=[1,4,8]);args=parser.parse_args();run([args.rust] if args.rust else ['1.99','1.85'],[args.concurrency] if args.concurrency else [1,4,8])
