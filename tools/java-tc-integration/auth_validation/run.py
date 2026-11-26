"""Live native V1 auth SPI, default-policy controls and two-resource reconnect; unchanged pinned TC JAR."""
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
import uuid
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('seata_backend_harness_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
support=importlib.util.module_from_spec(spec);spec.loader.exec_module(support)
TYPE='Phase0CredentialCheckAuthHandler'
PROVIDER='org.apache.seata.server.auth.'+TYPE


def run(args):
    build=Path(args.java_build);oracle=Path(args.java_source);base=Path(args.runtime_base)
    if any(not re.match(r'^[DE]:[/\\]',str(path)) for path in [build,base]):raise ValueError('Development must remain on D/E')
    if support.command(['git','-C',oracle,'rev-parse','HEAD']).stdout.decode().strip()!=support.PIN:raise RuntimeError('Java pin mismatch')
    jar=oracle/'server/target/seata-server.jar'
    if support.sha(jar)!=support.JAR_HASH:raise RuntimeError('Original TC JAR mismatch')
    with socket.socket() as check:check.bind(('127.0.0.1',28091))
    run_id=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    runtime=base/run_id;runtime.mkdir(parents=True,exist_ok=False)
    output=ROOT/'docs/phase-0/evidence/local-auth-registration'/('run_'+run_id);output.mkdir(parents=True,exist_ok=False)
    test=build/'test-suite/test-new-version'
    before=support.audit_build(oracle,build)
    for name in [TYPE,TYPE+'Test']:
        target=test/f'src/test/java/org/apache/seata/server/{name}.java'
        target.write_bytes((ROOT/f'tools/java-v1-golden/{name}.java').read_bytes())
        compiled=test/f'target/test-classes/org/apache/seata/server/auth/{name}.class'
        if compiled.exists():compiled.unlink()
    with (output/'java-tests.log').open('xb') as log:
        result=subprocess.run([str(support.MAVEN),'-B','-ntp','-s',str(support.SETTINGS),'-f',str(build/'pom.xml'),'-pl','test-suite/test-new-version','-am',
            '-Dtest='+TYPE+'Test','-Dsurefire.failIfNoSpecifiedTests=false','-Djava.io.tmpdir=D:/Temp','test'],
            env=support.ENV,stdout=log,stderr=subprocess.STDOUT,creationflags=support.FLAGS,timeout=300)
    if result.returncode:raise RuntimeError('Fresh Java auth policy tests failed')
    after=support.audit_build(oracle,build)
    support.write(output/'java-production-source-audit.json',json.dumps({'java_pin':support.PIN,'before_build':before,'after_build':after,'normalization':'CRLF to LF only'},indent=2)+'\n')
    report=test/f'target/surefire-reports/TEST-org.apache.seata.server.auth.{TYPE}Test.xml'
    suite=ET.parse(report).getroot()
    if suite.get('tests')!='7' or any(int(suite.get(key,'0')) for key in ['failures','errors','skipped']):raise RuntimeError('Fresh auth policy test inventory differs')
    support.write(output/'java-report.xml',report.read_bytes())
    for name in [TYPE,TYPE+'Test']:
        if (test/f'src/test/java/org/apache/seata/server/{name}.java').read_bytes()!=(ROOT/f'tools/java-v1-golden/{name}.java').read_bytes():raise RuntimeError('Reconcile tested formatter output before live run')
    classes=runtime/'auth-extension-classes'
    compiled=classes/f'org/apache/seata/server/auth/{TYPE}.class';compiled.parent.mkdir(parents=True)
    support.write(compiled,(test/f'target/test-classes/org/apache/seata/server/auth/{TYPE}.class').read_bytes())
    service=classes/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler';service.parent.mkdir(parents=True)
    support.write(service,PROVIDER+'\n')
    support.logged(['cargo','build','--manifest-path',ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'],output,'rust-build.log')
    probe=Path(support.ENV['CARGO_TARGET_DIR'])/'debug/seata-java-tc-it.exe'
    tc=None;client=None;profiles=[];success=False;cleanup_errors=[]
    try:
        for name,extension,enabled in [('default',False,True),('extension',True,True),('bypass',True,False)]:
            work=runtime/name;work.mkdir();directory=output/name;directory.mkdir()
            config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091')
            if enabled:config=config.replace('enable-check-auth: false','enable-check-auth: true')
            support.write(work/'application.yml',config);support.write(directory/'application.yml',config)
            env=support.ENV.copy();env.update(SEATA_TC_WORK_DIR=work.as_posix(),SEATA_TC_HOST='127.0.0.1',SEATA_TC_PORT='28091',SEATA_IT_HOLD_MS='0')
            command=[support.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix()]
            if extension:command+=['-Dloader.path='+classes.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher']
            else:command+=['-jar',jar]
            command+=['-h','127.0.0.1']
            tc=support.spawn(command,directory,'tc',work,env)
            support.wait_until(lambda:support.port_open(28091),90,'Owned auth TC startup',tc)
            store=work/'data/sessionStore/28091/root.data'
            if not store.exists() or store.stat().st_size!=0:raise RuntimeError('Fresh auth FILE store is not empty')
            if name!='extension':
                mode='--auth-default' if name=='default' else '--auth-bypass'
                result=subprocess.run([str(probe),mode],env=env,cwd=work,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=support.FLAGS,timeout=20)
                support.write(directory/'client.txt',result.stdout)
                if result.returncode or f'PASS AUTH_CONTROL mode={mode} roles=2 identified=true'.encode() not in result.stdout:raise RuntimeError('Auth control profile differs')
                if store.stat().st_size!=0:raise RuntimeError('Registration-only control unexpectedly allocated a global session')
                profiles.append(dict(profile=name,extension=extension,auth_enabled=enabled,identified=True,roles=2,global_store_bytes=0,tc_pid=tc.pid))
            else:
                result=subprocess.run([str(probe),'--auth-denied'],env=env,cwd=work,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=support.FLAGS,timeout=90)
                support.write(directory/'denied.txt',result.stdout)
                if result.returncode or result.stdout.count(b'AUTH_DENIED role=')!=16 or b'PASS AUTH_DENIED cases=16 noIdentifiedChannel=true' not in result.stdout:raise RuntimeError('Native denial/context matrix failed')
                if store.stat().st_size!=0:raise RuntimeError('Denied registration transaction stimulus allocated FILE session data')
                support.write(directory/'after-denial-store.json',json.dumps({'bytes':store.stat().st_size,'sha256':support.sha(store),'expected_denials':16,'global_begin_stimuli':16})+'\n')
                ready=work/'reconnect-ready.tsv';proceed=work/'reconnect-continue.txt'
                env.update(SEATA_IT_AUTH_RECONNECT_READY=str(ready),SEATA_IT_AUTH_RECONNECT_CONTINUE=str(proceed))
                client=support.spawn([probe,'--auth-accepted'],directory,'accepted',work,env)
                def ready_owned():
                    try:
                        values=ready.read_text(encoding='utf-8').strip().split('\t')
                        return len(values)==3 and values[0]==str(client.pid)
                    except FileNotFoundError:return False
                support.wait_until(ready_owned,15,'Owned reconnect readiness',client)
                pid,old,new=map(int,ready.read_text(encoding='utf-8').strip().split('\t'))
                if old==new or not (1<=old<=65535 and 1<=new<=65535):raise RuntimeError('Not a fresh RM TCP endpoint')
                def old_closed():
                    return any(b'remove channel:' in line and f"clientId='phase0-auth:127.0.0.1:{old}'".encode() in line for line in (directory/'tc-stdout.log').read_bytes().splitlines())
                support.wait_until(old_closed,5,'Owned TC observed old RM disconnect',tc)
                support.write(proceed,str(pid)+'\n')
                if client.wait(timeout=20)!=0:raise RuntimeError('Authenticated reconnect and old-branch callbacks failed')
                accepted=(directory/'accepted-stdout.log').read_text(encoding='utf-8')
                if 'PASS AUTH_ACCEPTED reconnect=true cachedResources=2 originalBranchCallbacks=2 branchReregistrations=0' not in accepted:raise RuntimeError('No complete reconnect proof')
                prepared=re.findall(r'AUTH_PREPARED xid=(\S+) branchId=(\d+) resource=(\S+)',accepted)
                callbacks=re.findall(r'AUTH_CALLBACK xid=(\S+) branchId=(\d+) resource=(\S+) kind=(\d+) requestId=(\d+) actions=1',accepted)
                if len(prepared)!=2 or len(callbacks)!=2 or {p[2] for p in prepared}!={'mysql://auth-a','mysql://auth-b'}:raise RuntimeError('Incomplete cached-resource routing')
                for (xid,branch,resource),callback in zip(prepared,callbacks):
                    if callback[:3]!=(xid,branch,resource):raise RuntimeError('Callback did not retain old branch identity')
                    verb='Commit' if callback[3]=='3' else 'Rollback'
                    needle=f'{verb} branch transaction successfully, xid = {xid} branchId = {branch}'.encode()
                    support.wait_until(lambda:needle in (directory/'tc-stdout.log').read_bytes(),5,'Actual TC callback receipt/completion',tc)
                support.write(directory/'reconnect-proof.json',json.dumps({'client_pid':pid,'old_rm_port':old,'new_rm_port':new,'old_disconnect_observed':True,'prepared':prepared,'callbacks':callbacks,'branch_reregistrations':0},indent=2)+'\n')
                profiles.append(dict(profile=name,extension=True,auth_enabled=True,denials=16,denied_global_store_bytes=0,authenticated_rm_connections=2,cached_resources=2,original_callbacks=2,branch_reregistrations=0,client_exit=client.returncode,tc_pid=tc.pid))
                client=None
            support.stop(tc);tc=None
            print('PASS live auth profile='+name,flush=True)
        success=True
    finally:
        for process in [client,tc]:
            try:support.stop(process)
            except Exception as error:cleanup_errors.append(str(error))
        if support.port_open(28091):cleanup_errors.append('Auth test port not released')
        result=dict(executed_at=datetime.datetime.now().astimezone().isoformat(),succeeded=success and not cleanup_errors,java_pin=support.PIN,jar_sha256=support.sha(jar),extension_class_sha256=support.sha(compiled),extension_service_sha256=support.sha(service),rust_sha256=support.sha(probe),java_report_sha256=support.sha(output/'java-report.xml'),profiles=profiles,cleanup_errors=cleanup_errors,port_released=not support.port_open(28091),scope='Actual native auth hook/flag/context and two cached-resource reconnect; default provider is permissive, custom checker is a loopback test fixture; no production credential/TLS/replay-security claim')
        support.write(output/'result.json',json.dumps(result,indent=2)+'\n')
        (output.parent/'latest_run.json').write_text(json.dumps({'evidence_directory':str(output),'runtime_directory':str(runtime),'succeeded':result['succeeded']},indent=2)+'\n',encoding='utf-8')
    if not result['succeeded']:raise RuntimeError('Auth validation incomplete')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--java-source',default='E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle');parser.add_argument('--java-build',default='D:/codex-builds/seata-java-exact-overlay-20261112');parser.add_argument('--runtime-base',default='D:/codex-builds/seata auth validation');run(parser.parse_args())
