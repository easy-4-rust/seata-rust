"""正式TM对原始固定JavaTC的六功能场景；不把该运行称为响应丢失或SQL验收。"""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import uuid

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('transport_loss_live_support',ROOT/'tools/formal-transport-loss/live.py')
support=importlib.util.module_from_spec(spec);spec.loader.exec_module(support)
reg=support.reg
sha,put,save,command=support.sha,support.put,support.save,support.command


def run():
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
        LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    jar=reg.ORACLE/'server/target/seata-server.jar'
    if command(['git','-C',reg.ORACLE,'rev-parse','HEAD'],env).decode().strip()!=reg.PIN or sha(jar)!=reg.JAR_SHA:raise ValueError('Original Java pin/JAR differs')
    historical=json.loads(reg.HISTORICAL.read_bytes());compiled=reg.FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class';service=reg.FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler'
    if sha(compiled)!=historical['extension_class_sha256'] or sha(service)!=historical['extension_service_sha256']:raise ValueError('Public auth fixture differs')
    with socket.socket() as check:check.bind(('127.0.0.1',38091))
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/tm-java'/('run_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-formal-tm-'+stamp);work.mkdir();print('TM_JAVA_RUN '+str(out),flush=True)
    extension=work/'fixture'
    for path in [compiled,service]:put(extension/path.relative_to(reg.FIXTURE),path.read_bytes())
    config_source=ROOT/'tools/java-tc-integration/config/application.yml'
    config=config_source.read_text(encoding='utf-8').replace('18091','38091').replace('enable-check-auth: false','enable-check-auth: true')
    put(work/'application.yml',config);put(out/'application.yml',config)
    nonce=uuid.uuid4().hex
    paths=[Path(__file__),Path(support.__file__),Path(reg.__file__),ROOT/'Cargo.toml',ROOT/'Cargo.lock',config_source,work/'application.yml',reg.HISTORICAL,compiled,service]
    paths += [p for p in extension.rglob('*') if p.is_file()]+[p for p in (ROOT/'crates').rglob('*') if p.is_file()]
    frozen=[]
    for path in sorted(set(paths)):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save(out/'source-inputs.json',frozen)
    command(['cargo','build','--offline','--locked','-p','seata-tm','--example','tm_live'],env,out/'build.log')
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/tm_live.exe';binary_sha=sha(binary);put(out/'artifacts/tm_live.exe',binary.read_bytes())
    runtime_env=env|dict(SEATA_TC_WORK_DIR=work.as_posix(),SEATA_TM_ENDPOINT='127.0.0.1:38091',SEATA_TM_NONCE=nonce)
    args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix(),
        '-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
    tc=None;functional=False;cleanup=[];origins={};xids=[]
    try:
        tc=support.spawn(args,work,runtime_env,out,'tc');support.wait_owned(tc,38091,env)
        save(out/'owned-process.json',dict(pid=tc.pid,command=[str(x) for x in args],runtime=str(work),nonce=nonce))
        command([binary],runtime_env,out/'client.log',work,timeout=25)
        text=(out/'client.log').read_text(encoding='utf-8')
        expected=['TM_BEGIN_COMMIT','TM_BUSINESS_ROLLBACK','TM_REQUIRED_PARTICIPANTS','TM_REQUIRES_NEW','TM_PARTICIPANT','TM_BUSINESS_DEADLINE']
        if re.findall(r'^PASS (\S+)',text,re.M)!=expected:raise ValueError('Six formal TM scenarios incomplete/duplicated')
        xids=re.findall(r'^CLEAN xid=(\S+) status=15$',text,re.M)
        reported=re.findall(r'\b(?:xid|outer|inner)=(\S+)', '\n'.join(line for line in text.splitlines() if line.startswith('PASS ')))
        if len(xids)!=7 or len(set(xids))!=7 or sorted(xids)!=sorted(reported):raise ValueError('Original seven XIDs not explicitly reconciled/cleaned')
        loaded={match[1]:match[2].replace('\\','/') for match in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(work/'class-load.log').read_text(encoding='utf-8'))}
        checker='org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler'
        if extension.as_posix() not in loaded.get(checker,''):raise ValueError('Runtime auth SPI class differs')
        origins[checker]=loaded[checker]
        for name in ['org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.core.rpc.netty.ChannelManager',
            'org.apache.seata.server.coordinator.DefaultCore','org.apache.seata.server.coordinator.AbstractCore','org.apache.seata.server.auth.AbstractCheckAuthHandler']:
            origin=loaded.get(name,'')
            if jar.as_posix() not in origin or (('seata-core-' not in origin) if '.core.' in name else ('BOOT-INF/classes' not in origin)):raise ValueError('TC class not from pinned JAR: '+name)
            origins[name]=origin
        save(out/'class-origins.json',origins);functional=True
    finally:
        support.stop(tc)
        if (work/'class-load.log').exists():put(out/'class-load.log',(work/'class-load.log').read_bytes())
        if support.listeners(38091,env):cleanup.append('owned TC port still listening')
        save(out/'result.json',dict(succeeded=False,functional_cases_completed=functional,cleanup_errors=cleanup,java_pin=reg.PIN,jar_sha256=reg.JAR_SHA,
            formal_tm_cases=6,original_transactions=7,final_status_observations=xids,class_origins=len(origins),input_closure_verified=False,
            tc_process_cleaned=not cleanup,external_services_started=1,new_java_compilation=False,new_java_unit_tests=False,
            mysql_started=False,RM_SQL_Undo_verified=False,TM_response_loss_java_verified=False,WP03_complete=False,full_V1_complete=False))
    if not functional or cleanup:raise ValueError('Formal TM functional/cleanup gate failed')
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Source/runtime input drift: '+row['path'])
    if sha(jar)!=reg.JAR_SHA or sha(binary)!=binary_sha:raise ValueError('JAR/binary drift')
    save(out/'test-artifact.json',dict(path=str(binary),sha256=binary_sha,snapshot='artifacts/tm_live.exe'))
    final=json.loads((out/'result.json').read_bytes());final.update(succeeded=True,input_closure_verified=True)
    (out/'result.json').write_text(json.dumps(final,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
