"""Owned pinned original Java TC + managed Rust TCC registration and normal two-phase end."""
import argparse,datetime,json,os,socket,subprocess,sys,time,uuid,zipfile,shutil
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
sys.path.insert(0,str(ROOT/'tools/formal-tc-lock-coverage'))
import live_guard as guard
import owned_writer_observation as observer

def port():
    with socket.socket() as probe:probe.bind(('127.0.0.1',0));return probe.getsockname()[1]

def run(versions,modes,db_modes,directions):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-4/evidence/tcc-managed'/('java-joint_'+stamp);out.mkdir(parents=True);work=Path('D:/codex-builds')/('seata-tcc-java-'+stamp);(work/'data').mkdir(parents=True);print('TCC_ORIGINAL_JAVA '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ['JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','RUSTC','RUSTDOC','SEATA_CONFIG_NAME','SEATA_ENV','SEATA_XA_JAVA_FAULT','SEATA_TCC_JAVA_FAULT','SEATA_TCC_DYNAMIC','SEATA_TCC_DYNAMIC_HOLD']:env.pop(key,None)
    commands=[]
    def command(label,argv,expected=0,input=None):
        value=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env,input=input,capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW);(out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr);commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-2000:].decode(errors='replace'))
        return value
    need(command('java-pin',['git','-C',guard.reg.ORACLE,'rev-parse','HEAD']).stdout.decode().strip()==guard.reg.PIN,'Original Java source pin differs');jar=guard.reg.ORACLE/'server/target/seata-server.jar';need(sha(jar)==guard.reg.JAR_SHA and sha(guard.DRIVER)==guard.DRIVER_SHA,'Original Java server/driver changed')
    owner=uuid.uuid4().hex;database='tcc_java_'+owner[:12];db_port=port();container=None;processes=[];catalog_sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED)as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'tools/formal-rm-java/rm_proxy.py',ROOT/'tools/formal-rm-java/tcc_joint_proxy.py',Path(observer.__file__)]):name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());catalog_sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',catalog_sources)
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-tcc-java-'+stamp.replace('_','-'),'--label','seata.tcc.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql','-p','127.0.0.1:'+str(db_port)+':3306','-e','MYSQL_ROOT_PASSWORD='+guard.PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,guard.IMAGE]).stdout.decode().strip();command('start',['docker','start',container]);mysql=['docker','exec','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e'];limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<limit,'Owned XA Java MySQL startup timeout');time.sleep(.3)
        server=command('original-server-schema',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':script/server/db/mysql.sql']).stdout;(out/'original-server.sql').write_bytes(server);command('install-server-schema',['docker','exec','-i','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database],input=server)
        (out/'original-DefaultCore.java').write_bytes(command('original-terminal-source',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java']).stdout)
        resource=f'jdbc:mysql://127.0.0.1:{db_port}/{database}'
        overlay=guard.compile_auth(jar,work,out,env,False)
        for version in versions:
            env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','tcc','--test','live_tcc_java','--offline','--locked','--no-run','--message-format=json']);artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_tcc_java'];need(len(artifacts)==1,'Unique original Java TCC client missing')
            for mode,db_mode,direction in __import__('itertools').product(modes,db_modes,directions):
                prefix=version+'-'+mode+'-'+db_mode+'-'+direction;case_work=work/prefix;case_work.mkdir();case_out=out/prefix;case_out.mkdir();tc,rm,tm=port(),port(),port();need(len({tc,rm,tm,db_port})==4,'Owned ports overlap');stop=case_work/'stop';processes=[];cut=case_work/'held-callback.json';exit_authorization=case_work/'joint-kill.json'
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
                    args=[guard.reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(case_work),'-Dspring.config.location=file:'+(case_work/'application.yml').as_posix(),'-Dseata.callback.fixture.resource=tcc-original-action','-Dloader.path='+overlay.as_posix()+','+guard.DRIVER.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
                    processes.append(guard.support.spawn(args,case_work,env,case_out,'tc'));guard.support.wait_owned(processes[-1],tc,env)
                    original_tc=processes[0]
                    for role,listen in [('RM',rm),('TM',tm)]:
                        proxy=ROOT/'tools/formal-rm-java'/('tcc_joint_proxy.py' if role=='RM' else 'rm_proxy.py')
                        argv=[sys.executable,'-B',proxy,'--listen-port',listen,'--upstream-port',tc,'--events',case_out/(role+'-before-frames.jsonl'),'--stop-file',stop,'--expected-peer-exit-file',exit_authorization]
                        if role=='RM':argv+=['--cut-file',cut]
                        else:argv+=['--role','TM','--mode','basic']
                        processes.append(guard.support.spawn(argv,case_work,env,case_out,role+'-before'));guard.support.wait_owned(processes[-1],listen,env)
                    command(prefix+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                    before=case_out/'writer-observations';before.mkdir();extra=dict(SEATA_TCC_JAVA_FAULT='joint_crash',SEATA_TCC_DYNAMIC=mode,SEATA_TCC_DYNAMIC_HOLD='1',SEATA_XA_OUT=str(before),SEATA_MYSQL_PORT=str(db_port),SEATA_MYSQL_PASSWORD=guard.PASSWORD,SEATA_MYSQL_DATABASE=database,SEATA_RM_ENDPOINT=f'127.0.0.1:{rm}',SEATA_TM_ENDPOINT=f'127.0.0.1:{tm}')
                    writer_args=[artifacts[0],'original_java_tc_'+('commits' if direction=='commit' else 'rolls_back')+'_original_tcc_branch','--ignored','--exact','--nocapture']
                    writer=guard.support.spawn(writer_args,ROOT,env|extra,case_out,'writer');processes.append(writer)
                    checkpoint=before/'proof/dynamic-before-end.json';deadline=time.monotonic()+40
                    while not checkpoint.exists():need(writer.poll() is None and original_tc.poll() is None and time.monotonic()<deadline,'Original writer/report checkpoint missing');time.sleep(.02)
                    proof=json.loads(checkpoint.read_bytes());need(proof['rust_process_id']==writer.pid,'Checkpoint not from original owned writer')
                    (before/'control').mkdir(exist_ok=True);(before/'control/allow-end').write_bytes(b'allow')
                    deadline=time.monotonic()+15
                    while not cut.exists():need(writer.poll() is None and original_tc.poll() is None and time.monotonic()<deadline,'Original TC callback cut not observed');time.sleep(.01)
                    shutil.copyfile(cut,case_out/'held-callback.json');held=json.loads(cut.read_bytes());need(held['xid']==proof['xid'] and held['branch_id']==proof['branch_id'] and held['kind']==(3 if direction=='commit' else 5),'Held callback changed original decision/branch')
                    state_sql='SELECT @@server_uuid; SELECT HEX(xid),status FROM '+database+'.global_table ORDER BY xid; SELECT HEX(xid),branch_id,branch_type,status,HEX(application_data) FROM '+database+'.branch_table ORDER BY branch_id; SELECT HEX(xid),branch_id,HEX(resource_id),HEX(application_data),HEX(server_uuid),schema_version,status FROM '+database+'.seata_rs_tcc_fence_v1; SELECT id,v FROM '+database+'.tcc_managed ORDER BY id'
                    context_sql='SELECT HEX(xid),branch_id,revision,HEX(resource_id),HEX(registration_data),HEX(application_data),HEX(server_uuid),schema_version,HEX(checksum) FROM '+database+'.seata_rs_tcc_context_v1 ORDER BY revision'
                    dml_sql="SELECT command_type,argument FROM mysql.general_log WHERE command_type IN ('Query','Execute') AND (argument LIKE 'INSERT INTO tcc_managed%' OR argument LIKE 'UPDATE tcc_managed%' OR argument LIKE 'DELETE FROM tcc_managed%') ORDER BY event_time,thread_id"
                    at_cut=command(prefix+'-state-cut',mysql+[state_sql]).stdout
                    command(prefix+'-context-cut',mysql+[context_sql])
                    decision=at_cut.splitlines()[1].split(b'\t');need(decision==[proof['xid'].encode().hex().upper().encode(),b'2' if direction=='commit' else b'4'],'Original TC decision was not durably stored before kill')
                    def kill_owned(process,label):
                        handle=observer.open_writer(process.pid)
                        try:
                            alive=observer.state(handle);need(alive==dict(alive=True,exit_code=259),'Original owned process already exited')
                            process.kill();process.wait(timeout=10);exited=observer.state(handle)
                            need(not exited['alive'] and exited['exit_code']!=0,'Original process did not forcibly exit')
                            save(case_out/(label+'-kill.json'),dict(pid=process.pid,before=alive,after=exited,monotonic_ns=time.perf_counter_ns()))
                        finally:observer.close(handle)
                    save(exit_authorization,dict(phase='joint-kill',writer_pid=writer.pid,tc_pid=original_tc.pid,monotonic_ns=time.perf_counter_ns()));shutil.copyfile(exit_authorization,case_out/'joint-kill.json')
                    kill_owned(writer,'writer');kill_owned(original_tc,'tc')
                    old_journal=before/'java-journal/branches.log';need(old_journal.read_bytes()==b'SRMJNL01','Undelivered callback changed original journal');shutil.copyfile(old_journal,case_out/'journal-at-cut.bin')
                    stop.write_bytes(b'stop')
                    for process in processes[1:3]:process.wait(timeout=10);need(process.returncode==0,'Original observer failed cleanup')
                    before_fault=command(prefix+'-state-before-database',mysql+[state_sql]).stdout;need(before_fault==at_cut,'Killed owners changed durable state')
                    command(prefix+'-business-dml-before',mysql+[dml_sql])
                    if db_mode!='none':
                        command(prefix+'-database-'+('kill' if db_mode=='crash' else 'stop'),['docker','kill','--signal','KILL',container] if db_mode=='crash' else ['docker','stop','--time','10',container])
                        stopped=json.loads(command(prefix+'-database-stopped-inspect',['docker','inspect',container]).stdout)[0];need(not stopped['State']['Running'] and stopped['State']['ExitCode']==(137 if db_mode=='crash' else 0),'Owned database fault missing')
                        command(prefix+'-database-start',['docker','start',container]);deadline=time.monotonic()+120
                        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<deadline,'Same-data database recovery timeout');time.sleep(.2)
                    after_fault=command(prefix+'-state-after-database',mysql+[state_sql]).stdout;need(after_fault==at_cut,'Database recovery changed original session/identity/UUID')
                    command(prefix+'-context-after-database',mysql+[context_sql])
                    command(prefix+'-restore-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                    shutil.copyfile(case_work/'class-load.log',case_out/'tc-before-class-load.log')
                    new_tc=guard.support.spawn(args,case_work,env,case_out,'tc-after');processes.append(new_tc);guard.support.wait_owned(new_tc,tc,env)
                    handle=observer.open_writer(new_tc.pid)
                    try:save(case_out/'tc-after-live.json',dict(pid=new_tc.pid,**observer.state(handle),monotonic_ns=time.perf_counter_ns()))
                    finally:observer.close(handle)
                    after_stop=case_work/'stop-after'
                    proxy_args=[sys.executable,'-B',ROOT/'tools/formal-rm-java/rm_proxy.py','--listen-port',rm,'--upstream-port',tc,'--events',case_out/'RM-after-frames.jsonl','--stop-file',after_stop,'--role','RM','--mode','basic']
                    new_proxy=guard.support.spawn(proxy_args,case_work,env,case_out,'RM-after');processes.append(new_proxy);guard.support.wait_owned(new_proxy,rm,env)
                    recovery_observations=case_out/'recovery-observations';recovery_observations.mkdir()
                    recovery_env=env|extra|dict(SEATA_XA_OUT=str(recovery_observations),SEATA_TCC_JOINT_BEFORE=str(checkpoint),SEATA_TCC_OLD_JOURNAL=str(old_journal),SEATA_TCC_JOINT_DIRECTION=direction)
                    recovery_args=[artifacts[0],'original_java_tc_recovers_joint_crash_tcc_branch','--ignored','--exact','--nocapture']
                    recovery=guard.support.spawn(recovery_args,ROOT,recovery_env,case_out,'recovery');processes.append(recovery);recovery.wait(timeout=110)
                    save(case_out/'recovery-command.json',dict(argv=list(map(str,recovery_args)),exit_code=recovery.returncode,artifact_sha256=sha(artifacts[0]),pid=recovery.pid,tc_pid=new_tc.pid))
                    need(recovery.returncode==0,'Original TC joint recovery failed: '+(case_out/'recovery-stderr.log').read_text(errors='replace')[-2000:])
                    need(b'test result: ok. 1 passed;' in (case_out/'recovery-stdout.log').read_bytes(),'Recovery native test not executed')
                    expected_global=('Committing global transaction is successfully done, xid = ' if direction=='commit' else 'Rollback global transaction successfully, xid = ')+proof['xid']+'.'
                    expected_branch=('Commit' if direction=='commit' else 'Rollback')+' branch transaction successfully, xid = '+proof['xid']+' branchId = '+proof['branch_id']
                    deadline=time.monotonic()+10
                    while not all(v.encode() in (case_out/'tc-after-stdout.log').read_bytes() for v in [expected_global,expected_branch]):need(time.monotonic()<deadline,'Restarted original TC terminal logs missing');time.sleep(.03)
                    save(case_out/'original-tc-terminal.json',dict(global_success_log=expected_global,branch_success_log=expected_branch,original_global_end_requests=1,new_global_end_requests=0,Finished_used_as_success=False))
                    command(prefix+'-state-final',mysql+[state_sql]);command(prefix+'-context-final',mysql+[context_sql]);command(prefix+'-business-dml-after',mysql+[dml_sql]);command(prefix+'-tc-final',mysql+['SELECT (SELECT COUNT(*) FROM '+database+'.global_table),(SELECT COUNT(*) FROM '+database+'.branch_table),(SELECT COUNT(*) FROM '+database+'.lock_table)'])
                    after_stop.write_bytes(b'stop');new_proxy.wait(timeout=10);need(new_proxy.returncode==0,'Restarted RM observer failed cleanup')
                finally:
                    stop.write_bytes(b'stop')
                    if (case_work/'stop-after').parent.exists():(case_work/'stop-after').write_bytes(b'stop')
                    for process in reversed(processes):guard.support.stop(process)
                    if (case_work/'class-load.log').exists():shutil.copyfile(case_work/'class-load.log',case_out/'tc-after-class-load.log')
                    save(case_out/'process-cleanup.json',dict(stopped=all(p.poll() is not None for p in processes),pids=[p.pid for p in processes]));processes=[]
    finally:
        for process in reversed(processes):guard.support.stop(process)
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==guard.IMAGE and state['Config']['Labels'].get('seata.tcc.owner')==owner,'Owned XA Java identity changed');command('mysql-log',['docker','logs',container]);command('stop',['docker','stop','--time','10',container]);command('remove',['docker','rm',container]);need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned XA Java container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),work=str(work)))
    for row in catalog_sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed original Java XA input drift')
    save(out/'result.json',dict(status='ORIGINAL_JAVA_TC_TCC_JOINT_CRASH_RECOVERY_VERIFIED',Rust_versions=versions,modes=modes,database_modes=db_modes,directions=directions,actual_global_transactions=len(versions)*len(modes)*len(db_modes)*len(directions),Java_pin=guard.reg.PIN,Java_jar_sha256=guard.reg.JAR_SHA,TCC_full_gate_complete=False,whole_plan_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--mode',choices=['delayed','immediate','precision']);parser.add_argument('--database',choices=['none','restart','crash']);parser.add_argument('--direction',choices=['commit','rollback']);args=parser.parse_args()
    run([args.rust] if args.rust else ['1.99','1.85'],[args.mode] if args.mode else ['delayed','immediate','precision'],[args.database] if args.database else ['none','restart','crash'],[args.direction] if args.direction else ['commit','rollback'])
