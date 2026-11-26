"""Run real SDK remoting via an owned fault proxy and unchanged pinned TC."""
import datetime
import importlib.util
import json
from pathlib import Path
import re
import socket
import subprocess
from collections import Counter

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('sdk_wire_proxy',HERE/'proxy.py');proxy=importlib.util.module_from_spec(spec);spec.loader.exec_module(proxy)


def verify(directory,proof):
    events=json.loads((directory/'proxy-events.json').read_bytes());result=json.loads((directory/'proxy-result.json').read_bytes())
    if result!=dict(connections=2,forced_disconnect=True,batches=4,ordinary_faults=3,errors=[]):raise RuntimeError('Actual SDK loop/proxy state differs: '+str(result))
    wires=[e for e in events if e['event']=='wire'];requests=[e for e in wires if e['direction']=='sdk-to-tc' and e['kind'] in [0,2]];responses=[e for e in wires if e['direction']=='tc-to-sdk' and e['kind']==1]
    if len(requests)!=17 or len(responses)!=17 or Counter(e['payload_type'] for e in requests)!=Counter({101:2,59:4,15:9,9:2}) or Counter(e['payload_type'] for e in responses)!=Counter({102:2,60:4,16:9,10:2}):raise RuntimeError('Actual SDK transaction request/response inventory differs')
    if len({e['id'] for e in requests})!=17 or max(e['id'] for e in requests)>128:raise RuntimeError('SDK request correlation/resend/cleanup range differs')
    if {(e['connection'],e['id']) for e in requests}!={(e['connection'],e['id']) for e in responses}:raise RuntimeError('Actual TC request/response pairing differs')
    for connection in [1,2]:
        pings={e['id'] for e in wires if e['connection']==connection and e['direction']=='sdk-to-tc' and e['kind']==3}
        pongs={e['id'] for e in wires if e['connection']==connection and e['direction']=='tc-to-sdk' and e['kind']==4}
        if not pings or not pongs or not pongs.issubset(pings):raise RuntimeError('Actual heartbeat roundtrip absent')
    modes={e['mode'] for e in events if e['event']=='injected'}
    if modes!={'short-result','wrong-child-type','foreign-id','request-envelope','oneway-envelope','branch-reply-type'}:raise RuntimeError('Exact six SDK injected cases missing')
    if sum(e['event']=='late-valid' for e in events)!=5 or sum(e['event']=='duplicate/coalesced' for e in events)!=11:raise RuntimeError('Duplicate/late-frame inventory differs')
    expected=dict(real_getty_loop=True,captured_profile_choice=True,mixed_result_order=True,reconnects=1,original_begin_queries=4,rollbacks=2,finished_queries=2,typed_unknown_replies=5,foreign_outer_id_ignored=True,transaction_resends=0,owned_future_ids_checked=128,owned_futures_remaining=0)
    if any(proof.get(key)!=value for key,value in expected.items()):raise RuntimeError('SDK proof fields differ')
    if len(proof['xids'])!=2 or len(set(proof['xids']))!=2 or not all(re.fullmatch(r'(?:\d{1,3}\.){3}\d{1,3}:28091:\d+',xid) for xid in proof['xids']):raise RuntimeError('Original SDK XID identity differs')
    tc=(directory/'tc-stdout.log').read_bytes()
    for xid in proof['xids']:
        if ('Rollback global transaction successfully, xid = '+xid).encode() not in tc:raise RuntimeError('Original XID rollback missing from actual TC')
    for name in ['sdk-begin-a','sdk-begin-b']:
        if tc.count(('transactionName: '+name+',').encode())!=1:raise RuntimeError('Mutating begin was resent or missing')
    return dict(requests=17,responses=17,connections=2,heartbeats_on_both_connections=True,duplicates_coalesced=11,late_valid_replies=5,injected_cases=6,transaction_resends=0)


