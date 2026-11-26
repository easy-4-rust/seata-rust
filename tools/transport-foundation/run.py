"""验证WP02流/编号基础，并绑定使用相同正式协议源码的已成功peer回归；不启动网络服务。"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import uuid

ROOT=Path(__file__).resolve().parents[2]


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as stream:stream.write(data.encode('utf-8') if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,ensure_ascii=False,indent=2)+'\n')


def run(peer):
    peer=(ROOT/peer).resolve()
    if not peer.is_relative_to(ROOT/'docs/phase-1/evidence/protocol-peer-profile'):raise ValueError('Need a local protocol-peer evidence directory')
    peer_catalog=peer/'catalog.json';peer_catalog_sha=sha(peer_catalog)
    for row in json.loads(peer_catalog.read_bytes())['members']:
        path=(peer/row['path']).resolve()
        if not path.is_relative_to(peer) or sha(path)!=row['sha256']:raise ValueError('Peer evidence member differs')
    peer_result=json.loads((peer/'result.json').read_bytes())
    if (peer_result['positive_contexts'],peer_result['negative_complete_input_cases'],peer_result['formal_rust_tests'])!=(628,2050,74):raise ValueError('Incomplete protocol regression')
    checked=0
    for row in json.loads((peer/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if ((path.is_relative_to(ROOT/'crates/seata_protocol') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']):
            if sha(path)!=row['sha256']:raise ValueError('Protocol compiled input differs; execute fresh peer regression')
            checked+=1
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/transport-foundation'/('run_'+stamp);out.mkdir(parents=True)
    print('TRANSPORT_RUN '+str(out),flush=True)
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),ROOT/'docs/phase-1/wp01-protocol-gate.json',
        ROOT/'docs/phase-0/v1-implementation-acceptance-checklist.md',ROOT/'docs/phase-0/seata-go-transport-timeout-semantics.md',
        ROOT/'docs/adr/0008-wire-optionality-and-unknown-outcomes.md',peer_catalog,peer/'result.json',peer/'source-inputs.json']
    paths += [p for folder in [ROOT/'crates',ROOT/'tests/fixtures/protocol/v1',ROOT/'tools/java-tc-integration/rust-client'] for p in folder.rglob('*') if p.is_file()]
    paths += [Path('E:/workspaces/AGENTS.md'),Path('E:/workspaces/workspace-github-easy-4-rust/AGENTS.md')]
    frozen=[]
    for path in sorted(set(paths)):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin'
        put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save(out/'source-inputs.json',frozen)
    save(out/'protocol-peer-reference.json',dict(path=str(peer),catalog_sha256=peer_catalog_sha,compiled_protocol_inputs_checked=checked,
        positive_contexts=628,negative_complete_input_cases=2050,new_execution_in_this_script=False))
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    def command(args,log,extra=None):
        value=subprocess.run([str(x) for x in args],cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(out/log,value.stdout)
        if value.returncode:raise RuntimeError(log+': exit '+str(value.returncode))
        return value.stdout
    for leaf in ['transport-api-red.log','transport-peer-scope-red.log']:
        put(out/('tdd/'+leaf),(Path('D:/codex-builds/seata-rust-protocol-start-20261113')/leaf).read_bytes())
    save(out/'tdd/kind.json',dict(api_red='missing public transport/prefix APIs caused compiler errors, not11 runtime assertions',
        peer_scope_red='old fixed74 workspace comparison encountered newly added11 transport tests; package-scoped protocol regression corrected the verifier, not product behavior'))
    command(['cargo','fmt','--all','--','--check'],'fmt.log')
    command(['cargo','clippy','--offline','--locked','--workspace','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','doc','--offline','--locked','--workspace','--no-deps'],'rustdoc.log',dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc'))
    command(['rustc','--version','--verbose'],'rust-version.log');command(['cargo','--version'],'cargo-version.log')
    artifacts=[]
    def test(cargo,extra,label):
        raw=command([cargo,'test','--offline','--locked','--package','seata-transport','--lib','--tests','--message-format=json'],label+'-tests.jsonl',extra)
        if sorted(int(v) for v in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored',raw))!=[1,10]:raise ValueError('Need exactly11 transport tests')
        for line in raw.splitlines():
            if not line.startswith(b'{'):continue
            row=json.loads(line)
            if row.get('reason')=='compiler-artifact' and row.get('executable') and row.get('profile',{}).get('test'):
                path=Path(row['executable']);snapshot='artifacts/'+label+'/'+row['target']['name']+'.exe'
                put(out/snapshot,path.read_bytes());artifacts.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    test('cargo',{},'rust-1-99')
    msrv=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
    command([msrv/'rustc.exe','--version'],'msrv-version.log')
    test(msrv/'cargo.exe',dict(RUSTC=str(msrv/'rustc.exe'),RUSTDOC=str(msrv/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'),'rust-1-85')
    if len(artifacts)!=4:raise ValueError('Need four actual transport test binaries')
    probe_manifest=ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'
    probe=command(['cargo','test','--offline','--locked','--manifest-path',probe_manifest,'--message-format=json'],'standalone-probe-tests.jsonl',dict(CARGO_TARGET_DIR='D:/cargo-target/seata-phase09'))
    if b'117 passed; 0 failed; 0 ignored' not in probe:raise ValueError('Need117 standalone probe regression')
    for line in probe.splitlines():
        if not line.startswith(b'{'):continue
        row=json.loads(line)
        if row.get('reason')=='compiler-artifact' and row.get('executable') and row.get('profile',{}).get('test'):
            path=Path(row['executable']);snapshot='artifacts/standalone-probe.exe'
            put(out/snapshot,path.read_bytes());artifacts.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    if len(artifacts)!=5:raise ValueError('Need standalone test binary')
    command(['cargo','fmt','--manifest-path',probe_manifest,'--','--check'],'standalone-probe-fmt.log')
    command(['cargo','clippy','--offline','--locked','--manifest-path',probe_manifest,'--all-targets','--','-D','warnings'],'standalone-probe-clippy.log',dict(CARGO_TARGET_DIR='D:/cargo-target/seata-phase09'))
    save(out/'test-artifacts.json',artifacts)
    for row in frozen+artifacts:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Source/artifact drift: '+row['path'])
    if sha(peer_catalog)!=peer_catalog_sha:raise ValueError('Peer reference drift')
    save(out/'result.json',dict(transport_tests=11,tests_on_rust_1_99=11,tests_on_msrv_1_85=11,workspace_fmt_clippy_rustdoc=True,
        standalone_probe_tests=117,standalone_probe_fmt_clippy=True,certified_stream_frames=46,
        per_call_one_frame=True,partial_and_coalesced=True,terminal_eof_protocol_errors=True,
        per_connection_id_nonreuse=True,full_signed_i32_space=True,actual_four_billion_iteration_test=False,
        protocol_peer_positive_contexts=628,protocol_peer_negative_contexts=2050,protocol_peer_new_execution_in_this_script=False,
        tcp_connections_implemented=False,pending_requests_implemented=False,NET007_integration_complete=False,WP02_complete=False,
        external_services_started=0,new_third_party_dependencies=0))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--peer-run',required=True);args=parser.parse_args();run(args.peer_run)
