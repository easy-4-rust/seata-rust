"""Execute faulty dynamic-report/RM production sources only in an isolated D workspace and target."""
import datetime
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time
import uuid
import zipfile

from inventory_rebuilt import ROOT, BASELINE, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD


def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-4/evidence/tcc-managed'/('dynamic-mutations_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-tcc-dynamic-mutations-'+stamp);work.mkdir();source=work/'source';source.mkdir()
    shutil.copytree(ROOT/'crates',source/'crates')
    for name in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/name,source/name)
    (work/'data').mkdir()
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config',SEATA_MYSQL_PASSWORD=PASSWORD)
    with zipfile.ZipFile(BASELINE/'portable_inputs_20261127/runtime-inputs.zip') as archive:undo=archive.read('stage/source/script/client/at/db/mysql.sql')
    (work/'undo.sql').write_bytes(undo);env['SEATA_UNDO_SCHEMA_PATH']=str(work/'undo.sql')
    with socket.socket() as probe:probe.bind(('127.0.0.1',0));port=probe.getsockname()[1]
    owner=uuid.uuid4().hex;database='dynamic_mut_'+owner[:12];env.update(SEATA_MYSQL_PORT=str(port),SEATA_MYSQL_DATABASE=database)
    inputs=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__)]):
            name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());inputs.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',inputs)
    commands=[]
    def command(label,argv,cwd=ROOT,expected=0,input=None):
        value=subprocess.run(list(map(str,argv)),cwd=cwd,env=env,input=input,capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+'.stdout')).write_bytes(value.stdout);(out/(label+'.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label,argv=list(map(str,argv)),cwd=str(cwd),exit_code=value.returncode));save(out/'commands.json',commands)
        if expected is not None:need(value.returncode==expected,label+' failed: '+(value.stdout+value.stderr)[-1500:].decode(errors='replace'))
        return value
    def build(version,role,label):
        argv=['cargo','+'+version+'.0','test','-p','seata-rm' if role=='rm' else 'seata-db-mysql']
        argv+=['--lib'] if role=='rm' else ['--features','tcc','--test','live_tcc']
        value=command(label,argv+['--offline','--locked','--no-run','--message-format=json'],source)
        rows=[Path(row['executable']) for line in value.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable')]
        need(len(rows)==1,'Unique isolated counterexample artifact missing');return rows[0]
    normal='dynamic_cases::full_context_reports_keep_original_receipt_and_recover_old_pending_with_new_callback'
    errors='dynamic_cases::report_rejection_loss_caught_failure_and_cancellation_never_commit_try'
    integrity='dynamic_integrity_cases::modified_report_versions_never_reuse_completed_and_original_bytes_restore_same_branch'
    rm_test='manager::callback_identity_tests::callback_identity_cannot_change_branch_or_resource_and_is_checked_before_completed_reuse'
    mutations=[
        ('checksum_ignored','crates/seata_db_mysql/src/mysql_tcc_context_store.rs','if hash.as_slice() != Self::checksum(source, &identity, revision, &app)? {','if false && hash.as_slice() != Self::checksum(source, &identity, revision, &app)? {','mysql',integrity),
        ('report_error_ignored','crates/seata_db_mysql/src/mysql_tcc_context_reporter.rs','            .await?;\n        self.last_data','            .await.ok();\n        self.last_data','mysql',errors),
        ('AT_status_in_TCC_report','crates/seata_db_mysql/src/mysql_tcc_context_reporter.rs','status: BranchStatus::Registered','status: BranchStatus::PhaseOneDone','mysql',normal),
        ('protocol_downgrade_accepted','crates/seata_db_mysql/src/mysql_tcc_fence_store.rs','|| version != protocol_version','|| !matches!(version,1|2)','mysql',integrity),
        ('cached_completion_unverified','crates/seata_rm/src/manager/branch_dispatcher.rs','resource.verify_callback_completion(receipt).await?;','let _ = receipt;','mysql',integrity),
        ('stable_identity_key_bypass','crates/seata_rm/src/manager/branch_dispatcher.rs','if stable.key() != identity.key() || stable.resource() != identity.resource() {','if false {','rm',rm_test),
    ]
    records=[];container=None
    print('TCC_DYNAMIC_MUTATIONS '+str(out),flush=True)
    try:
        container=command('create',['docker','create','--pull=never','--name','seata-dynamic-mutations-'+stamp.replace('_','-'),'--label','seata.dynamic.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql','-p','127.0.0.1:'+str(port)+':3306','-e','MYSQL_ROOT_PASSWORD='+PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,IMAGE]).stdout.decode().strip()
        command('start',['docker','start',container]);mysql=['docker','exec','-e','MYSQL_PWD='+PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e'];limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:need(time.monotonic()<limit,'Owned counterexample database startup timeout');time.sleep(.2)
        command('undo-schema',['docker','exec','-i','-e','MYSQL_PWD='+PASSWORD,container,'mysql','-h127.0.0.1','-uroot',database],input=undo)
        for version in ['1.99','1.85']:
            env['CARGO_TARGET_DIR']=str(work/('target-'+version))
            native=build(version,'mysql',version+'-baseline-build');rm=build(version,'rm',version+'-baseline-rm-build')
            for index,test in enumerate([normal,errors,integrity,rm_test]):
                observations=work/('baseline-'+version+'-'+str(index));observations.mkdir();env['SEATA_XA_OUT']=str(observations)
                argv=[rm if test==rm_test else native,test,*([] if test==rm_test else ['--ignored']),'--exact','--nocapture']
                value=command(version+'-baseline-'+str(index),argv,source);need(b'test result: ok. 1 passed;' in value.stdout,'Isolated behavioral baseline did not execute')
            for name,relative,needle,replacement,role,test in mutations:
                path=source/relative;original=path.read_bytes();need(original.decode().count(needle)==1,'Dynamic mutation target drift: '+name)
                prefix=version+'-'+name
                try:
                    path.write_bytes(original.decode().replace(needle,replacement).encode());(out/(prefix+'.rs')).write_bytes(path.read_bytes())
                    executable=build(version,role,prefix+'-build');observations=work/prefix;observations.mkdir();env['SEATA_XA_OUT']=str(observations)
                    argv=[executable,test,*([] if role=='rm' else ['--ignored']),'--exact','--nocapture']
                    value=command(prefix,argv,source,101);need(b'test result: FAILED. 0 passed; 1 failed;' in value.stdout and test.encode() in value.stdout,'Compiler/setup failure counted as behavioral counterexample')
                    records.append(dict(Rust=version,mutation=name,path=relative,modified_source_sha256=sha(path),artifact_sha256=sha(executable),test=test,exit_code=value.returncode));save(out/'records.json',records)
                finally:path.write_bytes(original)
    finally:
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==IMAGE and state['Config']['Labels'].get('seata.dynamic.owner')==owner,'Owned counterexample database changed')
            command('stop',['docker','stop','--time','10',container]);command('remove',['docker','rm',container]);need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Counterexample database remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),isolated_source=str(source),isolated_targets=[str(work/('target-'+v)) for v in ['1.99','1.85']]))
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Live source changed during isolated report counterexamples')
    save(out/'result.json',dict(status='DYNAMIC_REPORT_FAULTY_PRODUCTION_SOURCES_REJECTED',variants_per_version=6,actual_behavioral_failures=12,live_source_mutated=False,TCC_full_gate_complete=False,whole_plan_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':run()
