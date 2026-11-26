"""Owned MySQL Undo-store tests with frozen inputs and exact executed artifacts; no public COMMIT claim."""
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
def run(rust,case,red,red_reason,library):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/mysql-undo-store'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata_undo_store_'+stamp);(work/'data').mkdir(parents=True);(work/'init').mkdir()
    owner=uuid.uuid4().hex;database='undo_'+owner[:12];name='seata-undo-store-'+stamp.replace('_','-')
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',DOCKER_CONFIG='D:/Temp/seata-docker-config',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    cargo='cargo'
    if rust=='1.85':
        tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';env.update(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
    def put(leaf,data):
        p=out/leaf;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(leaf,data):put(leaf,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,leaf=None,check=True,timeout=180):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env,creationflags=subprocess.CREATE_NO_WINDOW,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
        if leaf:put(leaf,v.stdout)
        if check:need(v.returncode==0,(leaf or str(args[0]))+' failed '+v.stdout[-1000:].decode(errors='replace'))
        return v
    print('MYSQL_UNDO_STORE_RUN '+str(out),flush=True)
    need(sha(BASE/'catalog.json')=='58b48297821a70c461f6bca502a6597c9f83c5f8c4cd713b8b40b395c31eede2','Dependency authority differs')
    paths={Path(__file__),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-undo-store-contract.md',JAVA/'script/client/at/db/mysql.sql'}
    paths.update([HERE/'java_peer.py',HERE/'UndoStoreOracle.java',ROOT/'tools/formal-undo-format/runtime_binding.py'])
    paths.update(p for p in (ROOT/'crates').rglob('*') if p.is_file())
    for row in json.loads((BASE/'source-inputs.json').read_bytes()):
        if '/registry/' in row['path'].replace('\\','/'):
            p=Path(row['path']);need(sha(p)==row['sha256']==sha(BASE/row['snapshot']),'Frozen dependency source/archive drift');paths.add(p)
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),sha256=sha(p),snapshot=leaf))
    save('source-inputs.json',frozen)
    command([env.get('RUSTC','rustc'),'--version','--verbose'],'rust-version.txt')
    target=['--lib'] if library else ['--test','live_undo_store']
    build=command([cargo,'test','--locked','--offline','-p','seata-db-mysql']+target+['--no-run','--message-format=json'],'build.jsonl').stdout
    artifacts=[]
    for line in build.splitlines():
        try:r=json.loads(line)
        except ValueError:continue
        if r.get('reason')=='compiler-artifact' and r['target']['name']==('seata_db_mysql' if library else 'live_undo_store') and r.get('executable'):artifacts.append(Path(r['executable']))
    need(len(artifacts)==1,'Missing actual test executable');binary=artifacts[0];digest=sha(binary);put('artifacts/live_undo_store.exe',binary.read_bytes())
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    env.update(SEATA_MYSQL_PORT=str(port),SEATA_MYSQL_DATABASE=database,SEATA_MYSQL_PASSWORD=PASSWORD,SEATA_UNDO_STORE_OUT=str(work/'observations'))
    (work/'observations').mkdir()
    need(command(['git','-C',JAVA,'rev-parse','HEAD']).stdout.decode().strip()=='cba73e180fd8896a99df6415b859d275122feaa7','Java pin differs')
    schema=command(['git','-C',JAVA,'show','cba73e180fd8896a99df6415b859d275122feaa7:script/client/at/db/mysql.sql']).stdout
    need((JAVA/'script/client/at/db/mysql.sql').read_bytes().replace(b'\r\n',b'\n')==schema.replace(b'\r\n',b'\n'),'Original schema source differs')
    (work/'init/00-undo.sql').write_bytes(schema);put('original-undo.sql',schema);env['SEATA_UNDO_SCHEMA_PATH']=str(work/'init/00-undo.sql')
    container=None;status=False;java_peer=None
    try:
        container=command(['docker','create','--pull=never','--name',name,'--label','seata-undo-store-owner='+owner,
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
        args=[binary]+([case,'--exact'] if case else [])+['--ignored','--nocapture','--test-threads=1']
        value=command(args,'live-tests.log',check=False,timeout=180)
        if red:
            need(value.returncode==101 and b'test result: FAILED.' in value.stdout and red_reason.encode() in value.stdout,'Expected real target assertion did not fail')
        else:need(value.returncode==0 and re.search(rb'test result: ok\. [1-9]\d* passed; 0 failed;',value.stdout),'Actual Undo store test failed')
        for p in (work/'observations').rglob('*'):
            if p.is_file():put('observations/'+p.relative_to(work/'observations').as_posix(),p.read_bytes())
        if not red and not library and case is None:
            spec=importlib.util.spec_from_file_location('undo_store_java_peer',HERE/'java_peer.py');peer=importlib.util.module_from_spec(spec);spec.loader.exec_module(peer)
            java_peer=peer.run(out,work,env);save('java-peer.json',java_peer)
        command(mysql+['SELECT COUNT(*) FROM undo_log; SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID<>CONNECTION_ID() AND DB=DATABASE()'],'server-after.txt')
        status=True
    finally:
        if container:
            state=json.loads(command(['docker','inspect',container]).stdout)[0];need(state['Id']==container and state['Image']==IMAGE and state['Config']['Labels'].get('seata-undo-store-owner')==owner,'Owned container differs')
            command(['docker','logs',container],'mysql.log');command(['docker','stop','--time','10',container],'stop.log');command(['docker','rm',container],'remove.log')
            need(command(['docker','inspect',container],check=False).returncode!=0,'Owned container remains')
        save('cleanup.json',dict(owned_container_removed=bool(container),D_data_retained=str(work/'data')))
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source input drift')
    need(sha(binary)==digest,'Executed binary drift')
    save('result.json',dict(status='EXPECTED_MISSING_STORE_RED' if red else 'SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED',verified=status,Rust=rust,selected=case,
        source_inputs=len(frozen),actual_test_summary=re.findall(r'test result: .*',value.stdout.decode(errors='replace')),Java_peer=java_peer,public_COMMIT_available=False,TC_registration_gate_passed=False,SQL_Undo_commit_atomicity_verified=False,WP05_complete=False))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85'],default='1.99');parser.add_argument('--case');parser.add_argument('--red',action='store_true');parser.add_argument('--library',action='store_true');parser.add_argument('--red-reason',default='Undo flush persistence');args=parser.parse_args();run(args.rust,args.case,args.red,args.red_reason,args.library)
