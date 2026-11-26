"""Bind unchanged production inputs to fresh inter-query race evidence on two Rust versions."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

ROOT=Path(__file__).resolve().parents[2]
BASE=ROOT/'docs/phase-2/evidence/mysql-row/run_20261114_045819_c62f4e63'
BASE_CATALOG='10285b1534c237d5f1eab8944aa039851a70a9798eedd0f80a66bccdb1993ea6'
CASE='preflight_locks_prevent_row_growth_and_phantoms_before_full_query_is_forwarded'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(value,message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/mysql-image-race-gate'/('run_'+stamp)
    work=Path('D:/codex-builds')/('seata_mysql_image_race_'+stamp)
    out.mkdir(parents=True);work.mkdir()
    print('MYSQL_IMAGE_RACE_GATE_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    env.pop('RUSTC',None);env.pop('RUSTDOC',None);env.pop('SEATA_MYSQL_TARGET_DIR',None)

    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True)
        with p.open('xb') as f:f.write(data.encode('utf-8') if isinstance(data,str) else data)

    def save(name,data):
        put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')

    def command(args,name,cwd=ROOT,extra=None,exit_code=0):
        value=subprocess.run([str(v) for v in args],cwd=cwd,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,value.stdout);need(value.returncode==exit_code,name+' unexpected exit')
        return value.stdout

    need(sha(BASE/'catalog.json')==BASE_CATALOG,'Previous component catalog differs')
    for row in read(BASE/'catalog.json')['members']:need(sha(BASE/row['path'])==row['sha256'],'Previous sealed member differs')
    inputs=read(BASE/'source-inputs.json');unchanged_rust=[]
    paths={Path(row['path']) for row in inputs}
    for row in inputs:
        p=Path(row['path'])
        if (p.suffix=='.rs' and p.is_relative_to(ROOT/'crates')) or p.name in ['Cargo.toml','Cargo.lock'] or p.is_relative_to(Path(env['CARGO_HOME'])):
            need(sha(p)==row['sha256'],'Inherited compilation/dependency input changed: '+str(p))
        if p.suffix=='.rs' and p.is_relative_to(ROOT/'crates'):unchanged_rust.append(str(p.relative_to(ROOT)))
    need(len(unchanged_rust)==223,'Expected223 old Rust files including182 production files')
    paths.update([Path(__file__),ROOT/'crates/seata_db_mysql/tests/live_image_race.rs',ROOT/'crates/seata_db_mysql/tests/support/query_barrier_proxy.rs'])
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin'
        put(snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save('source-inputs.json',frozen)
    save('inherited-component.json',dict(path=str(BASE),catalog_sha256=BASE_CATALOG,members=10224,unchanged_Rust_files=223,
        unchanged_production_Rust_files=182,workspace_282_tests_per_toolchain_rerun=False,old_19_live_cases_per_toolchain_rerun=False,
        old_8_pure_and_5_live_controls_rerun=False,scope='Historical component evidence is inherited through unchanged production, old tests, Cargo and dependency source bytes. New race tests are additive.'))
    command(['cargo','fmt','-p','seata-db-mysql','--','--check'],'fmt.log')
    command(['cargo','clippy','--locked','--offline','-p','seata-db-mysql','--all-targets','--','-D','warnings'],'clippy.log')
    children=[]

    def packet(child,positive):
        log=(child/'live-tests.log').read_bytes()
        match=re.search(rb'query_barrier_packet_hex=([0-9a-f]+)',log)
        need(match is not None,'Actual held packet missing')
        raw=bytes.fromhex(match[1].decode());need(len(raw)>5,'Held packet truncated')
        need(int.from_bytes(raw[:3],'little')==len(raw)-4 and raw[3]==0 and raw[4]==0x16,'Actual MySQL frame header/command differs')
        sql=raw[5:].decode('utf-8')
        need(sql=='SELECT `id`, `payload` FROM `image_inter_query` WHERE id >= ? ORDER BY `id` LIMIT 1025 FOR UPDATE','Unexpected held original SQL')
        if positive:
            need(b'inter_query_barrier=true growth_blocked=1205 insert_blocked=1205 image_rows=1 payload_bytes=1 after_rollback_growth=1 after_rollback_insert=1' in log,'Actual schedule outcomes missing')
        return dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),hex=raw.hex(),command='COM_STMT_PREPARE',sql=sql)

    for rust in ['1.99','1.85']:
        print('REAL_IMAGE_RACE '+rust,flush=True)
        raw=command([sys.executable,ROOT/'tools/formal-mysql-foundation/live.py','--rust',rust,'--suite','image_race'],'live-'+rust+'.log')
        match=re.search(rb'MYSQL_IMAGE_RACE_RUN ([^\r\n]+)',raw);need(match is not None,'Actual child path missing')
        child=Path(match[1].decode());result=read(child/'result.json')
        need(result['succeeded'] and result['actual_live_tests']==1 and not result['cleanup_errors'],'Required actual race case missing')
        catalog=read(child/'catalog.json')['members']
        for row in catalog:need(sha(child/row['path'])==row['sha256'],'Actual positive child member differs')
        children.append(dict(path=str(child),rust=rust,actual_live_tests=1,members=len(catalog),catalog_sha256=sha(child/'catalog.json'),held_packet=packet(child,True)))
    save('live-children.json',children)
    controls=[]
    for name,reason in [('unlocked-preflight','row growth must be blocked before full value query'),('read-committed','phantom insert must be blocked before full value query')]:
        print('ACTUAL_IMAGE_RACE_ASSERTION '+name,flush=True)
        directory=work/name;directory.mkdir()
        for item in ['Cargo.toml','Cargo.lock']:shutil.copy2(ROOT/item,directory/item)
        shutil.copytree(ROOT/'crates',directory/'crates')
        (directory/'tools/formal-mysql-foundation').mkdir(parents=True)
        for item in ['live.py','schema.sql']:shutil.copy2(ROOT/'tools/formal-mysql-foundation'/item,directory/'tools/formal-mysql-foundation'/item)
        (directory/'docs/phase-2').mkdir(parents=True)
        for item in ['mysql-foundation-contract.md','mysql-row-read-contract.md']:shutil.copy2(ROOT/'docs/phase-2'/item,directory/'docs/phase-2'/item)
        path=directory/'crates/seata_db_mysql/src'/('mysql_image_reader.rs' if name=='unlocked-preflight' else 'data_source_proxy.rs')
        source=path.read_text(encoding='utf-8')
        if name=='unlocked-preflight':
            before='let preflight_sql = format!("SELECT {lengths}{suffix}");'
            need(source.count(before)==1,'Preflight mutation anchor differs')
            source=source.replace(before,'let preflight_sql = format!("SELECT {lengths}{}", suffix.trim_end_matches(" FOR UPDATE"));',1)
        else:
            need(source.count('REPEATABLE-READ')==2,'Both configure and profile checks must change in the RC control')
            source=source.replace('REPEATABLE-READ','READ-COMMITTED')
        path.write_text(source,encoding='utf-8')
        raw=command([sys.executable,directory/'tools/formal-mysql-foundation/live.py','--suite','image_race'],
            'controls/'+name+'/wrapper.log',cwd=directory,extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-mysql-image-race-controls'),exit_code=1)
        match=re.search(rb'MYSQL_IMAGE_RACE_RUN ([^\r\n]+)',raw);need(match is not None,'Negative child path missing')
        child=Path(match[1].decode());result=read(child/'result.json');log=(child/'live-tests.log').read_bytes()
        need(not result['succeeded'] and not result['cleanup_errors'] and b'test result: FAILED. 0 passed; 1 failed;' in log
            and b'panicked at crates' in log and reason.encode() in log and b'error[E' not in log,'Control must compile and fail intended custom assert')
        observed=packet(child,False)
        for row in read(child/'source-inputs.json'):need(sha(Path(row['path']))==row['sha256'] and sha(child/row['snapshot'])==row['sha256'],'Executed control source drift')
        for p in child.rglob('*'):
            if p.is_file():put('controls/'+name+'/child/'+p.relative_to(child).as_posix(),p.read_bytes())
        put('controls/'+name+'/executed.rs',path.read_bytes())
        controls.append(dict(name=name,target=CASE,actual_assertion_failed=True,assertion_message=reason,executed_source=str(path),
            snapshot='controls/'+name+'/executed.rs',sha256=sha(path),actual_child=str(child),held_packet=observed))
    save('controls.json',controls)
    need(len({x['held_packet']['sha256'] for x in children+controls})==1,'Positive/control held query packet changed')
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Frozen input drift during race verification')
    save('result.json',dict(status='verified_inter_query_race_schedule_pending_readonly_review',source_inputs=len(frozen),
        unchanged_old_Rust_files=223,unchanged_production_Rust_files=182,fresh_positive_live_tests_per_toolchain=1,Rust_versions=['1.99','1.85'],
        fresh_positive_live_artifacts=2,fresh_real_MySQL_assertion_controls=2,actual_owned_MySQL_containers=4,original_held_packet_bytes=children[0]['held_packet']['bytes'],
        inherited_282_workspace_and_19_live_cases_rerun=False,inherited_13_assertion_controls_rerun=False,
        inherited_dependency_archives=185,inherited_compiled_registry_packages=148,inherited_dependency_source_files=6745,
        inter_query_insert_and_growth_schedule_verified=True,production_code_changed=False,INSERT_images_complete=False,
        production_business_SQL_execution=False,after_images_complete=False,SQL_Undo_atomicity_verified=False,WP05_complete=False,
        full_V1_complete=False,release_authorized=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':
    run()
