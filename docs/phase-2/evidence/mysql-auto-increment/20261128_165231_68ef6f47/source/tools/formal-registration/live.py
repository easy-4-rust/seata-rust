"""正式注册池对未修改固定JavaTC及已认证公共SPI fixture的现场验证。"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import time
import uuid

ROOT=Path(__file__).resolve().parents[2]
ORACLE=Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
PIN='cba73e180fd8896a99df6415b859d275122feaa7'
JAR_SHA='aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5'
JAVA=Path('D:/Java/jdk-17.0.12/bin/java.exe')
FIXTURE=Path('D:/codex-builds/seata auth validation/20261112_161702_b4eda7e4/auth-extension-classes')
HISTORICAL=ROOT/'docs/phase-0/evidence/local-auth-registration/run_20261112_161702_b4eda7e4/result.json'


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as stream:stream.write(data.encode('utf-8') if isinstance(data,str) else data)
def save(path,data):put(path,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
def command(args,env,cwd=ROOT,timeout=60):
    result=subprocess.run([str(x) for x in args],cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=timeout)
    if result.returncode:raise RuntimeError(str(args[0])+': '+result.stdout[-2000:].decode(errors='replace'))
    return result.stdout


def run():
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
                        LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    if command(['git','-C',ORACLE,'rev-parse','HEAD'],env).decode().strip()!=PIN:raise ValueError('Java pin differs')
    jar=ORACLE/'server/target/seata-server.jar'
    if sha(jar)!=JAR_SHA:raise ValueError('Original Java JAR differs')
    historical=json.loads(HISTORICAL.read_bytes())
    compiled=FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'
    service=FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler'
    if not historical['succeeded'] or sha(compiled)!=historical['extension_class_sha256'] or sha(service)!=historical['extension_service_sha256']:raise ValueError('Cached fixture does not match prior certified artifacts')
    port=38091
    with socket.socket() as check:check.bind(('127.0.0.1',port))
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/registration-java'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-formal-registration-'+stamp);work.mkdir()
    print('REGISTRATION_JAVA_RUN '+str(out),flush=True)
    extension=work/'fixture'
    for path in [compiled,service]:put(extension/path.relative_to(FIXTURE),path.read_bytes())
    config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091',str(port)).replace('enable-check-auth: false','enable-check-auth: true')
    put(work/'application.yml',config);put(out/'application.yml',config)
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),HISTORICAL,compiled,service,ROOT/'tools/java-v1-golden/Phase0CredentialCheckAuthHandler.java',
           ROOT/'tools/java-tc-integration/config/application.yml',work/'application.yml']
    paths += [p for p in (ROOT/'crates').rglob('*') if p.is_file()]
    paths += [p for p in extension.rglob('*') if p.is_file()]
    frozen=[]
    for path in sorted(set(paths)):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save(out/'source-inputs.json',frozen)
    save(out/'jar-and-fixture-reference.json',dict(java_pin=PIN,jar=str(jar),jar_sha256=JAR_SHA,fixture_previous_result=str(HISTORICAL),
        fixture_class_sha256=sha(compiled),fixture_service_sha256=sha(service),new_java_compilation=False,new_java_unit_tests=False))
    put(out/'build.log',command(['cargo','build','--offline','--locked','-p','seata-transport','--example','registration_live'],env))
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/registration_live.exe';put(out/'artifacts/registration_live.exe',binary.read_bytes())
    binary_sha=sha(binary)
    runtime_env=env|dict(SEATA_TC_WORK_DIR=work.as_posix(),SEATA_REGISTRATION_ENDPOINT=f'127.0.0.1:{port}')
    args=[JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),
          '-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
    process=None;success=False;cleanup=[];origins={}
    def listeners():
        raw=command(['powershell','-NoProfile','-Command',f'@(Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue).OwningProcess | ConvertTo-Json -Compress'],env,timeout=10).decode('utf-8-sig').strip()
        if not raw:return []
        value=json.loads(raw);return value if isinstance(value,list) else [value]
    try:
        with (out/'tc-stdout.log').open('xb') as stdout,(out/'tc-stderr.log').open('xb') as stderr:
            process=subprocess.Popen([str(x) for x in args],cwd=work,env=runtime_env,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
        save(out/'owned-process.json',dict(pid=process.pid,command=[str(x) for x in args],work=str(work),port=port))
        deadline=time.monotonic()+90
        while True:
            if process.poll() is not None:raise RuntimeError('Owned TC exited during startup')
            owners=listeners()
            if owners:
                if set(owners)!={process.pid}:raise RuntimeError('TC listener owner differs')
                break
            if time.monotonic()>deadline:raise TimeoutError('Owned TC startup timed out')
            time.sleep(.2)
        put(out/'client.log',command([binary],runtime_env,cwd=work,timeout=25))
        raw=(out/'client.log').read_text(encoding='utf-8')
        expected=['TM_READY','TM_DENIED','RM_READY','RM_RECONNECTED','RM_PARTIAL_DENIED','RM_IDENTITY_DENIED']
        labels=re.findall(r'^PASS (\S+)',raw,re.M)
        if labels!=expected:raise ValueError('Formal client cases differ')
        match=re.search(r'RM_RECONNECTED old=(\d+) new=(\d+) revision=(\d+) accepted=2',raw)
        if not match or match[1]==match[2]:raise ValueError('Replacement identity missing')
        store=work/f'data/sessionStore/{port}/root.data'
        if not store.exists() or store.stat().st_size!=0:raise ValueError('Registration-only run created a global session or missing FILE store')
        if process.poll() is not None:raise ValueError('TC died during successful probes')
        loaded={match[1]:match[2].replace('\\','/') for match in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(work/'class-load.log').read_text(encoding='utf-8'))}
        checker='org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler'
        if extension.as_posix() not in loaded.get(checker,''):raise ValueError('Actual auth checker did not load from the frozen extension directory')
        origins[checker]=loaded[checker]
        for name in ['org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.core.rpc.processor.server.RegRmProcessor',
                     'org.apache.seata.core.rpc.netty.ChannelManager','org.apache.seata.server.auth.AbstractCheckAuthHandler']:
            if 'seata-server.jar' not in loaded.get(name,''):raise ValueError('Critical Java class did not originate in pinned JAR: '+name)
            origins[name]=loaded[name]
        save(out/'actual-class-origins.json',origins)
        success=True
    finally:
        if process and process.poll() is None:
            process.kill();process.wait(timeout=10)
        owners=listeners()
        if owners:cleanup.append('port still listening')
        save(out/'result.json',dict(succeeded=success and not cleanup,java_pin=PIN,jar_sha256=JAR_SHA,tc_pid=process.pid if process else None,
            listener_ownership_verified=success,port_released=not owners,cleanup_errors=cleanup,
            formal_client_cases=6 if success else 0,tm_and_rm_native_auth=True,rm_cached_resources=2,rm_replacement_generation=True,
            actual_class_origins_verified=len(origins)==5,actual_runtime_extension_files_frozen=True,
            partial_resource_denial=True,registry_revision_ready_guard=True,no_transaction_operations=True,global_store_bytes=0 if success else None,
            external_services_started=1,new_java_compilation=False,new_java_unit_tests=False,production_auth_complete=False,RM_business_callbacks_complete=False,WP02_complete=False))
    if not success or cleanup:raise ValueError('Formal Java integration/cleanup failed')
    put(out/'class-load.log',(work/'class-load.log').read_bytes())
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Live input drift: '+row['path'])
    if sha(jar)!=JAR_SHA or sha(binary)!=binary_sha:raise ValueError('JAR/binary drift')
    save(out/'test-artifact.json',dict(path=str(binary),sha256=binary_sha,snapshot='artifacts/registration_live.exe'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
