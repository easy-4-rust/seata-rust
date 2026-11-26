"""Owned MySQL Undo-rollback tests with frozen inputs and exact executed artifacts; no public COMMIT claim."""
import argparse
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
import uuid

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
JAVA=ROOT.parent/'seata-java-phase09-oracle'
BASE=ROOT/'docs/phase-2/evidence/tc-lock-live/run_20261116_020229_df8ce4db'
IMAGE='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902';PASSWORD='seata-mysql-fixture-only'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def need(value,reason):
    if not value:raise ValueError(reason)
def run(rust,case=None,red=False,red_reason='unused',target_override=None,java=False,wire=False,enrollment_fault=None):
    need(not enrollment_fault or (not case and not red and not target_override and not java and not wire),'Enrollment fault role conflict')
    need(not java or (not case and not red and not target_override and not wire),'Java role conflict')
    need(not target_override or (not case and not red and not wire),'Regression role conflict')
    need(not wire or (not case and not red),'Wire role conflict')
    if wire:case='preparation_uses_real_images_and_positive_receipt_but_keeps_business_and_undo_uncommitted'
    library=False
    target_name=target_override or ('live_first_phase_enrollment_loss' if enrollment_fault else 'live_first_phase_java' if java else 'live_first_phase_preparation')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/mysql-first-phase-preparation'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata_first_phase_prepare_'+stamp);(work/'data').mkdir(parents=True);(work/'init').mkdir()
    owner=uuid.uuid4().hex;database='undo_'+owner[:12];name='seata-first-phase-prepare-'+stamp.replace('_','-')
    env=os.environ|dict(CARGO_INCREMENTAL='0',CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',DOCKER_CONFIG='D:/Temp/seata-docker-config',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    cargo='cargo'
    if rust=='1.85':
        tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';env.update(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
    cache_root=ROOT/'docs/phase-2/evidence/mysql-rm-recovery/run_20261117_120200_cf61fbb6'
    cache={row['sha256']:cache_root/row['path'] for row in json.loads((cache_root/'catalog.json').read_bytes())['members']}
    storage=ROOT/'docs/phase-2/evidence/mysql-callback-ownership-gate/run_20261117_145639_40c3447e'
    need(sha(storage/'catalog.json')=='07435a2698a06f42ed3e9e28c70ca007dbdc915476f9b138ec1114b24bed5db6','Storage cache catalog differs')
    cache.update({row['sha256']:storage/row['path'] for row in json.loads((storage/'catalog.json').read_bytes())['members']})
    # 这里只选择内容摘要相同的存储字节，测试角色仍由显式 manifest 及返回 receipt 绑定。
    for previous in sorted((ROOT/'docs/phase-2/evidence/mysql-first-phase-preparation').glob('run_*/catalog.json')):
        try:rows=json.loads(previous.read_bytes())['members']
        except (ValueError,KeyError):continue
        cache.update({row['sha256']:previous.parent/row['path'] for row in rows})
    def put(leaf,data):
        p=out/leaf;p.parent.mkdir(parents=True,exist_ok=True);data=data.encode('utf-8') if isinstance(data,str) else data
        digest=hashlib.sha256(data).hexdigest();canonical=cache.get(digest)
        if not p.exists() and canonical and canonical.resolve().is_relative_to((ROOT/'docs/phase-2/evidence').resolve()) and canonical.stat().st_nlink<900 and sha(canonical)==digest:
            os.link(canonical,p)
        else:
            p.write_bytes(data);cache[digest]=p
    def save(leaf,data):put(leaf,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,leaf=None,check=True,timeout=180):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env,creationflags=subprocess.CREATE_NO_WINDOW,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
        if leaf:put(leaf,v.stdout)
        if check:need(v.returncode==0,(leaf or str(args[0]))+' failed '+v.stdout[-1000:].decode(errors='replace'))
        return v
    print('MYSQL_FIRST_PHASE_PREPARATION_RUN '+str(out),flush=True)
    need(sha(BASE/'catalog.json')=='58b48297821a70c461f6bca502a6597c9f83c5f8c4cd713b8b40b395c31eede2','Dependency authority differs')
    paths={Path(__file__),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-first-phase-preparation-contract.md',JAVA/'script/client/at/db/mysql.sql'}
    legacy_packet=ROOT/'docs/phase-2/evidence/mysql-rm-recovery/run_20261117_124814_c45c02c6'
    need(sha(legacy_packet/'catalog.json')=='6362c678ce4ee62f5185ed0eeca96e9b128038c5571b3e3493c2837c44d068ac','Legacy child authority differs')
    legacy_member=next(row for row in json.loads((legacy_packet/'catalog.json').read_bytes())['members'] if row['path']=='artifacts/mysql_rm_recovery_child.exe')
    legacy_exe=legacy_packet/legacy_member['path'];need(sha(legacy_exe)==legacy_member['sha256']=='a3a7bbcbb56db5e05a514767c79a16f490af68d18a01adc2d80671041db80321','Legacy actual executable differs')
    paths.update([legacy_packet/'catalog.json',legacy_exe])
    java_module=None
    if java:
        spec=importlib.util.spec_from_file_location('callback_original_java',HERE/'java_runtime.py');java_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(java_module);paths.update(java_module.inputs())
    paths.update(p for p in HERE.rglob('*') if p.is_file())
    paths.update(p for p in (ROOT/'tools/formal-undo-rollback-loss').rglob('*') if p.is_file())
    paths.add(ROOT/'tools/formal-undo-fence/validate_wire.py')
    paths.add(ROOT/'tools/formal-rm-mysql/validate.py')
    paths.add(ROOT/'tools/formal-callback-ownership/validate.py')
    paths.add(Path('D:/codex-builds/seata-first-phase-api-red-20261117.log'))
    paths.add(ROOT/'tools/formal-undo-format/runtime_binding.py')
    paths.update(p for p in (ROOT/'tools/formal-undo-rollback-java').rglob('*') if p.is_file())
    paths.update(p for p in (ROOT/'crates').rglob('*') if p.is_file())
    for row in json.loads((BASE/'source-inputs.json').read_bytes()):
        if '/registry/' in row['path'].replace('\\','/'):
            p=Path(row['path']);need(sha(p)==row['sha256']==sha(BASE/row['snapshot']),'Frozen dependency source/archive drift');paths.add(p)
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),sha256=sha(p),snapshot=leaf))
    save('source-inputs.json',frozen)
    legacy_copy=work/'legacy_mysql_rm_recovery_child.exe';legacy_copy.write_bytes(legacy_exe.read_bytes());need(sha(legacy_copy)==legacy_member['sha256'],'Legacy D executable differs');env['SEATA_RM_LEGACY_CHILD_EXE']=str(legacy_copy)
    save('legacy-reader-binding.json',dict(parent_catalog_sha256=sha(legacy_packet/'catalog.json'),source_artifact=str(legacy_exe),executed_artifact=str(legacy_copy),sha256=sha(legacy_copy),old_reader_expected='explicit expansion rejected without mutation'))
    child_build=command([cargo,'build','--locked','--offline','-p','seata-db-mysql','--example','mysql_rm_recovery_child','--message-format=json'],'child-build.jsonl').stdout
    children=[json.loads(line)['executable'] for line in child_build.splitlines() if line.startswith(b'{') and json.loads(line).get('reason')=='compiler-artifact' and json.loads(line).get('executable')]
    need(len(children)==1,'Exact recovery child artifact required');child=Path(children[0]);put('artifacts/mysql_rm_recovery_child.exe',child.read_bytes());env['SEATA_RM_CHILD_EXE']=str(child)
    command([env.get('RUSTC','rustc'),'--version','--verbose'],'rust-version.txt')
    target=['--lib'] if library else ['--test',target_name]
    build=command([cargo,'test','--locked','--offline','-p','seata-db-mysql']+target+['--no-run','--message-format=json'],'build.jsonl').stdout
    artifacts=[]
    for line in build.splitlines():
        try:r=json.loads(line)
        except ValueError:continue
        if r.get('reason')=='compiler-artifact' and r['target']['name']==('seata_db_mysql' if library else target_name) and r.get('executable'):artifacts.append(Path(r['executable']))
    need(len(artifacts)==1,'Missing actual test executable');binary=artifacts[0];digest=sha(binary);put('artifacts/live_rm_recovery.exe',binary.read_bytes())
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    env.update(SEATA_MYSQL_PORT=str(port),SEATA_MYSQL_DATABASE=database,SEATA_MYSQL_PASSWORD=PASSWORD,SEATA_UNDO_STORE_OUT=str(work/'observations'))
    (work/'observations').mkdir()
    need(command(['git','-C',JAVA,'rev-parse','HEAD']).stdout.decode().strip()=='cba73e180fd8896a99df6415b859d275122feaa7','Java pin differs')
    schema=command(['git','-C',JAVA,'show','cba73e180fd8896a99df6415b859d275122feaa7:script/client/at/db/mysql.sql']).stdout
    need((JAVA/'script/client/at/db/mysql.sql').read_bytes().replace(b'\r\n',b'\n')==schema.replace(b'\r\n',b'\n'),'Original schema source differs')
    (work/'init/00-undo.sql').write_bytes(schema);put('original-undo.sql',schema);env['SEATA_UNDO_SCHEMA_PATH']=str(work/'init/00-undo.sql')
    container=None;status=False;java_peer=None;observer=None;wire_data=None;java_runtime=None
    try:
        container=command(['docker','create','--pull=never','--name',name,'--label','seata-first-phase-prepare-owner='+owner,
            '--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql',
            '--mount','type=bind,source='+(work/'init').as_posix()+',target=/docker-entrypoint-initdb.d,readonly',
            '-p',f'127.0.0.1:{port}:3306','-e','MYSQL_ROOT_PASSWORD='+PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,IMAGE],'create.log').stdout.decode().strip()
        save('owned-runtime.json',dict(container_id=container,label_owner=owner,database=database,port=port,runtime=str(work),image=IMAGE))
        command(['docker','start',container],'start.log')
        mysql=['docker','exec','-e','MYSQL_PWD='+PASSWORD,container,'mysql','--protocol=TCP','-h127.0.0.1','-uroot','-N','-B',database,'-e']
        deadline=time.monotonic()+120
        while command(mysql+['SELECT COUNT(*) FROM undo_log'],check=False).returncode:
            need(time.monotonic()<deadline,'Owned MySQL startup timeout');time.sleep(.3)
        command(mysql+['SELECT VERSION(),@@max_allowed_packet,@@performance_schema,@@lower_case_table_names'],'server-before.txt')
        if java:java_runtime=java_module.Runtime(out,work,env)
        if wire or enrollment_fault:
            spec=importlib.util.spec_from_file_location('preparation_sql_proxy',HERE/'proxy.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            observer=module.UndoRollbackCommitObserver(port,enrollment_fault or 'observe');env['SEATA_MYSQL_PORT']=str(observer.port)
            if enrollment_fault:env['SEATA_PREPARATION_FAULT']=enrollment_fault
            save('proxy-binding.json',dict(upstream_port=port,proxy_port=observer.port,role='observe original local prepare transaction and independent enrollment'))
        args=[binary]+([case,'--exact'] if case else [])+['--ignored','--nocapture','--test-threads=1']
        value=command(args,'live-tests.log',check=False,timeout=180)
        if red:need(value.returncode==101 and b'test result: FAILED.' in value.stdout and red_reason.encode() in value.stdout,'Expected target assertion did not fail')
        else:need(value.returncode==0 and re.search(rb'test result: ok\. [1-9]\d* passed; 0 failed;',value.stdout),'Actual preparation test failed')
        summaries=re.findall(r'test result: .*',value.stdout.decode(errors='replace'))
        for p in (work/'observations').rglob('*'):
            if p.is_file():put('observations/'+p.relative_to(work/'observations').as_posix(),p.read_bytes())
        if wire:
            probe='START TRANSACTION; CREATE TEMPORARY TABLE `__seata_rollback_warning_probe`(id INT) ENGINE=InnoDB; DROP TEMPORARY TABLE `__seata_rollback_warning_probe`; ROLLBACK; SHOW WARNINGS'
            save('temporary-warning-probe-command.json',dict(sql=probe,connection='independent docker exec native mysql client',affects_business=False));command(mysql+[probe],'temporary-warning-probe.tsv')
        command(mysql+['SELECT COUNT(*) FROM undo_log; SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID<>CONNECTION_ID() AND DB=DATABASE()'],'server-after.txt')
        status=True
    finally:
        observer_error=None
        try:
            if java_runtime:java_runtime.stop()
            if observer:wire_data=observer.stop();save('mysql-wire.json',wire_data)
        except Exception as error:
            observer_error=dict(type=type(error).__name__,message=str(error));save('observer-cleanup-error.json',observer_error)
        finally:
            if container:
                state=json.loads(command(['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==IMAGE and state['Config']['Labels'].get('seata-first-phase-prepare-owner')==owner,'Owned container differs')
                try:command(['docker','logs',container],'mysql.log')
                finally:
                    try:command(['docker','stop','--time','10',container],'stop.log')
                    finally:command(['docker','rm',container],'remove.log')
                need(command(['docker','inspect',container],check=False).returncode!=0,'Owned container remains')
            save('cleanup.json',dict(owned_container_removed=bool(container),observer_error=observer_error,D_data_retained=str(work/'data')))
        need(observer_error is None,'Observer cleanup failed after owned container cleanup')
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source input drift')
    need(sha(binary)==digest,'Executed binary drift');need(sha(child)==sha(out/'artifacts/mysql_rm_recovery_child.exe'),'Recovery child binary drift')
    if java:
        observation=json.loads((out/'observations/java-first-phase-preparation.json').read_bytes());java_peer=java_module.validate(out/'java-tc',observation);save('java-tc/validation.json',java_peer);save('java-tc/negative-controls.json',java_module.negative_controls(out/'java-tc',observation))
    if wire:
        spec=importlib.util.spec_from_file_location('preparation_sql_validator',HERE/'validate.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        observation=json.loads((out/'observations/first-phase-positive.json').read_bytes());save('wire-validation.json',module.validate(wire_data,observation,(out/'temporary-warning-probe.tsv').read_text('utf-8')));save('wire-negative-controls.json',module.negative_controls(wire_data,observation,(out/'temporary-warning-probe.tsv').read_text('utf-8')))
    if enrollment_fault:
        spec=importlib.util.spec_from_file_location('preparation_enrollment_validator',HERE/'validate.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        observation=json.loads((out/'observations/prepare-enrollment-fault.json').read_bytes());save('enrollment-wire-validation.json',module.validate_enrollment_fault(wire_data,observation));save('enrollment-wire-negative-controls.json',module.enrollment_negative_controls(wire_data,observation))
    save('result.json',dict(status='EXPECTED_TARGET_BEHAVIOR_RED' if red else 'SELECTED_FIRST_PHASE_PREPARATION_OBSERVATIONS_VERIFIED',verified=status,Rust=rust,selected=case,enrollment_fault=enrollment_fault,
        source_inputs=len(frozen),wire_observation=wire,selected_DB_target=target_name,actual_test_summary=summaries,Java_peer=java_peer,public_COMMIT_available=False,TC_registration_gate_passed=False,SQL_Undo_commit_atomicity_verified=False,WP05_complete=False))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));receipt=dict(run=str(out),catalog_sha256=sha(out/'catalog.json'));print(json.dumps(receipt),flush=True);return receipt
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85'],default='1.99');parser.add_argument('--target',choices=['live_callback_ownership','live_rm_recovery','live_undo_fence','live_undo_rollback']);parser.add_argument('--case');parser.add_argument('--red',action='store_true');parser.add_argument('--red-reason',default='actual error=');parser.add_argument('--java',action='store_true');parser.add_argument('--wire',action='store_true');parser.add_argument('--enrollment-fault',choices=['drop_ok','block_commit','delay_ok']);args=parser.parse_args();run(args.rust,args.case,args.red,args.red_reason,args.target,args.java,args.wire,args.enrollment_fault)
