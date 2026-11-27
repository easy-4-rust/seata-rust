"""Owned MySQL native XA, actual original OK loss, independent SQL and physical restart."""
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
import zipfile
from inventory_rebuilt import BASELINE, ROOT, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD


def run(versions, red, case=None, expected_failure=None):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_structured_logs_mysql_'+stamp)
    (work/'data').mkdir(parents=True)
    out=ROOT/'docs/phase-4/evidence/xa-managed'/ (('mysql-red_' if red or expected_failure else 'mysql_')+stamp)
    out.mkdir(parents=True)
    with zipfile.ZipFile(BASELINE/'portable_inputs_20261127/runtime-inputs.zip') as archive:
        undo=archive.read('stage/source/script/client/at/db/mysql.sql')
    (work/'original-undo.sql').write_bytes(undo);(out/'original-undo.sql').write_bytes(undo)
    print('XA_MANAGED_MYSQL '+str(out),flush=True)
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
        value=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env|(extra or {}),capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-2200:].decode(errors='replace'))
        return value
    sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock']):
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
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--features','xa','--test','live_xa_managed',
                '--offline','--locked','--no-run','--message-format=json'],extra=dict(CARGO_TARGET_DIR=str(target)))
            artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_xa_managed']
            need(len(artifacts)==1,'Unique native fixture artifact missing')
            cases=[case] if case else ['registered_original_xa_prepares_and_completes_through_real_rm_callbacks','failed_or_lost_registration_cannot_start_mysql_xa','unsafe_claim_table_rejects_before_xa_start_and_keeps_original_receipt','bound_direction_closes_active_scope_and_absence_stays_unknown','persisted_original_identity_and_current_schema_guard_native_finish','changed_commit_durability_after_connect_blocks_xa_start','failure_cases::lost_native_commit_ok_preserves_rm_pending_and_does_not_replay','failure_cases::callback_before_registration_reply_fences_late_original_start','failure_cases::lost_native_rollback_ok_recovers_original_prepared_intent_without_replay','failure_cases::original_active_scope_is_fenced_by_independent_rm_without_caller_drop','failure_cases::rollback_callback_races_original_start_without_late_business']
            for index,selected in enumerate(cases):
                command(version+'-'+str(index)+'-enable-log',mysql+["SET GLOBAL general_log=OFF; SET GLOBAL log_output='TABLE'; TRUNCATE TABLE mysql.general_log; SET GLOBAL general_log=ON"])
                observations=work/'observations'/version/('case-'+str(index));observations.mkdir(parents=True)
                value=command(version+('-live' if case else '-case-'+str(index)),[artifacts[0],selected,'--ignored','--exact','--nocapture'],expected=101 if red or expected_failure else 0,extra=dict(SEATA_XA_OUT=str(observations)))
                need((b'test result: FAILED.' if red or expected_failure else b'test result: ok. 1 passed;') in value.stdout,'Managed XA test summary missing')
                if expected_failure:need(expected_failure.encode() in value.stdout+value.stderr,'Expected managed XA behavioral mutation failure missing')
                command(version+'-case-'+str(index)+'-independent-native-commands',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type='Query' AND argument LIKE 'XA %' ORDER BY event_time,thread_id"])
                command(version+'-case-'+str(index)+'-independent-business',mysql+['SELECT id,v FROM '+database+'.xa_managed ORDER BY id'])
                command(version+'-case-'+str(index)+'-independent-state',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_branch_v1 ORDER BY xid,branch_id'])
                command(version+'-case-'+str(index)+'-independent-prepared',mysql+['XA RECOVER CONVERT XID'])
                command(version+'-case-'+str(index)+'-independent-witness',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_commit_witness_v1 ORDER BY xid,branch_id'])
                command(version+'-case-'+str(index)+'-independent-intent',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_prepare_evidence_v1 ORDER BY xid,branch_id'])
                command(version+'-case-'+str(index)+'-independent-guard',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_session_guard_v1 ORDER BY xid,branch_id'])
                command(version+'-case-'+str(index)+'-independent-guard-enrollment',mysql+['SELECT HEX(xid),branch_id,phase,completed FROM '+database+'.seata_rs_xa_guard_enrollment_v1 ORDER BY xid,branch_id'])
                command(version+'-case-'+str(index)+'-independent-kill',mysql+["SELECT command_type,argument FROM mysql.general_log WHERE command_type='Query' AND argument LIKE 'KILL %' ORDER BY event_time,thread_id"])
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
    save(out/'result.json',dict(status='XA_MANAGED_MYSQL_BEHAVIOR_REPRODUCED' if red else 'EXPECTED_XA_MUTATION_FAILURE_OBSERVED' if expected_failure else 'MANAGED_XA_ORIGINAL_REGISTRATION_AND_CALLBACKS_VERIFIED',Rust_versions=versions,selected_case=case,expected_failure=expected_failure,XA_RM_gate_complete=False,whole_plan_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--red',action='store_true');parser.add_argument('--case');parser.add_argument('--expect-failure');args=parser.parse_args()
    run([args.rust] if args.rust else ['1.99','1.85'],args.red,args.case,args.expect_failure)
