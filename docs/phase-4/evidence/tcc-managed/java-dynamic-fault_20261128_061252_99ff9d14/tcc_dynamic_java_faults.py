"""Owned pinned original Java TC + managed Rust TCC registration and normal two-phase end."""
import argparse,datetime,json,os,socket,subprocess,sys,time,uuid,zipfile,shutil
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
sys.path.insert(0,str(ROOT/'tools/formal-tc-lock-coverage'))
import live_guard as guard

def port():
    with socket.socket() as probe:probe.bind(('127.0.0.1',0));return probe.getsockname()[1]

def run(versions,modes,faults):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-4/evidence/tcc-managed'/('java-dynamic-fault_'+stamp);out.mkdir(parents=True);work=Path('D:/codex-builds')/('seata-tcc-java-'+stamp);(work/'data').mkdir(parents=True);print('TCC_ORIGINAL_JAVA '+str(out),flush=True)
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
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'tools/formal-rm-java/rm_proxy.py',ROOT/'tools/formal-rm-java/tcc_dynamic_proxy.py']):name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());catalog_sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',catalog_sources)
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-tcc-java-'+stamp.replace('_','-'),'--label','seata.tcc.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql','-p','127.0.0.1:'+str(db_port)+':3306','-e','MYSQL_ROOT_PASSWORD='+guard.PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,guard.IMAGE]).stdout.decode().strip();command('start',['docker','start',container]);mysql=['docker','exec','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e'];limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<limit,'Owned XA Java MySQL startup timeout');time.sleep(.3)
        server=command('original-server-schema',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':script/server/db/mysql.sql']).stdout;(out/'original-server.sql').write_bytes(server);command('install-server-schema',['docker','exec','-i','-e','MYSQL_PWD='+guard.PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database],input=server)
        resource=f'jdbc:mysql://127.0.0.1:{db_port}/{database}'
        (out/'original-DefaultCore.java').write_bytes(command('original-terminal-source',['git','-C',guard.reg.ORACLE,'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java']).stdout)
        overlay=guard.compile_auth(jar,work,out,env,False)
        for version in versions:
            env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','tcc','--test','live_tcc_java','--offline','--locked','--no-run','--message-format=json']);artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_tcc_java'];need(len(artifacts)==1,'Unique original Java TCC client missing')
            for mode,fault,direction in __import__('itertools').product(modes,faults,['commit','rollback']):
                if fault.startswith('report_') and direction!='rollback':continue
                prefix=version+'-'+mode+'-'+fault+'-'+direction;case_work=work/prefix;case_work.mkdir();case_out=out/prefix;case_out.mkdir();tc,rm,tm=port(),port(),port();need(len({tc,rm,tm,db_port})==4,'Owned ports overlap');stop=case_work/'stop';processes=[];held=case_work/'report-held.json';release=case_work/'release-response'
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
                    for role,listen in [('RM',rm),('TM',tm)]:
                        proxy=ROOT/'tools/formal-rm-java'/('tcc_dynamic_proxy.py' if role=='RM' else 'rm_proxy.py')
                        args=[sys.executable,'-B',proxy,'--listen-port',listen,'--upstream-port',tc,'--events',case_out/(role+'-frames.jsonl'),'--stop-file',stop,'--mode',fault if role=='RM' else 'basic']
                        if role=='TM':args+=['--role','TM']
                        elif fault=='report_cancel':args+=['--hold-file',held,'--release-file',release]
                        processes.append(guard.support.spawn(args,case_work,env,case_out,role));guard.support.wait_owned(processes[-1],listen,env)
                    command(prefix+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                    observations=case_out/'observations';observations.mkdir();extra=dict(SEATA_TCC_JAVA_FAULT=fault,SEATA_TCC_DYNAMIC=mode,SEATA_TCC_DYNAMIC_HOLD="1",SEATA_XA_OUT=str(observations),SEATA_MYSQL_PORT=str(db_port),SEATA_MYSQL_PASSWORD=guard.PASSWORD,SEATA_MYSQL_DATABASE=database,SEATA_RM_ENDPOINT=f'127.0.0.1:{rm}',SEATA_TM_ENDPOINT=f'127.0.0.1:{tm}')
                    client_args=[str(artifacts[0]),'original_java_tc_'+('commits' if direction=='commit' else 'rolls_back')+'_original_tcc_branch','--ignored','--exact','--nocapture']
                    stdout=case_out/'client.stdout';stderr=case_out/'client.stderr'
                    with stdout.open('wb') as output,stderr.open('wb') as errors:
                        client=subprocess.Popen(client_args,cwd=ROOT,env=env|extra,stdout=output,stderr=errors,creationflags=subprocess.CREATE_NO_WINDOW)
                        try:
                            limit=time.monotonic()+90
                            checkpoint=observations/'proof/dynamic-before-end.json'
                            if fault=='report_cancel':
                                while client.poll() is None and not held.exists():
                                    need(time.monotonic()<limit,'Original successful report response never held');time.sleep(.02)
                                need(held.exists(),'Client exited before actual report cancellation boundary')
                                shutil.copyfile(held,case_out/'report-held.json')
                                command(prefix+'-held-tc',mysql+['SELECT HEX(xid),branch_id,branch_type,status,HEX(application_data) FROM '+database+'.branch_table'])
                                command(prefix+'-held-context',mysql+['SELECT HEX(xid),branch_id,revision,HEX(resource_id),HEX(registration_data),HEX(application_data),HEX(server_uuid),schema_version,HEX(checksum) FROM '+database+'.seata_rs_tcc_context_v1 ORDER BY revision'])
                                command(prefix+'-held-business',mysql+['SELECT id,v FROM '+database+'.tcc_managed ORDER BY id'])
                                command(prefix+'-held-fence',mysql+['SELECT COUNT(*) FROM '+database+'.seata_rs_tcc_fence_v1'])
                                (observations/'control').mkdir(exist_ok=True);(observations/'control/cancel-try').write_bytes(b'cancel')
                            while client.poll() is None and not checkpoint.exists():
                                need(time.monotonic()<limit,'Dynamic report checkpoint timeout');time.sleep(.02)
                            need(checkpoint.exists(),'Client exited before original TC observed context')
                            command(prefix+'-independent-report-tc',mysql+['SELECT HEX(xid),branch_id,branch_type,status,HEX(application_data) FROM '+database+'.branch_table'])
                            command(prefix+'-independent-report-context',mysql+['SELECT HEX(xid),branch_id,revision,HEX(resource_id),HEX(registration_data),HEX(application_data),HEX(server_uuid),schema_version,HEX(checksum) FROM '+database+'.seata_rs_tcc_context_v1 ORDER BY revision'])
                            command(prefix+'-independent-report-business',mysql+['SELECT id,v FROM '+database+'.tcc_managed ORDER BY id'])
                            command(prefix+'-before-end-fence',mysql+['SELECT COUNT(*) FROM '+database+'.seata_rs_tcc_fence_v1'])
                            if fault=='report_cancel':release.write_bytes(b'drop-after-cancel')
                            if fault.startswith('report_'):
                                # 先确认旧会话真实关闭及新 RM 登记，再允许原 TM 发送唯一 Rollback。
                                limit_reconnect=time.monotonic()+15
                                while True:
                                    rows=[json.loads(line) for line in (case_out/'RM-frames.jsonl').read_bytes().split(b'\n')[:-1] if line]
                                    closes=[r for r in rows if r.get('event')=='fault_close']
                                    acknowledgements=[r for r in rows if r.get('event')=='frame' and r.get('kind')==104 and r['direction']=='s2c' and r['connection']>1 and (body:=bytes.fromhex(r['frame_hex']))[int.from_bytes(body[7:9],'big')+2:int.from_bytes(body[7:9],'big')+4]==b'\x01\x00']
                                    if closes and acknowledgements:
                                        save(case_out/'reconnected-before-end.json',dict(closed_connection=closes[0]['connection'],new_connection=acknowledgements[0]['connection'],fault_close_index=closes[0]['index'],registration_ack_index=acknowledgements[0]['index']));break
                                    need(time.monotonic()<limit_reconnect,'Original report session not closed and re-registered');time.sleep(.02)
                            (observations/'control').mkdir(exist_ok=True);(observations/'control/allow-end').write_bytes(b'allow')
                            client.wait(timeout=90)
                        finally:
                            if client.poll() is None:client.kill();client.wait()
                    value=__import__('types').SimpleNamespace(returncode=client.returncode,stdout=stdout.read_bytes(),stderr=stderr.read_bytes());(case_out/'client.stdout').write_bytes(value.stdout);(case_out/'client.stderr').write_bytes(value.stderr);save(case_out/'client-command.json',dict(path=str(artifacts[0]),sha256=sha(artifacts[0]),exit_code=value.returncode));need(value.returncode==0,'Original Java TCC case failed: '+(value.stdout+value.stderr)[-2200:].decode(errors='replace'));need(b'test result: ok. 1 passed;' in value.stdout,'Original Java TCC case did not execute')
                    proof=json.loads((observations/'proof/original-java-tcc-dynamic-fault.json').read_bytes());xid=proof['xid'];branch=proof['branch_id']
                    global_log=(('Committing global transaction is successfully done, xid = ' if direction=='commit' else 'Rollback global transaction successfully, xid = ')+xid+'.')
                    branch_log=(('Commit' if direction=='commit' else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+branch)
                    terminal_deadline=time.monotonic()+5
                    while global_log.encode() not in (case_out/'tc-stdout.log').read_bytes() or branch_log.encode() not in (case_out/'tc-stdout.log').read_bytes():
                        need(time.monotonic()<terminal_deadline,'Original TC branch/global terminal logs missing');time.sleep(.05)
                    save(case_out/'original-tc-terminal.json',dict(xid=xid,branch_id=branch,direction=direction,original_branch_success_log=branch_log,original_global_success_log=global_log,Finished_used_as_success=False))
                    command(prefix+'-final-context',mysql+['SELECT HEX(xid),branch_id,revision,HEX(resource_id),HEX(registration_data),HEX(application_data),HEX(server_uuid),schema_version,HEX(checksum) FROM '+database+'.seata_rs_tcc_context_v1 ORDER BY revision'])
                    command(prefix+'-final-fence',mysql+['SELECT HEX(xid),branch_id,HEX(resource_id),HEX(application_data),HEX(server_uuid),schema_version,status FROM '+database+'.seata_rs_tcc_fence_v1'])
                    command(prefix+'-independent-state',mysql+['SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1']);command(prefix+'-independent-business',mysql+['SELECT id,v FROM '+database+'.tcc_managed ORDER BY id']);command(prefix+'-independent-prepared',mysql+['XA RECOVER CONVERT XID']);command(prefix+'-independent-tc',mysql+['SELECT (SELECT COUNT(*) FROM '+database+'.global_table),(SELECT COUNT(*) FROM '+database+'.branch_table),(SELECT COUNT(*) FROM '+database+'.lock_table)']);command(prefix+'-native-commands',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type='Query' AND argument LIKE 'XA %' ORDER BY event_time,thread_id"])
                    command(prefix+'-fence-protocol',mysql+['SELECT schema_version FROM '+database+'.seata_rs_tcc_fence_v1'])
                    command(prefix+'-business-dml',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type IN ('Query','Execute') AND (argument LIKE 'INSERT INTO tcc_managed%' OR argument LIKE 'UPDATE tcc_managed%' OR argument LIKE 'DELETE FROM tcc_managed%') ORDER BY event_time,thread_id"])
                finally:
                    stop.write_text('stop\n',encoding='utf-8')
                    for process in processes[1:]:
                        try:process.wait(timeout=10)
                        except subprocess.TimeoutExpired:guard.support.stop(process)
                    for process in reversed(processes):guard.support.stop(process)
                    save(case_out/'process-cleanup.json',dict(stopped=all(p.poll() is not None for p in processes),pids=[p.pid for p in processes]));processes=[]
                    for role in ['RM','TM']:
                        summaries=[json.loads(line) for line in (case_out/(role+'-frames.jsonl')).read_bytes().splitlines() if json.loads(line).get('event')=='summary'];need(len(summaries)==1 and summaries[0]['succeeded'],'Dynamic fault observer incomplete')
    finally:
        for process in reversed(processes):guard.support.stop(process)
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==guard.IMAGE and state['Config']['Labels'].get('seata.tcc.owner')==owner,'Owned XA Java identity changed');command('mysql-log',['docker','logs',container]);command('stop',['docker','stop','--time','10',container]);command('remove',['docker','rm',container]);need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned XA Java container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),work=str(work)))
    for row in catalog_sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed original Java XA input drift')
    save(out/'result.json',dict(status='ORIGINAL_JAVA_TC_TCC_DYNAMIC_FAULTS_VERIFIED',Rust_versions=versions,modes=modes,faults=faults,actual_global_transactions=len(versions)*len(modes)*sum(1 if f.startswith('report_') else 2 for f in faults),Java_pin=guard.reg.PIN,Java_jar_sha256=guard.reg.JAR_SHA,TCC_full_gate_complete=False,whole_plan_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--mode',choices=['delayed','immediate','precision']);parser.add_argument('--fault',choices=['duplicate','reconnect','report_lost','report_cancel']);args=parser.parse_args();run([args.rust] if args.rust else ['1.99','1.85'],[args.mode] if args.mode else ['delayed','immediate','precision'],[args.fault] if args.fault else ['duplicate','reconnect','report_lost','report_cancel'])
