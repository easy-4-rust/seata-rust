"""Compile the exact sealed observation client with Rust 1.85; no second network/storage run is claimed."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

ROOT=Path(__file__).resolve().parents[2];TOOLCHAIN=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def need(value,message):
    if not value:raise ValueError(message)
def run(packet):
    read=lambda path:json.loads(path.read_bytes())
    for row in read(packet/'catalog.json')['members']:need(sha(packet/row['path'])==row['sha256'],'Original observation archive drift')
    inputs=read(packet/'source-inputs.json')
    for row in inputs:need(sha(Path(row['path']))==row['sha256']==sha(packet/row['snapshot']),'Sealed compile input differs')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/tc-lock-msrv-client'/('run_'+stamp);out.mkdir(parents=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv',RUSTC=str(TOOLCHAIN/'rustc.exe'),RUSTDOC=str(TOOLCHAIN/'rustdoc.exe'),TEMP='D:/Temp',TMP='D:/Temp')
    def command(args,leaf):
        value=subprocess.run(list(map(str,args)),cwd=ROOT,env=env,creationflags=subprocess.CREATE_NO_WINDOW,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=240)
        (out/leaf).write_bytes(value.stdout);need(value.returncode==0,leaf+' failed');return value.stdout
    source=Path(__file__);(out/'driver.py').write_bytes(source.read_bytes());driver_sha=sha(source)
    version=command([TOOLCHAIN/'rustc.exe','--version','--verbose'],'rust-version.txt');need(b'rustc 1.85.0 ' in version,'MSRV toolchain differs')
    command([TOOLCHAIN/'cargo.exe','--version'],'cargo-version.txt')
    cargo=TOOLCHAIN/'cargo.exe';manifest=ROOT/'tools/formal-tc-lock-coverage/rust-client/Cargo.toml'
    command([cargo,'build','--locked','--offline','--manifest-path',manifest,'--message-format=json'],'build.jsonl')
    tests=command([cargo,'test','--locked','--offline','--manifest-path',manifest],'tests.log');need(b'2 passed; 0 failed' in tests,'Actual MSRV fixture tests differ')
    binary=Path(env['CARGO_TARGET_DIR'])/'debug/seata-tc-lock-coverage-validation.exe';(out/'client.exe').write_bytes(binary.read_bytes())
    for row in inputs:need(sha(Path(row['path']))==row['sha256'],'Compile input drift')
    need(sha(source)==driver_sha,'MSRV driver drift')
    result=dict(status='SEALED_CLIENT_MSRV_BUILD_AND_FIXTURE_TESTS_VERIFIED',toolchain='1.85.0',observation_packet=str(packet),catalog_sha256=sha(packet/'catalog.json'),
        inherited_verified_compile_inputs=len(inputs),fixture_tests=2,fresh_network_cases=0,fresh_TC_DB_snapshots=0,TC_safety_gate_passed=False,binary_sha256=sha(binary),driver_sha256=driver_sha)
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    (out/'catalog.json').write_text(json.dumps(dict(members=[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]),indent=2)+'\n',encoding='utf-8')
    print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':run(Path(sys.argv[1]))
