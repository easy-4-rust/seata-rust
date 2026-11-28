"""Owned original TC DB backend, native rolled-back key provenance, raw frame observers and actual lock rows."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
import tomllib
import tarfile
import uuid
import zipfile

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec);sys.modules[name]=value;spec.loader.exec_module(value);return value
support=module('tc_coverage_process',ROOT/'tools/formal-transport-loss/live.py');reg=support.reg
validator=module('tc_coverage_observer',HERE/'validate.py')
need=validator.need;sha=support.sha;put=support.put;save=support.save
IMAGE='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902';PASSWORD='seata-mysql-fixture-only'
DRIVER=Path('E:/Java/.m2/repository/com/mysql/mysql-connector-j/8.0.33/mysql-connector-j-8.0.33.jar')
DRIVER_SHA='e2a3b2fc726a1ac64e998585db86b30fa8bf3f706195b78bb77c5f99bf877bd9'

def restore_fixture():
    archive=ROOT/'docs/phase-1/evidence/rm-java/run_20261114_014916_69ccac54'
    need(sha(archive/'catalog.json')=='c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2','Auth archive differs')
    for row in json.loads((archive/'catalog.json').read_bytes())['members']:need(sha(archive/row['path'])==row['sha256'],'Auth source archive drift')
    historical=json.loads(reg.HISTORICAL.read_bytes());sources=json.loads((archive/'source-inputs.json').read_bytes())
    restored=[]
    for leaf,field in [('org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class','extension_class_sha256'),('META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','extension_service_sha256')]:
        target=reg.FIXTURE/leaf
        rows=[r for r in sources if Path(r['path'])==target];need(len(rows)==1,'Auth original artifact missing from archive')
        row=rows[0];data=(archive/row['snapshot']).read_bytes();need(hashlib.sha256(data).hexdigest()==row['sha256']==historical[field],'Auth original artifact hash differs')
        if target.exists():need(target.read_bytes()==data,'Existing auth artifact differs')
        else:put(target,data);restored.append(str(target))
    return dict(archive=str(archive),catalog_sha256=sha(archive/'catalog.json'),restored_exact_bytes=restored)

def run():
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
        DOCKER_CONFIG='D:/Temp/seata-docker-config',LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC','JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS','SEATA_CONFIG_NAME','SEATA_ENV']:env.pop(key,None)
    def command(args,log=None,cwd=ROOT,extra=None,timeout=120,check=True):
        value=subprocess.run([str(v) for v in args],cwd=cwd,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout,creationflags=subprocess.CREATE_NO_WINDOW)
        if log:put(out/log,value.stdout)
        if check:need(value.returncode==0,(log or str(args[0]))+' failed: '+value.stdout[-800:].decode(errors='replace'))
        return value
    jar=reg.ORACLE/'server/target/seata-server.jar'
    need(command(['git','-C',reg.ORACLE,'rev-parse','HEAD']).stdout.decode().strip()==reg.PIN and sha(jar)==reg.JAR_SHA,'Pinned original TC differs')
    restore=restore_fixture()
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/tc-lock-live'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata_tc_lock_live_'+stamp);work.mkdir();(work/'data').mkdir();(work/'init').mkdir();(work/'client').mkdir()
    print('TC_LOCK_LIVE_RUN '+str(out),flush=True)
    owner=uuid.uuid4().hex;database='tc_locks_'+owner[:12];name='seata-tc-locks-'+stamp.replace('_','-')
    ports=[]
    for _ in range(4):
        with socket.socket() as reserved:reserved.bind(('127.0.0.1',0));ports.append(reserved.getsockname()[1])
    need(len(set(ports))==4,'Ephemeral port collision');db_port,tc_port,rm_port,tm_port=ports
    schema=reg.ORACLE/'script/server/db/mysql.sql';blob=command(['git','-C',reg.ORACLE,'show',reg.PIN+':script/server/db/mysql.sql']).stdout
    need(schema.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Original TC schema differs')
    client_schema=reg.ORACLE/'script/client/at/db/mysql.sql'
    client_blob=command(['git','-C',reg.ORACLE,'show',reg.PIN+':script/client/at/db/mysql.sql']).stdout
    need(client_schema.read_bytes().replace(b'\r\n',b'\n')==client_blob.replace(b'\r\n',b'\n'),'Original Undo schema differs')
    fixture_sql='CREATE TABLE t(id VARCHAR(64) COLLATE utf8mb4_0900_as_cs PRIMARY KEY) ENGINE=InnoDB;\nCREATE TABLE t_binary(id VARBINARY(8) PRIMARY KEY) ENGINE=InnoDB;\n'
    put(work/'init/00-schema.sql',blob+b'\n'+client_blob+b'\n'+fixture_sql.encode());put(out/'original-TC-Git-schema.sql',blob);put(out/'original-Undo-Git-schema.sql',client_blob);put(out/'fixture.sql',fixture_sql)
    extension=work/'fixture'
    for path in reg.FIXTURE.rglob('*'):
        if path.is_file():put(extension/path.relative_to(reg.FIXTURE),path.read_bytes())
    need(sha(DRIVER)==DRIVER_SHA,'Frozen original TC external JDBC driver differs')
    runtime_driver=work/'driver/mysql-connector-j-8.0.33.jar';put(runtime_driver,DRIVER.read_bytes())
    config=f'''server:
  port: {tc_port}
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
    service-port: {tc_port}
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
      url: jdbc:mysql://127.0.0.1:{db_port}/{database}?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC
      user: root
      password: {PASSWORD}
      min-conn: 1
      max-conn: 4
  metrics:
    enabled: false
'''
    put(work/'application.yml',config);put(out/'application.yml',config);save(out/'auth-restoration.json',restore)
    manifest=HERE/'rust-client/Cargo.toml'
    metadata=json.loads(command(['cargo','metadata','--locked','--offline','--manifest-path',manifest,'--format-version=1']).stdout)
    save(out/'cargo-metadata.json',metadata)
    paths={Path(__file__),HERE/'proxy.py',HERE/'validate.py',Path(support.__file__),Path(reg.__file__),reg.HISTORICAL,
        ROOT/'tools/formal-rm-java/validate.py',ROOT/'tools/formal-rm-java/rm_proxy.py',ROOT/'tools/formal-rm-java/rust-client/src/fixture_resource.rs',
        ROOT/'docs/phase-2/tc-lock-coverage-contract.md',ROOT/'Cargo.toml',ROOT/'Cargo.lock',schema,client_schema,work/'application.yml',work/'init/00-schema.sql',reg.JAVA,DRIVER,runtime_driver}
    paths.update(p for folder in [ROOT/'crates',HERE/'rust-client',extension] for p in folder.rglob('*') if p.is_file())
    lock=tomllib.loads((HERE/'rust-client/Cargo.lock').read_text('utf-8'));registry={(p['name'],p['version']):p for p in lock['package'] if p.get('source','').startswith('registry+')}
    archives=[]
    for package in metadata['packages']:
        if package['source'] and package['source'].startswith('registry+'):
            paths.update(p for p in Path(package['manifest_path']).parent.rglob('*') if p.is_file())
            candidates=list((Path(env['CARGO_HOME'])/'registry/cache').glob('*/'+package['name']+'-'+package['version']+'.crate'))
            expected=registry[(package['name'],package['version'])]['checksum'];need(len(candidates)==1 and sha(candidates[0])==expected,'Actual registry archive differs from Cargo lock')
            paths.add(candidates[0]);archives.append(dict(name=package['name'],version=package['version'],archive=str(candidates[0]),sha256=expected))
            source_root=Path(package['manifest_path']).parent;expected_paths=set()
            with tarfile.open(candidates[0],'r:gz') as archive:
                prefix=package['name']+'-'+package['version']+'/'
                for member in archive.getmembers():
                    if member.isdir():continue
                    need(member.isfile() and member.name.startswith(prefix),'Unexpected registry archive member')
                    relative=member.name[len(prefix):];need('..' not in Path(relative).parts,'Registry archive path escapes root')
                    actual=source_root/relative;expected_paths.add(actual)
                    need(actual.read_bytes()==archive.extractfile(member).read(),'Unpacked registry source differs from lock-checksummed archive')
            need({p for p in source_root.rglob('*') if p.is_file() and p.name not in ['.cargo-ok','.cargo-checksum.json']}==expected_paths,'Unpacked registry source membership differs')
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    # Bind the unchanged production sources to the preceding component gate before any original TC request.
    previous=ROOT/'docs/phase-2/evidence/lock-key-gate/run_20261116_011215_dc9906b8'
    need(sha(previous/'catalog.json')=='3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07','Previous lock component authority differs')
    old=json.loads((previous/'source-inputs.json').read_bytes());compared=0
    for row in old:
        path=Path(row['path'])
        if path.is_relative_to(ROOT/'crates'):
            need(sha(path)==row['sha256'],'Production/test source differs from preceding component gate');compared+=1
    save(out/'source-inputs.json',frozen);save(out/'production-binding.json',dict(previous_catalog_sha256=sha(previous/'catalog.json'),unchanged_crate_inputs=compared))
    save(out/'registry-archives.json',archives)
    # Freeze and verify all selected sources/archives BEFORE the first compilation, then retain Cargo's actual artifact closure.
    command(['rustc','--version','--verbose'],'rust-version.txt')
    command(['cargo','build','--locked','--offline','--manifest-path',manifest,'--message-format=json'],'build.jsonl',timeout=240)
    command(['cargo','fmt','--manifest-path',manifest,'--','--check'],'fmt.log')
    command(['cargo','clippy','--locked','--offline','--manifest-path',manifest,'--all-targets','--','-D','warnings'],'clippy.log',timeout=180)
    command(['cargo','test','--locked','--offline','--manifest-path',manifest],'fixture-tests.log',timeout=180)
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/seata-tc-lock-coverage-validation.exe';binary_sha=sha(binary);put(out/'artifacts/client.exe',binary.read_bytes())
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Input drift during compilation')
    command(['docker','image','inspect',IMAGE],'image.json');container=None;tc=None;rm=None;tm=None;client=None;cleanup=[];checkpoints=[];cases=None;client_result=None
    extra=dict(SEATA_TC_WORK_DIR=str(work),SEATA_TC_LOCK_DIR=str(work/'client'),SEATA_TC_LOCK_NONCE=owner,
        SEATA_RM_ENDPOINT=f'127.0.0.1:{rm_port}',SEATA_TM_ENDPOINT=f'127.0.0.1:{tm_port}',SEATA_MYSQL_PORT=str(db_port),SEATA_MYSQL_PASSWORD=PASSWORD,SEATA_MYSQL_DATABASE=database)
    stop=work/'stop-proxy'
    try:
        container=command(['docker','create','--pull=never','--name',name,'--label','seata-rust-tc-lock-owner='+owner,
            '--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql',
            '--mount','type=bind,source='+(work/'init').as_posix()+',target=/docker-entrypoint-initdb.d,readonly',
            '-p',f'127.0.0.1:{db_port}:3306','-e','MYSQL_ROOT_PASSWORD='+PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,IMAGE],'create.log').stdout.decode().strip()
        save(out/'owned-runtime.json',dict(container_id=container,label_owner=owner,database=database,ports=ports,runtime=str(work),public_fixture_password=True))
        command(['docker','start',container],'start.log')
        mysql=['docker','exec','-e','MYSQL_PWD='+PASSWORD,container,'mysql','--protocol=TCP','-h127.0.0.1','-uroot','--default-character-set=utf8mb4','-N','-B','--raw',database,'-e']
        deadline=time.monotonic()+120
        while True:
            value=command(mysql+['SELECT COUNT(*) FROM t'],check=False)
            if value.returncode==0:break
            need(time.monotonic()<deadline,'Owned MySQL startup timeout');time.sleep(.3)
        command(mysql+['SELECT VERSION(),@@lower_case_table_names,@@transaction_isolation; SHOW CREATE TABLE t; SHOW CREATE TABLE lock_table'],'mysql-identity.txt')
        args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),'-Dloader.path='+extension.as_posix()+','+runtime_driver.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
        tc=support.spawn(args,work,env|extra,out,'tc');support.wait_owned(tc,tc_port,env)
        for role,port,leaf in [('RM',rm_port,'events.jsonl'),('TM',tm_port,'tm-events.jsonl')]:
            process=support.spawn([sys.executable,HERE/'proxy.py','--listen-port',port,'--upstream-port',tc_port,'--events',out/leaf,'--stop-file',stop,'--role',role],work,env|extra,out,role.lower()+'-proxy')
            if role=='RM':rm=process
            else:tm=process
            support.wait_owned(process,port,env)
        client=support.spawn([binary],work,env|extra,out,'client')
        save(out/'owned-processes.json',dict(tc_pid=tc.pid,rm_proxy_pid=rm.pid,tm_proxy_pid=tm.pid,client_pid=client.pid,TC_command=list(map(str,args)),observer_identity_given_to_client=False))
        consumed=0;deadline=time.monotonic()+240
        while True:
            lines=(out/'client-stdout.log').read_text('utf-8').splitlines()
            # A complete line has been flushed before the client waits for this independent observer.
            raw=(out/'client-stdout.log').read_bytes()
            if raw and not raw.endswith(b'\n'):lines=lines[:-1]
            for line in lines[consumed:]:
                if line.startswith('CHECKPOINT '):
                    item=json.loads(line[len('CHECKPOINT '):]);item['observer_time_ns']=time.perf_counter_ns()
                    sql="SELECT JSON_OBJECT('xid',xid,'branch_id',branch_id,'resource',resource_id,'table',table_name,'pk',pk,'row_key',row_key) FROM lock_table ORDER BY row_key;"
                    branches="SELECT JSON_OBJECT('xid',xid,'branch_id',branch_id,'resource',resource_id) FROM branch_table ORDER BY branch_id;"
                    globals_sql="SELECT JSON_OBJECT('xid',xid,'status',status) FROM global_table ORDER BY xid;"
                    for key,query in [('locks',sql),('branches',branches),('globals',globals_sql)]:
                        item[key]=[json.loads(r) for r in command(mysql+[query]).stdout.decode('utf-8').splitlines()]
                    if item['stage']=='provenance':
                        cases=item['data']['cases'];item['business_counts']={}
                        for table in ['t','t_binary','undo_log']:item['business_counts'][table]=int(command(mysql+['SELECT COUNT(*) FROM '+table]).stdout)
                    item['observer_completed_ns']=time.perf_counter_ns();checkpoints.append(item);save(out/f'checkpoint-{len(checkpoints):02d}.json',item)
                    put(work/'client'/f"ack-{item['index']}-{item['stage']}",'observed\n')
                elif line.startswith('RESULT '):client_result=json.loads(line[len('RESULT '):])
            consumed=len(lines)
            if client.poll() is not None:break
            need(time.monotonic()<deadline,'Native coverage client timed out');time.sleep(.03)
        need(client.returncode==0 and client_result is not None,'Native client failed; inspect retained stdout/stderr')
        put(stop,'complete\n');rm.wait(timeout=12);tm.wait(timeout=12);need(rm.returncode==0 and tm.returncode==0,'Original observer failed')
        for path in (work/'client').iterdir():
            if path.is_file() and not path.name.startswith('ack-'):put(out/'client-artifacts'/path.name,path.read_bytes())
        save(out/'checkpoints.json',checkpoints);save(out/'client-result.json',client_result)
        rm_rows=[json.loads(r) for r in (out/'events.jsonl').read_text('utf-8').splitlines()];tm_rows=[json.loads(r) for r in (out/'tm-events.jsonl').read_text('utf-8').splitlines()]
        native={p.name:json.loads(p.read_bytes()) for p in (work/'client').glob('native-*.json') if p.name!='native-cases.json'}
        verified=validator.validate(cases,checkpoints,rm_rows,tm_rows,owner,client_result,native);save(out/'verified.json',verified)
        command(mysql+['SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM t),(SELECT COUNT(*) FROM t_binary),(SELECT COUNT(*) FROM undo_log)'],'final-counts.txt')
        need((out/'final-counts.txt').read_text().strip()=='0\t0\t0\t0\t0\t0','Owned TC/business rows remain')
    finally:
        if not stop.exists():put(stop,'cleanup\n')
        for process in [client,rm,tm,tc]:support.stop(process)
        for port in [tc_port,rm_port,tm_port]:
            if support.listeners(port,env):cleanup.append(dict(port=port,reason='Listener remains'))
        if (work/'class-load.log').exists():put(out/'class-load.log',(work/'class-load.log').read_bytes())
        if container:
            state=json.loads(command(['docker','inspect',container]).stdout)[0]
            need(state['Id']==container and state['Config']['Labels'].get('seata-rust-tc-lock-owner')==owner,'Owned container identity differs')
            command(['docker','logs',container],'mysql.log');command(['docker','stop','--time','10',container],'stop.log');command(['docker','rm',container],'remove.log')
            need(command(['docker','inspect',container],check=False).returncode!=0,'Owned container remains')
        save(out/'cleanup.json',dict(errors=cleanup,owned_processes_stopped=True,owned_container_removed=bool(container),D_data_retained=str(work/'data')))
    need(not cleanup,'Owned cleanup incomplete')
    # Every non-JDK file class origin must resolve to the pinned server archive or exact authenticated public SPI.
    loaded=re.findall(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(out/'class-load.log').read_text('utf-8'));bound={}
    with zipfile.ZipFile(jar) as server:
        entries={entry.filename:hashlib.sha256(server.read(entry)).hexdigest() for entry in server.infolist() if not entry.is_dir()}
    for name,origin in loaded:
        origin=origin.replace('\\','/')
        if origin.startswith(('file:','jar:')):
            need(jar.as_posix() in origin or extension.as_posix() in origin or runtime_driver.as_posix() in origin,'Unexpected TC file class origin: '+origin);bound[name]=origin
    critical=['org.apache.seata.server.lock.AbstractLockManager','org.apache.seata.server.session.BranchSession','org.apache.seata.core.lock.RowLock',
        'org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler','com.mysql.cj.jdbc.Driver','org.apache.seata.server.coordinator.DefaultCore']
    for name in critical:need(name in bound,'Missing original TC class origin: '+name)
    need(extension.as_posix() in bound[critical[3]] and runtime_driver.as_posix() in bound['com.mysql.cj.jdbc.Driver'] and all(jar.as_posix() in bound[n] for n in critical if n not in [critical[3],'com.mysql.cj.jdbc.Driver']),'Critical TC class shadowed')
    save(out/'class-origins.json',dict(bound_file_classes=bound,pinned_jar_entries=entries,critical_classes=critical))
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source/runtime drift: '+row['path'])
    need(sha(jar)==reg.JAR_SHA and sha(binary)==binary_sha,'Java/client executable drift')
    save(out/'result.json',dict(status='ORIGINAL_TC_NETWORK_STORAGE_OBSERVATIONS_VERIFIED',java_pin=reg.PIN,original_jar_sha256=reg.JAR_SHA,actual_original_TC_processes=1,
        native_key_cases=8,actual_original_XIDs=16,actual_TC_database_snapshots=len(checkpoints),independent_raw_observers=2,SQL_key_provenance_rolled_back=True,
        business_commit_performed=False,production_sources_unchanged=True,TC_safety_gate_passed=False,TC_registration_gate_passed=False,SQL_Undo_atomicity_verified=False,
        public_COMMIT_available=False,full_original_source_JUnit_migrated=False,WP05_complete=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)

if __name__=='__main__':run()
