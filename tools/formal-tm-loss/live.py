"""正式TM execute/recover对固定原JavaTC的三种成功响应丢失；RM只作内存模拟。"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import uuid
from validate import validate,support

ROOT=Path(__file__).resolve().parents[2]
reg=support.reg
sha,put,save,command=support.sha,support.put,support.save,support.command


def run():
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
        LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    jar=reg.ORACLE/'server/target/seata-server.jar'
    if command(['git','-C',reg.ORACLE,'rev-parse','HEAD'],env).decode().strip()!=reg.PIN or sha(jar)!=reg.JAR_SHA:raise ValueError('Original Java pin/JAR differs')
    historical=json.loads(reg.HISTORICAL.read_bytes());compiled=reg.FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class';service=reg.FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler'
    if sha(compiled)!=historical['extension_class_sha256'] or sha(service)!=historical['extension_service_sha256']:raise ValueError('Public runtime auth fixture differs')
    for port in [38091,38092]:
        with socket.socket() as check:check.bind(('127.0.0.1',port))
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/tm-loss-java'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-formal-tm-loss-'+stamp);work.mkdir();print('TM_LOSS_JAVA_RUN '+str(out),flush=True)
    extension=work/'fixture'
    for path in [compiled,service]:put(extension/path.relative_to(reg.FIXTURE),path.read_bytes())
    config_source=ROOT/'tools/java-tc-integration/config/application.yml'
    paths=[Path(support.__file__),Path(reg.__file__),reg.HISTORICAL,compiled,service,config_source,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'tools/formal-transport-loss/loss_proxy.py',ROOT/'tools/formal-transport-loss/test_proxy.py']
    paths += list((ROOT/'tools/formal-tm-loss').glob('*.py'))+[p for p in extension.rglob('*') if p.is_file()]+[p for p in (ROOT/'crates').rglob('*') if p.is_file()]
    frozen=[]
    for path in sorted(set(paths)):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    command(['cargo','build','--offline','--locked','-p','seata-tm','--example','tm_loss_live'],env,out/'build.log')
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/tm_loss_live.exe';binary_sha=sha(binary);put(out/'artifacts/tm_loss_live.exe',binary.read_bytes())
    command(['python','-m','unittest','discover','-s','tools/formal-tm-loss','-p','test_validate.py','-v'],env,out/'validator-tests.log')
    command(['python','-m','unittest','discover','-s','tools/formal-transport-loss','-p','test_proxy.py','-v'],env,out/'proxy-tests.log')
    tc=None;proxy=None;cases=[];functional=False;cleanup=[]
    try:
        for mode,kind in [('begin',2),('commit',8),('rollback',10)]:
            case=out/mode;case.mkdir();runtime=work/mode;runtime.mkdir();nonce=uuid.uuid4().hex
            config=config_source.read_text(encoding='utf-8').replace('18091','38091').replace('enable-check-auth: false','enable-check-auth: true')
            put(runtime/'application.yml',config);put(case/'actual-runtime-application.yml',config)
            frozen.append(dict(path=str(runtime/'application.yml'),sha256=sha(runtime/'application.yml'),snapshot=mode+'/actual-runtime-application.yml'))
            save(case/'runtime-config-reference.json',frozen[-1])
            runtime_env=env|dict(SEATA_TC_WORK_DIR=runtime.as_posix(),SEATA_TM_LOSS_MODE=mode,SEATA_TM_LOSS_ENDPOINT='127.0.0.1:38092',SEATA_TM_LOSS_NONCE=nonce)
            args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),
                '-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
            tc=support.spawn(args,runtime,runtime_env,case,'tc');support.wait_owned(tc,38091,env)
            proxy=support.spawn(['python',ROOT/'tools/formal-transport-loss/loss_proxy.py','--listen-port','38092','--upstream-port','38091','--drop-response-kind',kind,'--events',case/'events.jsonl'],runtime,runtime_env,case,'proxy');support.wait_owned(proxy,38092,env)
            save(case/'owned-processes.json',dict(tc_pid=tc.pid,proxy_pid=proxy.pid,tc_command=[str(x) for x in args],runtime=str(runtime),nonce=nonce,observer_data_given_to_client=False))
            command([binary],runtime_env,case/'client.log',runtime,timeout=30)
            proxy.wait(timeout=10)
            if proxy.returncode:raise ValueError('Owned frame proxy failed')
            rows=[json.loads(line) for line in (case/'events.jsonl').read_text(encoding='utf-8').splitlines()]
            result=validate(mode,(case/'client.log').read_text(encoding='utf-8'),rows,nonce)
            loaded={match[1]:match[2].replace('\\','/') for match in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(runtime/'class-load.log').read_text(encoding='utf-8'))};origins={}
            checker='org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler'
            if extension.as_posix() not in loaded.get(checker,''):raise ValueError('Runtime checker not from frozen SPI')
            origins[checker]=loaded[checker]
            for name in ['org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.core.rpc.processor.server.RegRmProcessor','org.apache.seata.core.rpc.netty.ChannelManager',
                'org.apache.seata.server.coordinator.DefaultCore','org.apache.seata.server.coordinator.AbstractCore','org.apache.seata.server.auth.AbstractCheckAuthHandler']:
                origin=loaded.get(name,'')
                if jar.as_posix() not in origin or (('seata-core-' not in origin) if '.core.' in name else ('BOOT-INF/classes' not in origin)):raise ValueError('Critical runtime class differs:'+name)
                origins[name]=origin
            save(case/'class-origins.json',origins)
            result.update(tc_pid=tc.pid,proxy_pid=proxy.pid,actual_class_origins=7,nonce=nonce,simulated_RM_only=True)
            if mode=='begin':
                store=runtime/'data/sessionStore/38091/root.data'
                if not store.exists() or store.stat().st_size==0:raise ValueError('Lost successful Begin left no original FILE store evidence')
                put(case/'retained-root.data',store.read_bytes());result['retained_store_bytes']=(case/'retained-root.data').stat().st_size
            support.stop(proxy);support.stop(tc);put(case/'class-load.log',(runtime/'class-load.log').read_bytes())
            if support.listeners(38091,env) or support.listeners(38092,env):raise ValueError('Case owned ports not released')
            result['ports_released']=True;save(case/'result.json',result);cases.append(result);tc=None;proxy=None
        functional=True
    finally:
        support.stop(proxy);support.stop(tc)
        for port in [38091,38092]:
            if support.listeners(port,env):cleanup.append('port still listening:'+str(port))
        save(out/'source-inputs.json',frozen)
        save(out/'result.json',dict(succeeded=False,functional_cases_completed=functional,cleanup_errors=cleanup,input_closure_verified=False,cases=cases,
            java_pin=reg.PIN,jar_sha256=reg.JAR_SHA,formal_TM_execute_and_recover_API=True,observer_data_given_to_client=False,simulated_RM_only=True,SQL_Undo_verified=False,
            new_java_compilation=False,new_java_unit_tests=False,external_services_started=6 if functional else None,WP03_complete=False,full_V1_complete=False))
    if not functional or cleanup:raise ValueError('TM response loss/cleanup failed')
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Input drift:'+row['path'])
    if sha(jar)!=reg.JAR_SHA or sha(binary)!=binary_sha:raise ValueError('TC JAR/formal client binary drift')
    save(out/'test-artifact.json',dict(path=str(binary),sha256=binary_sha,snapshot='artifacts/tm_loss_live.exe'))
    final=json.loads((out/'result.json').read_bytes());final.update(succeeded=True,input_closure_verified=True)
    (out/'result.json').write_text(json.dumps(final,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
