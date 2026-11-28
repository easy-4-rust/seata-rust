"""固定原JavaTC+真实字节丢响应，正式transport驱动NET010/011/012；模拟RM不等同SQL补偿。"""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import time
import uuid

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('formal_registration_live_support',ROOT/'tools/formal-registration/live.py')
reg=importlib.util.module_from_spec(spec);spec.loader.exec_module(reg)


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as stream:stream.write(data.encode('utf-8') if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def command(args,env,log=None,cwd=ROOT,timeout=60):
    value=subprocess.run([str(x) for x in args],env=env,cwd=cwd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=timeout)
    if log:put(log,value.stdout)
    if value.returncode:raise RuntimeError(str(args[0])+': '+value.stdout[-1800:].decode(errors='replace'))
    return value.stdout
def listeners(port,env):
    try:
        raw=command(['powershell','-NoProfile','-Command',f'@(Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue).OwningProcess | ConvertTo-Json -Compress'],env,timeout=10).decode('utf-8-sig').strip()
    except subprocess.TimeoutExpired:
        # Windows CIM 冷启动可能超时；回退仍须读取并核对真正的监听进程 PID。
        rows=command(['netstat','-ano','-p','tcp'],env,timeout=10).decode(errors='replace').splitlines()
        owners=[]
        for row in rows:
            fields=row.split()
            if len(fields)==5 and fields[0].upper()=='TCP' and fields[3].upper()=='LISTENING' and fields[1].rsplit(':',1)[-1]==str(port):
                owners.append(int(fields[4]))
        return owners
    if not raw:return []
    value=json.loads(raw);return value if isinstance(value,list) else [value]
def wait_owned(process,port,env):
    deadline=time.monotonic()+90
    while True:
        if process.poll() is not None:raise RuntimeError('Owned service exited during startup')
        owners=listeners(port,env)
        if owners:
            if set(owners)!={process.pid}:raise ValueError('Listener owner differs')
            return
        if time.monotonic()>=deadline:raise TimeoutError('Owned service startup timeout')
        time.sleep(.2)
def spawn(args,work,env,out,prefix):
    with (out/(prefix+'-stdout.log')).open('xb') as stdout,(out/(prefix+'-stderr.log')).open('xb') as stderr:
        return subprocess.Popen([str(x) for x in args],cwd=work,env=env,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
def stop(process):
    if process and process.poll() is None:process.kill();process.wait(timeout=10)
def payload(row):
    raw=bytes.fromhex(row['frame_hex']);head=struct.unpack('>h',raw[7:9])[0];return raw[head:]
def string(data,at,wide=False):
    width=4 if wide else 2
    if at+width>len(data):raise ValueError('Truncated observed string length')
    length=struct.unpack('>i' if wide else '>h',data[at:at+width])[0];at+=width
    if length<0 or at+length>len(data):raise ValueError('Invalid observed string')
    return data[at:at+length].decode('utf-8'),at+length
def branch_identity(row,callback):
    data=payload(row);xid,at=string(data,2)
    branch=struct.unpack('>q',data[at:at+8])[0] if callback else None
    if callback:at+=8
    branch_type=data[at];resource,at=string(data,at+1)
    if not callback:lock,at=string(data,at,True)
    else:lock=None
    application,at=string(data,at,True)
    if at!=len(data):raise ValueError('Trailing bytes in observed branch identity')
    return dict(xid=xid,branch_id=branch,branch_type=branch_type,resource=resource,lock=lock,data=application)

def lock_results(rows):
    values=[]
    for row in rows:
        data=payload(row)
        if len(data)!=6 or data[2:4]!=b'\x01\x00' or data[4:6] not in [b'\x00\x00',b'\x00\x01']:raise ValueError('Lock response is not successful canonical Java u16 boolean')
        values.append(struct.unpack('>H',data[4:6])[0])
    return values


def validate_wire_rows(rows):
    """独立重读原始字节；元数据必须一致，双向请求均按原物理连接和signed ID配对。"""
    pending={};callbacks={};verified=[]
    for index,row in enumerate(rows):
        if row['event']!='frame':verified.append(row);continue
        raw=bytes.fromhex(row['frame_hex'])
        if len(raw)<18 or raw[:3]!=b'\xda\xda\x01' or struct.unpack('>I',raw[3:7])[0]!=len(raw) or struct.unpack('>H',raw[7:9])[0]!=16 or raw[10:12]!=b'\x01\x00':raise ValueError('Noncanonical native fixture frame')
        direction=row['direction'];connection=row['connection'];request_type=raw[9];wire_id=struct.unpack('>i',raw[12:16])[0];data=raw[16:];kind=struct.unpack('>H',data[:2])[0]
        if direction not in ['c2s','s2c'] or request_type not in [0,1] or (kind,request_type,wire_id)!=(row['kind'],row['request_type'],row['request_id']):raise ValueError('Frame metadata differs from original bytes')
        xid=None;branch=None;status=None;pair=None;key=(connection,wire_id)
        if direction=='c2s' and request_type==0:
            if key in pending:raise ValueError('Client reused an original physical connection request ID')
            if kind in [7,9,11,15,17,21]:xid,_=string(data,2)
            pending[key]=(kind,xid,index)
        elif direction=='s2c' and request_type==0:
            if kind not in [3,5] or key in callbacks:raise ValueError('Unexpected or duplicate branch callback')
            identity=branch_identity(row,True);xid=identity['xid'];branch=identity['branch_id'];callbacks[key]=(kind,xid,branch,index)
        elif direction=='s2c' and request_type==1:
            if key not in pending:raise ValueError('Server response has no original connection/ID request')
            original,xid,pair=pending[key]
            if kind!=original+1:raise ValueError('Server response family differs')
            if kind in [2,8,10,12,16,18,22]:
                if data[2:4]!=b'\x01\x00':raise ValueError('Transaction response was not successful')
                if kind==2:
                    xid,end=string(data,4)
                    if not xid or data[end:]!=b'\x00\x00':raise ValueError('Invalid successful Begin XID/empty extraData')
                elif kind==12:
                    if len(data)!=12:raise ValueError('Invalid BranchRegister response length')
                    branch=struct.unpack('>q',data[4:])[0]
                    if branch<=0:raise ValueError('Invalid registered BranchId')
                elif kind in [8,10,16,18]:
                    if len(data)!=5:raise ValueError('Invalid global status response length')
                    status=data[4]
        else:
            if key not in callbacks:raise ValueError('Branch reply did not use original callback connection/ID')
            original,original_xid,original_branch,pair=callbacks[key]
            if kind!=original+1 or data[2:4]!=b'\x01\x00':raise ValueError('Branch reply family/result differs')
            xid,at=string(data,4)
            if len(data)!=at+9:raise ValueError('Invalid branch response length')
            branch=struct.unpack('>q',data[at:at+8])[0]
            if (xid,branch,data[-1])!=(original_xid,original_branch,5 if kind==4 else 8):raise ValueError('Branch response identity/terminal status differs')
        if (xid,branch,status)!=(row['xid'],row['branch_id'],row['global_status']):raise ValueError('Derived transaction metadata differs from raw bytes')
        verified.append(dict(row,matched_request_index=pair))
    return verified


def validate(mode,text,rows,nonce):
    rows=validate_wire_rows(rows)
    frames=[row for row in rows if row['event']=='frame'];summaries=[row for row in rows if row['event']=='summary']
    if len(summaries)!=1 or not summaries[0]['succeeded'] or summaries[0]['drop_count']!=1:raise ValueError('Proxy did not prove exactly one successful response loss')
    drops=[row for row in frames if row['dropped']]
    if len(drops)!=1 or drops[0]['kind']!={'begin':2,'commit':8,'branch-register':12}[mode]:raise ValueError('Wrong dropped response')
    drop=drops[0]
    if payload(drop)[2:4]!=b'\x01\x00':raise ValueError('Dropped response was not success')
    requests=[row for row in frames if row['direction']=='c2s' and row['request_type']==0]
    target=drop['xid']
    if not target:raise ValueError('Observer did not identify original XID')
    if mode=='begin':
        if len([row for row in requests if row['kind']==1])!=1 or any(row['kind'] in [7,9,11] for row in requests):raise ValueError('Lost Begin was replayed or observer XID used in client cleanup')
        match=re.search(r'PASS LOSS_BEGIN operation=1 request_id=(-?\d+) xid_known=false begin_attempts=1 pending=0 unknown=true',text)
        if not match or int(match[1])!=drop['request_id'] or target in text:raise ValueError('Client learned observer XID or did not retain original unknown')
        return dict(mode=mode,operation=1,observer_only_xid=target,drop_count=1,begin_requests=1,client_xid_known=False,original_unknown_retained=True,transaction_replay=False)
    registrations=[row for row in requests if row['kind']==11]
    if len(registrations)!=1 or registrations[0]['xid']!=target:raise ValueError('Missing/duplicate target BranchRegister')
    registered=branch_identity(registrations[0],False)
    registration_index=rows.index(registrations[0])
    registration_results=[row for row in frames if row['direction']=='s2c' and row['kind']==12 and row['matched_request_index']==registration_index]
    if len(registration_results)!=1:raise ValueError('Missing uniquely correlated original successful branch registration response')
    callbacks=[row for row in frames if row['direction']=='s2c' and row['request_type']==0 and row['kind']==(3 if mode=='commit' else 5)]
    replies=[row for row in frames if row['direction']=='c2s' and row['request_type']==1 and row['kind']==(4 if mode=='commit' else 6)]
    if len(callbacks)!=1 or len(replies)!=1:raise ValueError('Expected one actual branch callback/reply')
    callback=branch_identity(callbacks[0],True);reply=replies[0]
    if callback['branch_id']!=registration_results[0]['branch_id'] or rows.index(registration_results[0])>=rows.index(callbacks[0]):raise ValueError('Callback was not for the original registered BranchId')
    if callback['xid']!=target or callback['branch_type']!=0 or callback['resource']!=registered['resource'] or callback['data']!=registered['data'] or json.loads(callback['data'])!={'formalOperation':nonce}:raise ValueError('TC callback did not retain original operation identity')
    if reply['xid']!=target or reply['branch_id']!=callback['branch_id'] or reply['request_id']!=callbacks[0]['request_id']:raise ValueError('Response lost original branch identity or wire correlation')
    target_requests=[row for row in requests if row['xid']==target]
    locks=[row for row in frames if row['direction']=='s2c' and row['request_type']==1 and row['kind']==22]
    queries=[row for row in requests if row['kind']==21]
    if len(queries)!=2 or queries[0]['xid']!=queries[1]['xid'] or queries[0]['xid']==target:raise ValueError('Lock contender identity differs')
    for query,result in zip(queries,locks):
        identity=branch_identity(query,False)
        if (identity['resource'],identity['lock'],identity['branch_type'])!=(registered['resource'],registered['lock'],registered['branch_type']) or result['matched_request_index']!=rows.index(query):raise ValueError('Lock query/result did not retain original resource/key/connection/ID')
    if lock_results(locks)!=[0,1]:raise ValueError('Original branch lock was not occupied then released')
    if mode=='commit':
        commits=[index for index,row in enumerate(target_requests) if row['kind']==7]
        if len(commits)!=2 or any(row['kind']==9 for row in target_requests) or not any(row['kind']==15 for row in target_requests[commits[0]+1:commits[1]]):raise ValueError('Expected explicit same-XID Commit duplicate after status observation, no Rollback')
        match=re.search(r'PASS LOSS_COMMIT operation=7 request_id=(-?\d+) xid=(\S+) branch_id=(\d+) callback_id=(-?\d+) commit_attempts=2 explicit_retry=true observed=(\d+) retry_status=(\d+) confirmed=(true|false) actions=1',text)
        if not match or int(match[1])!=drop['request_id'] or match[2]!=target or int(match[3])!=callback['branch_id'] or int(match[4])!=callbacks[0]['request_id']:raise ValueError('Client Commit uncertainty/callback evidence differs')
        if match[5]=='15' and match[6]=='15' and match[7]!='false':raise ValueError('Finished promoted to historical Commit proof')
        return dict(mode=mode,xid=target,branch_id=callback['branch_id'],drop_count=1,commit_requests=2,explicit_retry_after_status=True,operation_changed_to_rollback=False,
            branch_register_requests=1,phase_two_actions=1,observed_status=int(match[5]),retry_status=int(match[6]),confirmed=match[7]=='true',lock_released=True)
    if callback['branch_id']!=drop['branch_id'] or any(row['kind']==7 for row in target_requests) or len([row for row in target_requests if row['kind']==9])!=1:raise ValueError('Pending BranchRegister was not explicitly cancelled using original XID')
    match=re.search(r'PASS LOSS_BRANCH_REGISTER operation=11 request_id=(-?\d+) xid=(\S+) branch_id=(\d+) callback_id=(-?\d+) learned_from_callback=true registration_attempts=1 actions=1 cancellation_acknowledged=true',text)
    if not match or int(match[1])!=drop['request_id'] or match[2]!=target or int(match[3])!=drop['branch_id'] or int(match[4])!=callbacks[0]['request_id']:raise ValueError('Pending branch reconciliation differs from independent dropped-response oracle')
    return dict(mode=mode,xid=target,branch_id=callback['branch_id'],drop_count=1,branch_register_requests=1,learned_only_from_matching_callback=True,
        cancellation_acknowledged=True,phase_two_actions=1,lock_released=True,registration_retried=False)


def run():
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',
        LOCALAPPDATA='D:/Temp/seata-localappdata',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    jar=reg.ORACLE/'server/target/seata-server.jar'
    if command(['git','-C',reg.ORACLE,'rev-parse','HEAD'],env).decode().strip()!=reg.PIN or sha(jar)!=reg.JAR_SHA:raise ValueError('Original Java pin/JAR differs')
    historical=json.loads(reg.HISTORICAL.read_bytes());compiled=reg.FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class';service=reg.FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler'
    if sha(compiled)!=historical['extension_class_sha256'] or sha(service)!=historical['extension_service_sha256']:raise ValueError('Cached auth fixture differs')
    for port in [38091,38092]:
        with socket.socket() as check:check.bind(('127.0.0.1',port))
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];work=Path('D:/codex-builds')/('seata-formal-loss-'+stamp);work.mkdir()
    out=ROOT/'docs/phase-1/evidence/transport-loss-java'/('run_'+stamp);out.mkdir(parents=True);print('LOSS_JAVA_RUN '+str(out),flush=True)
    extension=work/'fixture'
    for path in [compiled,service]:put(extension/path.relative_to(reg.FIXTURE),path.read_bytes())
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'tools/formal-transport-loss/loss_proxy.py',ROOT/'tools/formal-transport-loss/test_proxy.py',ROOT/'tools/java-tc-integration/config/application.yml',Path(reg.__file__),reg.HISTORICAL,compiled,service]
    paths += [p for p in extension.rglob('*') if p.is_file()]+[p for p in (ROOT/'crates').rglob('*') if p.is_file()]
    frozen=[]
    for path in sorted(set(paths)):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    command(['cargo','build','--offline','--locked','-p','seata-transport','--example','transport_loss_live'],env,out/'rust-build.log')
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/transport_loss_live.exe';put(out/'artifacts/transport_loss_live.exe',binary.read_bytes());binary_sha=sha(binary)
    command(['python','-m','unittest','discover','-s','tools/formal-transport-loss','-p','test_proxy.py','-v'],env,out/'proxy-tests.log')
    cases=[];success=False;cleanup=[];tc=None;proxy=None
    try:
        for mode,kind in [('begin',2),('commit',8),('branch-register',12)]:
            case=out/mode;case.mkdir();runtime=work/mode;runtime.mkdir();nonce=uuid.uuid4().hex
            config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','38091').replace('enable-check-auth: false','enable-check-auth: true')
            put(runtime/'application.yml',config);put(case/'application.yml',config)
            actual_config=runtime/'application.yml';snapshot=mode+'/actual-runtime-application.yml'
            put(out/snapshot,actual_config.read_bytes());frozen.append(dict(path=str(actual_config),sha256=sha(actual_config),snapshot=snapshot))
            save(case/'runtime-config-reference.json',dict(path=str(actual_config),sha256=sha(actual_config),snapshot=snapshot))
            runtime_env=env|dict(SEATA_TC_WORK_DIR=runtime.as_posix(),SEATA_LOSS_ENDPOINT='127.0.0.1:38092',SEATA_LOSS_MODE=mode,SEATA_LOSS_NONCE=nonce)
            args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),
                '-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
            tc=spawn(args,runtime,runtime_env,case,'tc');wait_owned(tc,38091,env)
            proxy=spawn(['python',ROOT/'tools/formal-transport-loss/loss_proxy.py','--listen-port','38092','--upstream-port','38091','--drop-response-kind',kind,'--events',case/'events.jsonl'],runtime,runtime_env,case,'proxy');wait_owned(proxy,38092,env)
            save(case/'owned-processes.json',dict(tc_pid=tc.pid,proxy_pid=proxy.pid,tc_command=[str(x) for x in args],runtime=str(runtime),nonce=nonce,observer_data_given_to_client=False))
            command([binary],runtime_env,case/'client.log',runtime,timeout=25)
            proxy.wait(timeout=10)
            if proxy.returncode:raise ValueError('Owned loss proxy failed')
            rows=[json.loads(line) for line in (case/'events.jsonl').read_text(encoding='utf-8').splitlines()]
            result=validate(mode,(case/'client.log').read_text(encoding='utf-8'),rows,nonce)
            loaded={match[1]:match[2].replace('\\','/') for match in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(runtime/'class-load.log').read_text(encoding='utf-8'))};origins={}
            checker='org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler'
            if extension.as_posix() not in loaded.get(checker,''):raise ValueError('Checker runtime origin differs')
            origins[checker]=loaded[checker]
            for name in ['org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.core.rpc.processor.server.RegRmProcessor','org.apache.seata.core.rpc.netty.ChannelManager',
                         'org.apache.seata.server.coordinator.DefaultCore','org.apache.seata.server.coordinator.AbstractCore','org.apache.seata.server.auth.AbstractCheckAuthHandler']:
                origin=loaded.get(name,'')
                if jar.as_posix() not in origin or (('seata-core-' not in origin) if '.core.' in name else ('BOOT-INF/classes' not in origin)):raise ValueError('Critical TC class not from pinned JAR: '+name)
                origins[name]=loaded[name]
            save(case/'class-origins.json',origins)
            result.update(tc_pid=tc.pid,proxy_pid=proxy.pid,actual_class_origins=7,nonce=nonce,simulated_rm_only=True)
            if mode=='begin':
                store=runtime/'data/sessionStore/38091/root.data'
                if not store.exists() or store.stat().st_size==0:raise ValueError('Lost Begin did not leave original FILE evidence')
                result['retained_test_store_bytes']=store.stat().st_size
            stop(proxy);stop(tc);put(case/'class-load.log',(runtime/'class-load.log').read_bytes())
            if listeners(38091,env) or listeners(38092,env):raise ValueError('Owned case ports not released')
            result['ports_released']=True;save(case/'result.json',result);cases.append(result);tc=None;proxy=None
        success=True
    finally:
        stop(proxy);stop(tc)
        for port in [38091,38092]:
            if listeners(port,env):cleanup.append('port still listening:'+str(port))
        save(out/'source-inputs.json',frozen)
        save(out/'result.json',dict(succeeded=False,functional_cases_completed=success and not cleanup,input_closure_verified=False,java_pin=reg.PIN,jar_sha256=reg.JAR_SHA,cases=cases,cleanup_errors=cleanup,
            observer_data_given_to_client=False,formal_transport_api=True,simulated_rm_only=True,mysql_compensation=False,new_java_compilation=False,new_java_unit_tests=False,
            external_services_started=6 if success else None,WP02_complete=False,full_V1_complete=False,production_recovery_complete=False))
    if not success or cleanup:raise ValueError('Formal response-loss or cleanup gate failed')
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Input drift:'+row['path'])
    if sha(jar)!=reg.JAR_SHA or sha(binary)!=binary_sha:raise ValueError('JAR/client artifact drift')
    save(out/'test-artifact.json',dict(path=str(binary),sha256=binary_sha,snapshot='artifacts/transport_loss_live.exe'))
    final=json.loads((out/'result.json').read_bytes());final.update(succeeded=True,input_closure_verified=True)
    (out/'result.json').write_text(json.dumps(final,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),source_inputs=len(frozen),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
