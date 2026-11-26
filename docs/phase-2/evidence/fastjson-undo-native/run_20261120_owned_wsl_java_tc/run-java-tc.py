from pathlib import Path
import hashlib
import importlib.util
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile

root=Path.cwd()
evidence=Path(__file__).resolve().parent
role,version=sys.argv[1:]
assert role in ('registration-loss','compensation-retry') and version in ('1.99','1.85')
registered=role=='registration-loss'
out=evidence/f'{role}-{version}-verified3'
out.mkdir()
work=Path(tempfile.mkdtemp(prefix='seata_wsl_java_'+role+'_',dir='D:/codex-builds'))
obs=work/'observations';obs.mkdir()
env=os.environ|dict(SEATA_MYSQL_PORT='43318',
    SEATA_MYSQL_DATABASE='undo_wsl_java_validation' if registered else 'undo_wsl_java_retry'+version.split('.')[1],
    SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only',SEATA_UNDO_STORE_OUT=str(obs),
    SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql')
cargo='cargo'
if version=='1.85':
    tools=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
    cargo=str(tools/'cargo.exe')
    env.update(RUSTC=str(tools/'rustc.exe'),RUSTDOC=str(tools/'rustdoc.exe'),
        RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv')
if not registered:
    with socket.socket() as s:
        s.bind(('127.0.0.1',0))
        port=str(s.getsockname()[1])
    env.update(SEATA_MYSQL_PORT=port,SEATA_RM_MYSQL_PROXY_PORT=port)
directory='formal-callback-ownership' if registered else 'formal-commit-direction'
spec=importlib.util.spec_from_file_location('wsl_pinned_java_runtime',root/f'tools/{directory}/java_runtime.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
historical=json.loads(module.reg.HISTORICAL.read_bytes())
restoration=json.loads((evidence/'auth-fixture-restoration.json').read_bytes())
module.reg.FIXTURE=Path(restoration['fixture'])
assert restoration['class_sha256']==historical['extension_class_sha256']
assert restoration['service_sha256']==historical['extension_service_sha256']
runtime=module.Runtime(out,work,env)
try:
    env['SEATA_MYSQL_PORT']='43318'
    target='live_callback_java' if registered else 'live_commit_direction_java'
    args=[] if registered else ['original_java_tc_retries_compensation_after_mysql_acquisition_rejection','--exact']
    cmd=[cargo,'test','--locked','--offline','-p','seata-db-mysql','--test',target,'--',*args,'--ignored','--nocapture','--test-threads=1']
    r=subprocess.run(cmd,cwd=root,env=env,timeout=110,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/'test.log').write_bytes(r.stdout)
    print(r.stdout.decode(errors='replace')[-4600:])
    assert r.returncode==0
finally:
    runtime.stop()
    shutil.copytree(obs,out/'observations')
if registered:
    observation=json.loads((obs/'java-callback-ownership.json').read_bytes())
    module.support.save(out/'java-tc/validation.json',module.validate(out/'java-tc',observation))
    module.support.save(out/'java-tc/negative-controls.json',module.negative_controls(out/'java-tc',observation))
print(role,version,'passed')
