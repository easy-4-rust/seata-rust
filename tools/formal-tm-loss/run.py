"""正式WP03组合门禁：基础Java、真实TM丢响应、双版本与验收器负控。"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib
import uuid
import importlib.util
import observer_controls
from validate import validate
spec=importlib.util.spec_from_file_location("formal_tm_library_controls",Path(__file__).resolve().parents[1]/"formal-tm/mutation_controls.py")
mutation_controls=importlib.util.module_from_spec(spec);spec.loader.exec_module(mutation_controls)

ROOT=Path(__file__).resolve().parents[2]
CACHE=Path('D:/codex-builds/seata-rust-cargo-home')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as stream:stream.write(data.encode('utf-8') if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def catalog(directory):
    for row in json.loads((directory/'catalog.json').read_bytes())['members']:
        path=(directory/row['path']).resolve()
        if not path.is_relative_to(directory) or sha(path)!=row['sha256']:raise ValueError('Evidence member differs:'+str(path))


def run(java_argument,peer_argument,loss_argument):
    java=(ROOT/java_argument).resolve();peer=(ROOT/peer_argument).resolve()
    if not java.is_relative_to(ROOT/'docs/phase-1/evidence/tm-java') or not peer.is_relative_to(ROOT/'docs/phase-1/evidence/protocol-peer-profile'):raise ValueError('Need local sealed Java/peer children')
    catalog(java);catalog(peer)
    live=json.loads((java/'result.json').read_bytes());profile=json.loads((peer/'result.json').read_bytes())
    if not live['succeeded'] or not live['input_closure_verified'] or not live['tc_process_cleaned'] or live['formal_tm_cases']!=6 or live['original_transactions']!=7 or live['class_origins']!=6:raise ValueError('Formal TM Java coverage/origins/cleanup differs')
    if (profile['positive_contexts'],profile['negative_complete_input_cases'],profile['formal_rust_tests'])!=(628,2050,74):raise ValueError('Incomplete peer regression')
    live_inputs=0
    for row in json.loads((java/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if sha(path)!=row['sha256']:raise ValueError('Java source/runtime input drift:'+str(path))
        if (path.is_relative_to(ROOT/'crates') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:live_inputs+=1
    loss=(ROOT/loss_argument).resolve()
    if not loss.is_relative_to(ROOT/'docs/phase-1/evidence/tm-loss-java'):raise ValueError('Need local formal TM loss evidence')
    catalog(loss);fault=json.loads((loss/'result.json').read_bytes())
    if not fault['succeeded'] or not fault['input_closure_verified'] or fault['cleanup_errors'] or fault['observer_data_given_to_client'] or not fault['formal_TM_execute_and_recover_API'] or not fault['simulated_RM_only']:raise ValueError('Formal TM loss boundary/cleanup differs')
    if fault['java_pin']!=live['java_pin'] or fault['jar_sha256']!=live['jar_sha256']:raise ValueError('Different original Java TC pins')
    if [case['mode'] for case in fault['cases']]!=['begin','commit','rollback'] or any(case['drop_count']!=1 or not case['ports_released'] or case['actual_class_origins']!=7 for case in fault['cases']):raise ValueError('Three actual TM faults incomplete')
    loss_inputs=0;audits=[]
    for row in json.loads((loss/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if sha(path)!=row['sha256']:raise ValueError('Formal TM loss input drift:'+str(path))
        if (path.is_relative_to(ROOT/'crates') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:loss_inputs+=1
    for mode in ['begin','commit','rollback']:
        case=loss/mode;rows=[json.loads(line) for line in (case/'events.jsonl').read_text(encoding='utf-8').splitlines()]
        nonce=json.loads((case/'owned-processes.json').read_bytes())['nonce']
        audits.append(validate(mode,(case/'client.log').read_text(encoding='utf-8'),rows,nonce))
    protocol_inputs=0
    for row in json.loads((peer/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if (path.is_relative_to(ROOT/'crates/seata_protocol') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock'] or (path.parent.parent==ROOT/'crates' and path.name=='Cargo.toml'):
            if sha(path)!=row['sha256']:raise ValueError('Protocol/manifest input drift:'+str(path))
            protocol_inputs+=1
    # 使用原始日志字节匹配ASCII字段，不猜测Windows JVM启动banner的文本编码。
    tc_log=(java/'tc-stdout.log').read_bytes();nonce=json.loads((java/'owned-process.json').read_bytes())['nonce']
    begins=re.findall(rb'Begin new global transaction applicationId: .*transactionName: (\S+),timeout:(\d+),xid:(\S+)',tc_log)
    commits=re.findall(rb'receive msg\[single\]: GlobalCommitRequest\{xid=\x27([^\x27]+)',tc_log)
    rollbacks=re.findall(rb'receive msg\[single\]: GlobalRollbackRequest\{xid=\x27([^\x27]+)',tc_log)
    actual_xids=[xid.decode('ascii') for _,_,xid in begins]
    if len(begins)!=7 or set(actual_xids)!=set(live['final_status_observations']) or len(commits)!=4 or len(rollbacks)!=3 or len(set(commits+rollbacks))!=7 or set(commits+rollbacks)!={xid for _,_,xid in begins}:raise ValueError('Actual TC seven Begin/four Commit/three Rollback proof differs')
    if any(not name.startswith(('tm-'+nonce+'-').encode()) for name,_,_ in begins):raise ValueError('TC transaction names differ from preselected operation nonce')
    if sorted(int(timeout) for _,timeout,_ in begins)!=[50,60000,60000,60000,60000,60000,60000]:raise ValueError('TC transaction timeout differs from formal API')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/tm-gate'/('run_'+stamp);out.mkdir(parents=True);print('TM_GATE_RUN '+str(out),flush=True)
    save(out/'actual-loss-wire-revalidation.json',audits)
    env=os.environ|dict(CARGO_HOME=str(CACHE),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    def command(args,leaf,extra=None):
        result=subprocess.run([str(x) for x in args],cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(out/leaf,result.stdout)
        if result.returncode:raise ValueError(leaf+': exit '+str(result.returncode))
        return result.stdout
    metadata=json.loads(command(['cargo','metadata','--offline','--locked','--format-version=1'],'cargo-metadata.json'))
    paths={ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),Path(mutation_controls.__file__),ROOT/'tools/formal-tm/live.py',Path(observer_controls.__file__),
        ROOT/'docs/phase-0/tm-tc-rm-call-chains.md',ROOT/'docs/phase-0/v1-implementation-acceptance-checklist.md',ROOT/'docs/adr/0008-wire-optionality-and-unknown-outcomes.md',
        Path('E:/workspaces/AGENTS.md'),Path('E:/workspaces/workspace-github-easy-4-rust/AGENTS.md')}
    paths.update(p for p in (ROOT/'crates').rglob('*') if p.is_file())
    paths.update((ROOT/'tools/formal-tm-loss').glob('*.py'))
    paths.update([ROOT/'tools/formal-transport-loss/live.py',ROOT/'tools/formal-transport-loss/loss_proxy.py',ROOT/'tools/formal-transport-loss/test_proxy.py',ROOT/'tools/formal-registration/live.py'])
    paths.update(loss/leaf for leaf in ['catalog.json','source-inputs.json','result.json','test-artifact.json'])
    paths.update(loss/mode/leaf for mode in ['begin','commit','rollback'] for leaf in ['class-origins.json','runtime-config-reference.json','result.json','client.log','events.jsonl','owned-processes.json'])
    paths.update(java/leaf for leaf in ['catalog.json','source-inputs.json','result.json','owned-process.json','class-origins.json','test-artifact.json','tc-stdout.log','client.log'])
    paths.update(peer/leaf for leaf in ['catalog.json','source-inputs.json','result.json'])
    dependencies=[]
    for package in tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'))['package']:
        if not package.get('source','').startswith('registry+'):continue
        leaf=package['name']+'-'+package['version']+'.crate';matches=list((CACHE/'registry/cache').glob('*/'+leaf))
        if len(matches)!=1 or sha(matches[0])!=package['checksum']:raise ValueError('Registry checksum differs:'+leaf)
        paths.add(matches[0]);dependencies.append(dict(name=package['name'],version=package['version'],path=str(matches[0]),sha256=package['checksum']))
    for package in metadata['packages']:
        if package.get('source'):
            folder=Path(package['manifest_path']).parent
            if not folder.is_relative_to(CACHE):raise ValueError('Dependency source outside task cache')
            paths.update(p for p in folder.rglob('*') if p.is_file())
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(out/snapshot,path.read_bytes());frozen.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    save(out/'source-inputs.json',frozen);save(out/'registry-archive-checksums.json',dependencies)
    save(out/'live-java-reference.json',dict(path=str(java),catalog_sha256=sha(java/'catalog.json'),compiled_rust_cargo_inputs_checked=live_inputs,
        native_tm_cases=6,actual_tc_transactions=7,actual_tc_commit_requests=4,actual_tc_rollback_requests=3,actual_class_origins=6,new_java_execution_in_this_script=False,new_java_execution_in_child_checkpoint=True,
        TM_response_loss_java_verified=False,RM_SQL_Undo_verified=False))
    save(out/'live-tm-loss-reference.json',dict(path=str(loss),catalog_sha256=sha(loss/'catalog.json'),compiled_rust_cargo_inputs_checked=loss_inputs,actual_cases=3,actual_class_origins_per_case=7,new_execution_in_this_script=False,new_execution_in_child_checkpoint=True,simulated_RM_only=True))
    save(out/'protocol-peer-reference.json',dict(path=str(peer),catalog_sha256=sha(peer/'catalog.json'),compiled_inputs_checked=protocol_inputs,new_execution_in_this_script=False))
    save(out/'actual-tc-log-verification.json',dict(begins=[dict(name=name.decode(),timeout=int(timeout),xid=xid.decode()) for name,timeout,xid in begins],commits=[x.decode() for x in commits],rollbacks=[x.decode() for x in rollbacks]))
    tdd=Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for leaf in ['tm-context-api-red.log','tm-manager-api-red.log','tm-scoped-api-red.log','tm-historical-unknown-red.log','tm-historical-unknown-green.log','tm-failed-begin-fields-api-red.log','tm-reconcile-revoke-red.log','tm-reconcile-revoke-green.log','tm-builder-debug-red.log','tm-resume-api-red.log','tm-resume-green.log','tm-recovery-authority-red.log','tm-recovery-fabrication-red.log','tm-recovery-proof-green.log','tm-loss-validator-api-red.log','tm-loss-observer-review-red.log','tm-loss-observer-review-green.log']:
        put(out/'tdd'/leaf,(tdd/leaf).read_bytes())
    save(out/'tdd/kind.json',dict(api_red='missing interfaces compiler errors, not runtime failures',actual_red=['response extraData in Debug','Failed/Finished erased prior unknown','Failed status retained prior retry permission','builder Debug exposed name','public recovery diagnostic could change original intent or fabricate a Launcher'],mutation_controls='intentional isolated regressions, not original product bugs'))
    command(['cargo','fmt','--all','--','--check'],'fmt.log')
    command(['cargo','clippy','--offline','--locked','--workspace','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','doc','--offline','--locked','--workspace','--no-deps'],'rustdoc.log',dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc'))
    command(['rustc','--version','--verbose'],'rust-version.log');command(['cargo','--version'],'cargo-version.log')
    artifacts=[]
    def tests(cargo,label,extra=None,helper=False):
        raw=command([cargo,'test','--offline','--locked',*(['-p','seata-tm','--example','tm_loss_live'] if helper else ['-p','seata-tm','-p','seata-transport','--lib','--tests']),'--message-format=json'],label+'-tests.jsonl',extra)
        if sorted(int(x) for x in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored',raw))!=([2] if helper else [0,7,7,8,9,10,10,13,13,14,24]):raise ValueError('Need28 TM+87 transport tests in11 targets')
        for line in raw.splitlines():
            if not line.startswith(b'{'):continue
            row=json.loads(line)
            if row.get('reason')=='compiler-artifact' and row.get('executable') and row.get('profile',{}).get('test'):
                path=Path(row['executable']);snapshot='artifacts/'+label+'/'+row['target']['name']+'.exe';put(out/snapshot,path.read_bytes());artifacts.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    tests('cargo','rust-1-99')
    tests('cargo','driver-rust-1-99',helper=True)
    msrv=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');command([msrv/'rustc.exe','--version','--verbose'],'msrv-version.log')
    tests(msrv/'cargo.exe','rust-1-85',dict(RUSTC=str(msrv/'rustc.exe'),RUSTDOC=str(msrv/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))
    tests(msrv/'cargo.exe','driver-rust-1-85',dict(RUSTC=str(msrv/'rustc.exe'),RUSTDOC=str(msrv/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'),helper=True)
    if len(artifacts)!=24:raise ValueError('Need24 actually executed test binaries')
    save(out/'test-artifacts.json',artifacts);controls=out/'mutation-controls/library';controls.mkdir(parents=True)
    if len(mutation_controls.run(ROOT,controls,env))!=10:raise ValueError('Need ten actual failed assertion controls')
    observers=out/'mutation-controls/observer';observers.mkdir()
    if len(observer_controls.run(ROOT,observers,env))!=9:raise ValueError('Need nine actual observer assertion failure controls')
    command(['python','-m','unittest','discover','-s','tools/formal-tm-loss','-p','test_validate.py','-v'],'observer-tests.log')
    command(['python','-m','unittest','discover','-s','tools/formal-transport-loss','-p','test_proxy.py','-v'],'proxy-tests.log')
    for row in frozen+artifacts:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Input/artifact drift:'+row['path'])
    save(out/'result.json',dict(formal_tm_tests=28,formal_transport_regressions=87,rust_1_99=True,msrv_1_85=True,mutation_controls=19,library_mutation_controls=10,observer_mutation_controls=9,validation_driver_rust_tests=2,python_observer_tests=13,python_proxy_tests=7,workspace_fmt_clippy_rustdoc=True,
        formal_tm_java_cases=9,basic_java_cases=6,response_loss_java_cases=3,actual_java_class_origins=27,typed_unknown_and_failed_status_preserved=True,scoped_context_and_six_propagations=True,
        no_automatic_transaction_replay=True,java_tc_started=False,external_services_started=0,new_java_execution_in_child_checkpoint=True,mysql_started=False,
        TM_response_loss_java_verified=True,WP03_complete=True,full_client_complete=False,production_recovery_complete=False,registry_dependencies=len(dependencies),network_fetches=0))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),artifacts=len(artifacts),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--java-run',required=True);parser.add_argument('--peer-run',required=True);parser.add_argument('--loss-run',required=True)
    args=parser.parse_args();run(args.java_run,args.peer_run,args.loss_run)
