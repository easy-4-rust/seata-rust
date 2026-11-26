"""Pinned original JavaTC + formal Rust RM/TM, exact-frame duplicate/reconnect scenarios; no SQL/Undo claim."""
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

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent


def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec);sys.modules[name]=value;spec.loader.exec_module(value);return value


support=module('formal_rm_java_process_support',ROOT/'tools/formal-transport-loss/live.py')
validator=module('formal_rm_java_observer',HERE/'validate.py')
reg=support.reg
sha,put,save,command=support.sha,support.put,support.save,support.command
ALL_CASES=[mode+'-'+op for mode in ['basic','duplicate','reconnect'] for op in ['commit','rollback']]


def run(selected):
    if not selected or len(set(selected))!=len(selected) or any(case not in ALL_CASES for case in selected):raise ValueError('Invalid unique case selection')
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
        LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    jar=reg.ORACLE/'server/target/seata-server.jar'
    if command(['git','-C',reg.ORACLE,'rev-parse','HEAD'],env).decode().strip()!=reg.PIN or sha(jar)!=reg.JAR_SHA:raise ValueError('Pinned Java authority differs')
    historical=json.loads(reg.HISTORICAL.read_bytes());compiled=reg.FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class';service=reg.FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler'
    if sha(compiled)!=historical['extension_class_sha256'] or sha(service)!=historical['extension_service_sha256']:raise ValueError('Public runtime SPI differs')
    for port in [38091,38092,38093]:
        with socket.socket() as check:check.bind(('127.0.0.1',port))
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/rm-java'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-formal-rm-'+stamp);work.mkdir();print('RM_JAVA_RUN '+str(out),flush=True)
    extension=work/'fixture'
    for path in [compiled,service]:put(extension/path.relative_to(reg.FIXTURE),path.read_bytes())
    manifest=HERE/'rust-client/Cargo.toml';config_source=ROOT/'tools/java-tc-integration/config/application.yml'
    paths={Path(__file__),Path(support.__file__),Path(reg.__file__),compiled,service,reg.HISTORICAL,config_source,ROOT/'Cargo.toml',ROOT/'Cargo.lock'}
    paths.update(path for path in HERE.rglob('*') if path.is_file() and path.suffix in ['.py','.rs','.toml','.lock'])
    paths.update(path for path in extension.rglob('*') if path.is_file())
    paths.update(path for path in (ROOT/'crates').rglob('*') if path.is_file())
    runtime_gate=ROOT/'docs/phase-1/evidence/rm-runtime/run_20261114_003214_ee6afbb3'
    if sha(runtime_gate/'catalog.json')!='66e1711307ef6a78c94ed1bde0acdd9e1ce1492bb035d6fbf133d9599490cad4':raise ValueError('Formal runtime gate catalog differs')
    paths.update([runtime_gate/'catalog.json',runtime_gate/'source-inputs.json',runtime_gate/'result.json'])
    for row in json.loads((runtime_gate/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if '/registry/' in path.as_posix() or path.is_relative_to(ROOT/'crates') and (path.suffix=='.rs' or path.name in ['Cargo.toml','Cargo.lock']):
            if sha(path)!=row['sha256']:raise ValueError('Compiled/dependency input differs from component gate: '+str(path))
            paths.add(path)
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save(out/'source-inputs.json',frozen)
    command(['cargo','build','--locked','--offline','--manifest-path',manifest],env,out/'build.log')
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/seata-rm-live-validation.exe';binary_sha=sha(binary);put(out/'artifacts/seata-rm-live-validation.exe',binary.read_bytes())
    command(['cargo','fmt','--manifest-path',manifest,'--','--check'],env,out/'fmt.log')
    command(['cargo','clippy','--locked','--offline','--manifest-path',manifest,'--all-targets','--','-D','warnings'],env,out/'clippy.log')
    command(['cargo','test','--locked','--offline','--manifest-path',manifest],env,out/'fixture-tests.log')
    py_tests=command([sys.executable,'-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],env,out/'python-tests.log')
    py_count=re.search(rb'Ran (\d+) tests',py_tests)
    if not py_count:raise ValueError('Python verifier test count missing')
    completed=[];cleanup=[];launched=[];proxy_launched=[];tc=None;proxy=None;tm_proxy=None
    try:
        for case_name in selected:
            mode,op=case_name.split('-');case=out/case_name;case.mkdir();runtime=work/case_name;runtime.mkdir();nonce=uuid.uuid4().hex
            config=config_source.read_text('utf-8').replace('18091','38091').replace('enable-check-auth: false','enable-check-auth: true')
            put(runtime/'application.yml',config);put(case/'actual-runtime-application.yml',config)
            frozen.append(dict(path=str(runtime/'application.yml'),sha256=sha(runtime/'application.yml'),snapshot=case_name+'/actual-runtime-application.yml'))
            runtime_env=env|dict(SEATA_TC_WORK_DIR=runtime.as_posix(),SEATA_RM_MODE=op,SEATA_RM_NONCE=nonce,SEATA_RM_TEST_DIR=runtime.as_posix(),SEATA_RM_ENDPOINT='127.0.0.1:38092',SEATA_TM_ENDPOINT='127.0.0.1:38093')
            args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),
                '-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
            stop_file=runtime/'stop-proxy'
            print('CASE_START '+case_name,flush=True)
            try:
                tc=support.spawn(args,runtime,runtime_env,case,'tc');launched.append(dict(case=case_name,pid=tc.pid));support.wait_owned(tc,38091,env)
                proxy=support.spawn([sys.executable,HERE/'rm_proxy.py','--listen-port','38092','--upstream-port','38091','--mode',mode,'--events',case/'events.jsonl','--stop-file',stop_file],runtime,runtime_env,case,'proxy');proxy_launched.append(dict(case=case_name,pid=proxy.pid));support.wait_owned(proxy,38092,env)
                tm_proxy=support.spawn([sys.executable,HERE/'rm_proxy.py','--listen-port','38093','--upstream-port','38091','--mode','basic','--role','TM','--events',case/'tm-events.jsonl','--stop-file',stop_file],runtime,runtime_env,case,'tm-proxy');proxy_launched.append(dict(case=case_name,pid=tm_proxy.pid,role='TM'));support.wait_owned(tm_proxy,38093,env)
                save(case/'owned-processes.json',dict(tc_pid=tc.pid,proxy_pid=proxy.pid,tm_proxy_pid=tm_proxy.pid,tc_command=[str(value) for value in args],runtime=str(runtime),nonce=nonce,observer_identity_given_to_client=False))
                command([binary],runtime_env,case/'client.log',runtime,timeout=45)
                put(stop_file,'complete\n');proxy.wait(timeout=10);tm_proxy.wait(timeout=10)
                if proxy.returncode or tm_proxy.returncode:raise ValueError('Owned RM/TM proxy failed')
                journal=runtime/'branches.log';put(case/'branches.log',journal.read_bytes())
                rows=[json.loads(line) for line in (case/'events.jsonl').read_text('utf-8').splitlines()]
                tm_rows=[json.loads(line) for line in (case/'tm-events.jsonl').read_text('utf-8').splitlines()]
                client_text=(case/'client.log').read_text('utf-8');result=validator.validate(case_name,client_text,rows,nonce,journal.read_bytes())
                # 原TC批日志异步输出；只等待日志落地，不发新业务请求或推断Finished为历史成功。
                until=time.monotonic()+3
                while True:
                    tc_bytes=(case/'tc-stdout.log').read_bytes()
                    if len(re.findall(rb'receive msg\[single\]: GlobalBeginRequest',tc_bytes))>=2 and len(re.findall(rb'receive msg\[single\]: Global(?:Commit|Rollback)Request',tc_bytes))>=2:break
                    if time.monotonic()>=until:raise ValueError('Original TC request log did not drain')
                    time.sleep(.05)
                result['independent_TM']=validator.validate_tm(result,client_text,tm_rows,rows,tc_bytes,nonce)
                loaded={match[1]:match[2].replace('\\','/') for match in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(runtime/'class-load.log').read_text('utf-8'))};origins={}
                checker='org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler'
                if extension.as_posix() not in loaded.get(checker,''):raise ValueError('Actual runtime auth checker differs')
                origins[checker]=loaded[checker]
                for name in ['org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.core.rpc.processor.server.RegRmProcessor','org.apache.seata.core.rpc.netty.ChannelManager',
                    'org.apache.seata.server.coordinator.DefaultCore','org.apache.seata.server.coordinator.AbstractCore','org.apache.seata.server.auth.AbstractCheckAuthHandler','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.AbstractTCInboundHandler']:
                    origin=loaded.get(name,'')
                    if jar.as_posix() not in origin or (('seata-core-' not in origin) if '.core.' in name else ('BOOT-INF/classes' not in origin)):raise ValueError('Critical TC class differs: '+name)
                    origins[name]=origin
                save(case/'class-origins.json',origins);result.update(tc_pid=tc.pid,proxy_pid=proxy.pid,nonce=nonce,class_origins=len(origins))
                save(case/'verified.json',result);completed.append(result);print('CASE_PASS '+case_name,flush=True)
            finally:
                if not stop_file.exists():put(stop_file,'cleanup\n')
                for owned_proxy in [proxy,tm_proxy]:
                    if owned_proxy and owned_proxy.poll() is None:
                        try:owned_proxy.wait(timeout=5)
                        except subprocess.TimeoutExpired:support.stop(owned_proxy)
                support.stop(tc)
                for port in [38091,38092,38093]:
                    if support.listeners(port,env):cleanup.append(dict(case=case_name,port=port,reason='owned service listener remains'))
                for leaf in ['class-load.log','branches.log']:
                    if (runtime/leaf).exists() and not (case/leaf).exists():put(case/leaf,(runtime/leaf).read_bytes())
                tc=None;proxy=None;tm_proxy=None
    finally:
        support.stop(proxy);support.stop(tm_proxy);support.stop(tc)
        save(out/'source-inputs-final.json',frozen)
        save(out/'result.json',dict(succeeded=False,cases=completed,selected=selected,cleanup_errors=cleanup,java_pin=reg.PIN,jar_sha256=reg.JAR_SHA,
            new_java_compilation=False,new_java_unit_tests=False,actual_TC_process_launches=launched,actual_proxy_process_launches=proxy_launched,verified_TC_cases=len(completed),fixture_Rust_tests=2,Python_tests=int(py_count[1]),SQL_Undo_verified=False,WP04_complete=False,full_V1_complete=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    if len(completed)!=len(selected) or cleanup:raise ValueError('Selected Java RM cases/cleanup incomplete')
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Source/runtime input drift: '+row['path'])
    if sha(jar)!=reg.JAR_SHA or sha(binary)!=binary_sha:raise ValueError('Java JAR/Rust binary drift')
    save(out/'test-artifact.json',dict(path=str(binary),sha256=binary_sha,snapshot='artifacts/seata-rm-live-validation.exe'))
    final=json.loads((out/'result.json').read_bytes());final.update(succeeded=True,input_closure_verified=True,all_six_Java_RM_cases_verified=set(selected)==set(ALL_CASES))
    (out/'result.json').write_text(json.dumps(final,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    members=[dict(path=path.relative_to(out).as_posix(),sha256=sha(path),bytes=path.stat().st_size) for path in sorted(out.rglob('*')) if path.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--cases',default='basic-commit,basic-rollback');args=parser.parse_args();run(args.cases.split(','))
