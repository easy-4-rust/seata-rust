"""Owned MySQL + native TLS regression of persisted TM trust boundaries."""
import argparse
import datetime
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
import zipfile
from inventory_rebuilt import ROOT, need, save, sha
from mysql_undo_executor_live import IMAGE, PASSWORD


def run(versions, red, case=None, expected_failure=None):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_tm_trust_'+stamp)
    (work/'data').mkdir(parents=True)
    out=ROOT/'docs/phase-2/evidence/tm-transport-binding'/ (('red_' if red or expected_failure else 'run_')+stamp)
    out.mkdir(parents=True)
    print('TM_TRANSPORT_BINDING '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),
        CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',DOCKER_CONFIG='D:/Temp/seata-docker-config',
        SEATA_MYSQL_PASSWORD=PASSWORD)
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
        container=command('create',['docker','create','--pull=never','--name','seata-tm-trust-'+stamp.replace('_','-'),
            '--label','seata.tm.trust.owner='+owner,'--mount','type=bind,source='+(work/'data').as_posix()+',target=/var/lib/mysql',
            '-p','127.0.0.1:'+str(port)+':3306','-e','MYSQL_ROOT_PASSWORD='+PASSWORD,'-e','MYSQL_ROOT_HOST=%','-e','MYSQL_DATABASE='+database,IMAGE]).stdout.decode().strip()
        command('start',['docker','start',container])
        mysql=['docker','exec','-e','MYSQL_PWD='+PASSWORD,container,'mysql','-h127.0.0.1','-uroot','-N','-B','-e']
        limit=time.monotonic()+120
        while subprocess.run(mysql+['SELECT VERSION()'],env=env,capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW).returncode:
            need(time.monotonic()<limit,'Owned MySQL startup timeout');time.sleep(.3)
        for version in versions:
            target=ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185')
            build=command(version+'-build',['cargo','+'+version+'.0','test','-p','seata-db-mysql','--test','live_tm_transport_binding',
                '--features','transport_tls','--offline','--locked','--no-run','--message-format=json'],extra=dict(CARGO_TARGET_DIR=str(target)))
            artifacts=[Path(row['executable']) for line in build.stdout.splitlines() if line.startswith(b'{') and (row:=json.loads(line)).get('executable') and row['target']['name']=='live_tm_transport_binding']
            need(len(artifacts)==1,'Unique native fixture artifact missing')
            argv=[artifacts[0],'--ignored','--skip','binding_owner_child','--test-threads=1','--nocapture']
            if case:argv=[artifacts[0],case,'--ignored','--exact','--nocapture']
            observations=out/'observations'/version
            value=command(version+'-live',argv,expected=101 if red or expected_failure else 0,extra=dict(SEATA_TM_TRUST_OUT=str(observations)))
            if expected_failure:need(expected_failure.encode() in value.stdout+value.stderr,'Expected behavioral mutation failure missing')
            if red:need(b'TLS source cannot trust a plaintext replacement' in value.stdout+value.stderr,'RED did not expose persisted trust downgrade')
            save(out/(version+'-artifact.json'),dict(path=str(artifacts[0]),sha256=sha(artifacts[0])))
    finally:
        if container:
            state=json.loads(command('inspect',['docker','inspect',container]).stdout)[0]
            need(state['Id']==container and state['Image']==IMAGE and state['Config']['Labels'].get('seata.tm.trust.owner')==owner,'Owned runtime identity drift')
            command('mysql-log',['docker','logs',container])
            try:command('stop',['docker','stop','--time','10',container])
            finally:command('remove',['docker','rm',container])
            need(command('confirm-removed',['docker','inspect',container],expected=None).returncode!=0,'Owned container remains')
        save(out/'cleanup.json',dict(owned_container_removed=bool(container),D_data_retained=str(work)))
    for row in sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed source drift')
    save(out/'result.json',dict(status='PERSISTED_TLS_DOWNGRADE_REPRODUCED' if red else 'EXPECTED_TRUST_MUTATION_FAILURE_OBSERVED' if expected_failure else 'PERSISTED_TM_TRANSPORT_BINDING_NATIVE_VERIFIED',Rust_versions=versions,selected_case=case,expected_failure=expected_failure,WP07_complete=False))
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85']);parser.add_argument('--red',action='store_true');parser.add_argument('--case');parser.add_argument('--expect-failure');args=parser.parse_args()
    run([args.rust] if args.rust else ['1.99','1.85'],args.red,args.case,args.expect_failure)
