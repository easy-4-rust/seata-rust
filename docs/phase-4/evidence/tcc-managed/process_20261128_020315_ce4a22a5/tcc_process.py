"""Owned TCC writer process deaths at real transaction boundaries and same-data MySQL recovery."""
import argparse
import shutil
import datetime
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
from types import SimpleNamespace
import owned_writer_observation as writer_observer
import zipfile
from inventory_rebuilt import BASELINE, ROOT, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD


def run(versions, red, case=None, expected_failure=None):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_structured_logs_mysql_'+stamp)
    (work/'data').mkdir(parents=True)
    out=ROOT/'docs/phase-4/evidence/tcc-managed'/ (('process-red_' if red or expected_failure else 'process_')+stamp)
    out.mkdir(parents=True)
    with zipfile.ZipFile(BASELINE/'portable_inputs_20261127/runtime-inputs.zip') as archive:
        undo=archive.read('stage/source/script/client/at/db/mysql.sql')
    (work/'original-undo.sql').write_bytes(undo);(out/'original-undo.sql').write_bytes(undo)
    print('TCC_MYSQL '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),
        CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config',
        SEATA_MYSQL_PASSWORD=PASSWORD,SEATA_UNDO_SCHEMA_PATH=str(work/'original-undo.sql'))
    with socket.socket() as probe:
        probe.bind(('127.0.0.1',0));port=probe.getsockname()[1]
    owner=uuid.uuid4().hex
    database='undo_'+owner[:12]
    env.update(SEATA_MYSQL_DATABASE=database,SEATA_MYSQL_PORT=str(port))
    commands=[]
    def command(label,argv,expected=0,extra=None):
        if '-case-' in label and label.rsplit('-case-',1)[1].isdigit():
            stdout_path=out/(label+'.stdout');stderr_path=out/(label+'.stderr')
            with stdout_path.open('wb') as stdout,stderr_path.open('wb') as stderr:
                native=subprocess.Popen(list(map(str,argv)),cwd=ROOT,env=env|(extra or {}),stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    handles={}
                    limit=time.monotonic()+900
                    while native.poll() is None:
                        for request_path in (work/'observations').rglob('cut-request.json'):
                            ready=request_path.with_name('cut-ready.json')
                            if ready.exists():continue
                            request=json.loads(request_path.read_bytes());prefix=label+'-'+request['stage']
                            handle=writer_observer.open_writer(request['writer_pid']);handles[str(request_path.parent)]=(handle,request['writer_pid'])
                            before=writer_observer.state(handle);need(before['alive'] and before['exit_code']==259,'Writer exited before observed cut')
                            save(out/(prefix+'-writer-before.json'),dict(pid=request['writer_pid'],**before))
                            command(prefix+'-database-cut',mysql+['SELECT @@server_uuid; SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1; SELECT id,v FROM '+database+'.tcc_managed'])
                            ready.write_bytes(json.dumps(dict(writer_alive=True)).encode())
                        for request_path in (work/'observations').rglob('restart-request.json'):
                            ready=request_path.with_name('restart-ready.json')
                            if ready.exists():continue
                            request=json.loads(request_path.read_bytes());mode=request['mode']
                            need(mode in ['none','restart','crash'] and request['writer_exited'] is True,'Invalid owned database fault request')
                            prefix=label+'-'+request['stage']
                            handle,pid=handles.pop(str(request_path.parent));after=writer_observer.state(handle);writer_observer.close(handle)
                            need(not after['alive'] and after['exit_code']!=0,'Writer still alive or exited successfully')
                            save(out/(prefix+'-writer-after.json'),dict(pid=pid,**after))
                            before=command(prefix+'-database-before',mysql+['SELECT @@server_uuid; SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1; SELECT id,v FROM '+database+'.tcc_managed'])
                            need(before.stdout.splitlines()[0].decode()==request['uuid'],'Original database UUID differs before fault')
                            if mode!='none':
                                if mode=='crash':command(prefix+'-kill',['docker','kill','--signal','KILL',container])
                                else:command(prefix+'-stop',['docker','stop','--time','10',container])
                                state=json.loads(command(prefix+'-stopped-inspect',['docker','inspect',container]).stdout)[0]
                                need(not state['State']['Running'] and (mode!='crash' or state['State']['ExitCode']==137),'Owned MySQL did not actually stop/crash')
                                command(prefix+'-start',['docker','start',container])
                                deadline=time.monotonic()+120
                                while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:
                                    need(time.monotonic()<deadline,'Same-data MySQL recovery timeout');time.sleep(.2)
                            after=command(prefix+'-database-after',mysql+['SELECT @@server_uuid; SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1; SELECT id,v FROM '+database+'.tcc_managed'])
                            need(before.stdout==after.stdout,'Restart changed original UUID/Fence/business')
                            command(prefix+'-restore-log',mysql+["SET GLOBAL log_output='TABLE'; SET GLOBAL general_log=ON"])
                            ready.write_bytes(json.dumps(dict(mode=mode,uuid=request['uuid'])).encode())
                        for request_path in (work/'observations').rglob('finished-request.json'):
                            ready=request_path.with_name('finished-ready.json')
                            if ready.exists():continue
                            request=json.loads(request_path.read_bytes());prefix=label+'-'+request['stage']
                            command(prefix+'-database-final',mysql+['SELECT @@server_uuid; SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1; SELECT id,v FROM '+database+'.tcc_managed'])
                            command(prefix+'-business-dml',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type IN ('Query','Execute') AND (argument LIKE 'INSERT INTO tcc_managed%' OR argument LIKE 'UPDATE tcc_managed%' OR argument LIKE 'DELETE FROM tcc_managed%') ORDER BY event_time,thread_id"])
                            command(prefix+'-clear-log',mysql+["SET GLOBAL general_log=OFF; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                            ready.write_bytes(b'{}')
                        need(time.monotonic()<limit,'Owned TCC process matrix timeout')
                        time.sleep(.05)
                    native.wait()
                finally:
                    if native.poll() is None:native.kill();native.wait()
                    for handle,_ in handles.values():writer_observer.close(handle)
            value=SimpleNamespace(returncode=native.returncode,stdout=stdout_path.read_bytes(),stderr=stderr_path.read_bytes())
        else:
            value=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env|(extra or {}),capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-2200:].decode(errors='replace'))
        return value
    sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),Path(writer_observer.__file__)]):
            name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',sources)
    container=None
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-xa-'+stamp.replace('_','-'),
            '--label','seata.structured.logs.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql',
            '-p','127.0.0.1:'+str(port)+':3306','-e','MYSQL_ROOT_PASSWORD='+PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,IMAGE]).stdout.decode().strip()
        command('start',['docker','start',container])
        mysql=['docker','exec','-e','MYSQL_PWD='+PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e']
        limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:
            need(time.monotonic()<limit,'Owned MySQL startup timeout');time.sleep(.3)
        schema_command=['docker','exec','-i','-e','MYSQL_PWD='+PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database]
        applied=subprocess.run(schema_command,cwd=ROOT,env=env,input=undo,capture_output=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/'original-schema.stdout').write_bytes(applied.stdout);(out/'original-schema.stderr').write_bytes(applied.stderr)
        commands.append(dict(label='original-schema',argv=schema_command,exit_code=applied.returncode));save(out/'commands.json',commands);need(applied.returncode==0,'Original Undo schema install failed')
        for version in versions:
            target=ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185')
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','tcc','--test','live_tcc',
                '--offline','--locked','--no-run','--message-format=json'],extra=dict(CARGO_TARGET_DIR=str(target)))
            artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_tcc']
            need(len(artifacts)==1,'Unique native fixture artifact missing')
            cases=[case] if case else ['none','restart','crash']
            for index,selected in enumerate(cases):
                command(version+'-'+str(index)+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                observations=work/'observations'/version/('case-'+str(index));observations.mkdir(parents=True)
                value=command(version+'-case-'+str(index),[artifacts[0],'process_cases::managed_tcc_process_crash_matrix','--ignored','--exact','--nocapture'],expected=101 if red or expected_failure else 0,extra=dict(SEATA_XA_OUT=str(observations),SEATA_TCC_DATABASE_FAULT=selected))
                need((b'test result: FAILED.' if red or expected_failure else b'test result: ok. 1 passed;') in value.stdout,'TCC process matrix test summary missing')
                if expected_failure:need(expected_failure.encode() in value.stdout+value.stderr,'Expected TCC process failure missing')
                if not red:
                    command(version+'-case-'+str(index)+'-business',mysql+['SELECT id,v FROM '+database+'.tcc_managed ORDER BY id'])
                    command(version+'-case-'+str(index)+'-fence',mysql+['SELECT HEX(xid),branch_id,status FROM '+database+'.seata_rs_tcc_fence_v1 ORDER BY xid,branch_id'])
                    command(version+'-case-'+str(index)+'-undo',mysql+['SELECT COUNT(*) FROM '+database+'.undo_log'])
                    command(version+'-case-'+str(index)+'-business-dml',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type IN ('Query','Execute') AND (argument LIKE 'INSERT INTO tcc_managed%' OR argument LIKE 'UPDATE tcc_managed%' OR argument LIKE 'DELETE FROM tcc_managed%') ORDER BY event_time,thread_id"])
            save(out/(version+'-artifact.json'),dict(path=str(artifacts[0]),sha256=sha(artifacts[0])))
    finally:
        for file in (work/'observations').rglob('*'):
            if file.is_file():
                dest=out/'observations'/file.relative_to(work/'observations');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(file,dest)
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0]
            need(state['Id']==container and state['Image']==IMAGE and state['Config']['Labels'].get('seata.structured.logs.owner')==owner,'Owned runtime identity drift')
            command('mysql-log',['docker','logs',container])
            try:command('stop',['docker','stop','--time','10',container])
            finally:command('remove',['docker','rm',container])
            need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),D_data_retained=str(work)))
    for row in sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed source drift')
    save(out/'result.json',dict(status='TCC_PROCESS_MATRIX_EXPECTED_FAILURE' if red or expected_failure else 'TCC_OWNED_PROCESS_AND_SAME_DATA_DATABASE_RECOVERY_VERIFIED',Rust_versions=versions,database_fault_modes=cases,selected_case=case,expected_failure=expected_failure,TCC_full_gate_complete=False,whole_plan_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--red',action='store_true');parser.add_argument('--case',choices=['none','restart','crash']);parser.add_argument('--expect-failure');args=parser.parse_args()
    run([args.rust] if args.rust else ['1.99','1.85'],args.red,args.case,args.expect_failure)