def run(out,work,base):
    h=base.h;l=base.l;c=base.c
    directory=out/'sdk-live';directory.mkdir()
    runtime=Path('D:/codex-builds')/('seata-sdk-live-'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'-'+out.name[-8:]);runtime.mkdir(exist_ok=False)
    if h.port_open(28091) or h.port_open(28092):raise RuntimeError('Owned SDK test ports already occupied')
    jar=c.ORACLE/'server/target/seata-server.jar'
    if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR drifted')
    binary=directory/'sdk-live.exe';l.run_command(['go','build','-o',binary,HERE/'live_main.go'],work,c.GO_ENV,directory/'go-build.log')
    config=(base.ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091')
    h.write(runtime/'application.yml',config);h.write(directory/'application.yml',config)
    env=h.ENV.copy();env.update(SEATA_TC_WORK_DIR=runtime.as_posix())
    tc=None;relay=None;success=False;proof=None;inventory=None
    try:
        tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),'-jar',jar,'-h','127.0.0.1'],directory,'tc',runtime,env)
        h.wait_until(lambda:h.port_open(28091),90,'Owned SDK TC startup',tc)
        relay=proxy.SdkProxy(runtime,directory);relay.start()
        result=subprocess.run([str(binary),str(runtime)],cwd=runtime,env=env,capture_output=True,creationflags=h.FLAGS,timeout=65)
        h.write(directory/'client.log',result.stdout+result.stderr)
        relay.close();relay=None
        if (runtime/'sdk.log').exists():h.write(directory/'sdk.log',(runtime/'sdk.log').read_bytes())
        rows=[json.loads(line.split(b'SDK_LIVE_PASS ',1)[1]) for line in result.stdout.splitlines() if line.startswith(b'SDK_LIVE_PASS ')]
        if result.returncode or len(rows)!=1:raise RuntimeError('Real SDK event-loop scenario failed')
        if result.stdout.count(b'SDK_UNKNOWN label=')!=5:raise RuntimeError('Typed unknown labels missing')
        proof=rows[0];inventory=verify(directory,proof)
        # 单独验证原TC的首字节探测问题；不是SDK场景的额外成功消息或兼容性豁免。
        with socket.create_connection(('127.0.0.1',28091),timeout=2) as partial:
            partial.sendall(b'\xda');closed=partial.recv(1)==b''
        if not closed:raise RuntimeError('Pinned Java preface negative behavior changed; investigate')
        h.wait_until(lambda:b'preface = [-38]' in (directory/'tc-stdout.log').read_bytes(),3,'Actual Java first-byte rejection',tc)
        detectors=['core/src/main/java/org/apache/seata/core/rpc/netty/ProtocolDetectHandler.java','core/src/main/java/org/apache/seata/core/protocol/detector/SeataDetector.java']
        source_rows=[]
        for relative in detectors:
            source=c.ORACLE/relative;target=directory/(source.name+'.source.txt');h.write(target,source.read_bytes());source_rows.append(dict(path=relative,sha256=h.sha(source)))
        h.write(directory/'java-preface-negative.json',json.dumps(dict(issue='JAVA-V1-PREFACE-001',status='OPEN',reproduced=True,first_fragment_hex='da',tc_closed_before_next_fragment=closed,jar_sha256=h.sha(jar),source=source_rows,scope='Negative evidence, not interoperability success; TC-bound forced fragmentation excluded from SDK scenario and remains unverified/known failing.'),indent=2)+'\n')
        success=True
    finally:
        if relay:relay.close()
        h.stop(tc);h.wait_until(lambda:not h.port_open(28091) and not h.port_open(28092),10,'Owned SDK TC/proxy cleanup')
        state=dict(succeeded=success,runtime_directory=str(runtime),tc_pid=tc.pid if tc else None,ports_released=not h.port_open(28091) and not h.port_open(28092),jar_sha256=h.sha(jar),binary_sha256=h.sha(binary),source_sha256=h.sha(HERE/'live_main.go'),proof=proof,wire_inventory=inventory,
          scope='Original InitGetty/SessionManager/dubbo-getty loop/listener/processor/futures with explicit profile. FILE/auth-disabled TC, transparent fault proxy; no real RM/Undo/production Rust.')
        h.write(directory/'result.json',json.dumps(state,indent=2)+'\n')
    return state
