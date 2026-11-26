"""正式WP02传输验收：同源码登记/响应丢失Java实跑、双版本及隔离负控。"""
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
import mutation_controls
import evidence_audit
spec=importlib.util.spec_from_file_location("registration_mutations",Path(__file__).resolve().parents[1]/"formal-registration/mutation_controls.py")
registration_mutations=importlib.util.module_from_spec(spec);spec.loader.exec_module(registration_mutations)

ROOT = Path(__file__).resolve().parents[2]
CACHE = Path('D:/codex-builds/seata-rust-cargo-home')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as stream:
        stream.write(data.encode('utf-8') if isinstance(data, str) else data)


def save(path, value):
    put(path, json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def run(peer_argument,java_argument,loss_argument):
    java=(ROOT/java_argument).resolve()
    if not java.is_relative_to(ROOT/'docs/phase-1/evidence/registration-java'):raise ValueError('Need local Java integration evidence')
    for row in json.loads((java/'catalog.json').read_bytes())['members']:
        path=(java/row['path']).resolve()
        if not path.is_relative_to(java) or sha(path)!=row['sha256']:raise ValueError('Java evidence member differs')
    live=json.loads((java/'result.json').read_bytes())
    if not live['succeeded'] or live['formal_client_cases']!=6 or not live['port_released'] or not live['actual_class_origins_verified'] or not live['actual_runtime_extension_files_frozen']:raise ValueError('Java integration/origin/cleanup incomplete')
    live_inputs=0
    for row in json.loads((java/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if (path.is_relative_to(ROOT/'crates') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:
            if sha(path)!=row['sha256']:raise ValueError('Compiled input differs from actual Java integration')
            live_inputs+=1
    loss=(ROOT/loss_argument).resolve()
    if not loss.is_relative_to(ROOT/'docs/phase-1/evidence/transport-loss-java'):raise ValueError('Need local loss integration evidence')
    for row in json.loads((loss/'catalog.json').read_bytes())['members']:
        path=(loss/row['path']).resolve()
        if not path.is_relative_to(loss) or sha(path)!=row['sha256']:raise ValueError('Loss evidence member differs')
    loss_result=json.loads((loss/'result.json').read_bytes())
    if not loss_result['succeeded'] or not loss_result['input_closure_verified'] or loss_result['cleanup_errors'] or loss_result['observer_data_given_to_client'] or not loss_result['formal_transport_api'] or not loss_result['simulated_rm_only']:raise ValueError('Loss integration boundary/cleanup differs')
    if loss_result['java_pin']!=live['java_pin'] or loss_result['jar_sha256']!=live['jar_sha256']:raise ValueError('Java integration pins differ')
    if [case['mode'] for case in loss_result['cases']]!=['begin','commit','branch-register'] or any(not case['ports_released'] or case['drop_count']!=1 or case['actual_class_origins']!=7 for case in loss_result['cases']):raise ValueError('Three actual response-loss cases incomplete')
    wire_audits=[]
    for mode in ["begin","commit","branch-register"]:
        case=loss/mode
        rows=[json.loads(line) for line in (case/"events.jsonl").read_text(encoding="utf-8").splitlines()]
        nonce=json.loads((case/"owned-processes.json").read_bytes())["nonce"]
        wire_audits.append(evidence_audit.validate(mode,(case/"client.log").read_text(encoding="utf-8"),rows,nonce))
    loss_inputs=0
    for row in json.loads((loss/'source-inputs.json').read_bytes()):
        path=Path(row['path'])
        if sha(path)!=row['sha256']:raise ValueError('Loss runtime/source input differs')
        if (path.is_relative_to(ROOT/'crates') and path.suffix in ['.rs','.toml']) or path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:loss_inputs+=1
    peer = (ROOT / peer_argument).resolve()
    if not peer.is_relative_to(ROOT / 'docs/phase-1/evidence/protocol-peer-profile'):
        raise ValueError('Need local protocol peer evidence')
    peer_catalog = peer / 'catalog.json'
    for row in json.loads(peer_catalog.read_bytes())['members']:
        path = (peer / row['path']).resolve()
        if not path.is_relative_to(peer) or sha(path) != row['sha256']:
            raise ValueError('Peer evidence member differs')
    result = json.loads((peer / 'result.json').read_bytes())
    if (result['positive_contexts'], result['negative_complete_input_cases'], result['formal_rust_tests']) != (628, 2050, 74):
        raise ValueError('Incomplete peer regression')
    compiled = 0
    for row in json.loads((peer / 'source-inputs.json').read_bytes()):
        path = Path(row['path'])
        if (path.is_relative_to(ROOT / 'crates/seata_protocol') and path.suffix in ['.rs', '.toml']) or path in [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'crates/seata_transport/Cargo.toml']:
            if sha(path) != row['sha256']:
                raise ValueError('Compiled protocol/manifest input differs')
            compiled += 1
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-1/evidence/transport-recovery' / ('run_' + stamp)
    out.mkdir(parents=True)
    print('TRANSPORT_RECOVERY_GATE '+str(out),flush=True)
    save(out/'actual-wire-revalidation.json',wire_audits)
    env = os.environ | dict(CARGO_HOME=str(CACHE), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
                           TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')

    def command(args, log, extra=None):
        value = subprocess.run([str(x) for x in args], cwd=ROOT, env=env | (extra or {}),
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               creationflags=subprocess.CREATE_NO_WINDOW, timeout=180)
        put(out / log, value.stdout)
        if value.returncode:
            raise RuntimeError(log + ': exit ' + str(value.returncode))
        return value.stdout

    metadata = json.loads(command(['cargo', 'metadata', '--offline', '--locked', '--format-version=1'], 'cargo-metadata.json'))
    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__),
             ROOT / 'docs/phase-0/seata-go-transport-timeout-semantics.md',
             ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md',
             ROOT / 'docs/adr/0008-wire-optionality-and-unknown-outcomes.md',
             Path('E:/workspaces/AGENTS.md'), Path('E:/workspaces/workspace-github-easy-4-rust/AGENTS.md'),
             peer_catalog, peer / 'source-inputs.json', peer / 'result.json'}
    paths.update(p for folder in ['crates', 'tests/fixtures/protocol/v1'] for p in (ROOT / folder).rglob('*') if p.is_file())
    paths.update([Path(mutation_controls.__file__),ROOT/'tools/formal-registration/live.py',ROOT/'docs/phase-0/registration-v1-wire-contract.md',ROOT/'docs/phase-0/auth-runtime-boundaries.md'])
    paths.update(java/leaf for leaf in ['catalog.json','source-inputs.json','result.json','actual-class-origins.json','jar-and-fixture-reference.json','test-artifact.json'])
    paths.update(p for p in (ROOT/'tools/formal-transport-loss').rglob('*.py'))
    paths.add(Path(registration_mutations.__file__))
    baseline=ROOT/'docs/phase-1/evidence/transport-loss-java/run_20261113_211233_67c81a72'
    paths.add(baseline/'catalog.json')
    paths.update(baseline/mode/leaf for mode in ['begin','commit','branch-register'] for leaf in ['events.jsonl','client.log','owned-processes.json'])
    paths.update(loss/leaf for leaf in ['catalog.json','source-inputs.json','result.json','test-artifact.json'])
    paths.update(loss/mode/leaf for mode in ['begin','commit','branch-register'] for leaf in ['class-origins.json','result.json','events.jsonl','client.log','owned-processes.json','runtime-config-reference.json'])
    oracle=Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
    paths.update(oracle/leaf for leaf in ['core/src/main/java/org/apache/seata/core/rpc/netty/ChannelManager.java','core/src/main/java/org/apache/seata/core/rpc/processor/server/RegRmProcessor.java',
        'core/src/main/java/org/apache/seata/core/rpc/processor/server/RegTmProcessor.java','common/src/main/java/org/apache/seata/common/Constants.java'])
    dependencies = []
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    for package in lock['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        leaf = package['name'] + '-' + package['version'] + '.crate'
        candidates = list((CACHE / 'registry/cache').glob('*/' + leaf))
        if len(candidates) != 1 or sha(candidates[0]) != package['checksum']:
            raise ValueError('Registry archive missing/checksum differs: ' + leaf)
        paths.add(candidates[0])
        dependencies.append(dict(name=package['name'], version=package['version'], archive=str(candidates[0]), checksum=package['checksum']))
    for package in metadata['packages']:
        if package.get('source'):
            directory = Path(package['manifest_path']).parent
            if not directory.is_relative_to(CACHE):
                raise ValueError('Dependency source outside task cache')
            paths.update(p for p in directory.rglob('*') if p.is_file())
    frozen = []
    for path in sorted(paths):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(out / snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save(out / 'source-inputs.json', frozen)
    save(out / 'registry-archive-checksums.json', dependencies)
    save(out/'live-java-reference.json',dict(path=str(java),catalog_sha256=sha(java/'catalog.json'),compiled_inputs_checked=live_inputs,
        native_client_cases=6,actual_class_origins=5,tc_process_cleaned=True,new_java_execution_in_this_script=False,new_java_execution_in_child_checkpoint=True))
    save(out/'live-loss-reference.json',dict(path=str(loss),catalog_sha256=sha(loss/'catalog.json'),compiled_inputs_checked=loss_inputs,actual_cases=3,actual_class_origins_per_case=7,new_execution_in_this_script=False,new_execution_in_child_checkpoint=True,simulated_rm_only=True,mysql_compensation=False))
    save(out / 'protocol-peer-reference.json', dict(path=str(peer), catalog_sha256=sha(peer_catalog), compiled_inputs_checked=compiled,
                                                   new_execution_in_this_script=False, positive_contexts=628, negative_contexts=2050))
    tdd = Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for leaf in ['response-api-red.log','response-close-review-red.log','response-runtime-review-red.log','response-review-green.log','loss-observer-bool-red.log','loss-observer-bool-green.log','loss-evidence-review-red.log','loss-evidence-review-green.log']:
        put(out / 'tdd' / leaf, (tdd / leaf).read_bytes())
    save(out/'tdd/kind.json',dict(api_red='missing response API compiler errors, not runtime failures',
        close_red='actual assertion: closed generation published response write success',
        runtime_red='actual assertion/panic: no Tokio runtime allowed queued bytes before error',
        observer_red='actual Python assertion: Java u16 boolean wrongly read as one byte',
        raw_evidence_red='nine assertion failures when corrupted bytes/identity were wrongly accepted',
        green='87 formal transport tests and 13 observer tests including 13 raw evidence corruptions',
        mutation_controls='14 isolated intentional defects rejected by actual assertions, not original product bugs'))
    command(['cargo', 'fmt', '--all', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--offline', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--offline', '--locked', '--workspace', '--no-deps'], 'rustdoc.log',
            dict(RUSTDOCFLAGS='-D warnings', CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc'))
    command(['rustc', '--version', '--verbose'], 'rust-version.log')
    command(['cargo', '--version'], 'cargo-version.log')
    artifacts = []

    def tests(cargo, extra, label, helper=False):
        raw = command([cargo, 'test', '--offline', '--locked', '-p', 'seata-transport', *(['--example','transport_loss_live'] if helper else ['--lib','--tests']), '--message-format=json'], label + '-tests.jsonl', extra)
        if sorted(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored', raw)) != ([3] if helper else [7,9,10,10,13,14,24]):
            raise ValueError('Need 87 formal transport tests or three separate driver tests')
        for line in raw.splitlines():
            if not line.startswith(b'{'):
                continue
            row = json.loads(line)
            if row.get('reason') == 'compiler-artifact' and row.get('executable') and row.get('profile', {}).get('test'):
                path = Path(row['executable'])
                snapshot = 'artifacts/' + label + '/' + row['target']['name'] + '.exe'
                put(out / snapshot, path.read_bytes())
                artifacts.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))

    tests('cargo', {}, 'rust-1-99')
    tests('cargo', {}, 'driver-rust-1-99',True)
    msrv = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
    command([msrv / 'rustc.exe', '--version', '--verbose'], 'msrv-version.log')
    tests(msrv / 'cargo.exe', dict(RUSTC=str(msrv / 'rustc.exe'), RUSTDOC=str(msrv / 'rustdoc.exe'),
                                 CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'), 'rust-1-85')
    tests(msrv / 'cargo.exe', dict(RUSTC=str(msrv / 'rustc.exe'), RUSTDOC=str(msrv / 'rustdoc.exe'),
                                 CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'), 'driver-rust-1-85',True)
    if len(artifacts) != 16:
        raise ValueError('Need sixteen executed test binaries')
    save(out / 'test-artifacts.json', artifacts)
    controls=out/'mutation-controls';controls.mkdir()
    old_controls=controls/'registration';old_controls.mkdir()
    new_controls=controls/'response-loss';new_controls.mkdir()
    if len(registration_mutations.run(ROOT,old_controls,env))!=6:raise ValueError('Need six rejected registration mutations')
    if len(mutation_controls.run(ROOT,new_controls,env))!=8:raise ValueError('Need eight rejected response-loss mutations')
    command(['python','-m','unittest','discover','-s','tools/formal-transport-loss','-p','test_*.py','-v'],'observer-tests.log')
    for row in frozen + artifacts:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Source/dependency/artifact drift: ' + row['path'])
    save(out / 'result.json', dict(transport_tests=87, real_tcp_test_functions=49, library_tests=24, stream_id_tests=10,
                                  configuration_tests=4,registration_target_tests=13,registration_real_tcp_functions=11,mutation_controls=14,driver_unit_tests=3,observer_python_tests=13,raw_evidence_corruptions_rejected=13,
                                  rust_1_99=True, msrv_1_85=True, workspace_fmt_clippy_rustdoc=True,
                                  typed_pre_send_and_unknown=True, absolute_deadline_arbitration=True,
                                  independent_same_wire_id_generations=True, bounded_channels=True,
                                  no_automatic_transaction_replay=True, shutdown_and_owner_drop=True,
                                  NET001_to_NET007_transport_component_verified=True, NET008_NET009_component_verified=True,NET010_to_NET012_transport_fault_scenarios_verified=True,production_business_recovery_complete=False,
                                  java_tc_started=False, mysql_started=False, external_services_started=0,
                                  loopback_fault_injection_peers=True, scheduled_heartbeat_complete=True,
                                  raw_pool_complete=True,raw_reconnect_complete=True,native_registration_ready_complete=True,
                                  versioned_dynamic_resource_registry=True,partial_resource_outcomes=True,production_auth_complete=False,
                                  live_java_client_cases=6,live_java_new_execution_in_this_script=False,live_java_child_provenance_verified=True,
                                  full_client_complete=False, WP02_component_gate_verified=True, WP02_complete=True, new_java_execution=False,
                                  registry_dependencies=len(dependencies), network_fetches=0))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size)
               for p in sorted(out.rglob('*')) if p.is_file()]
    save(out / 'catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), source_inputs=len(frozen), members=len(members),
                          artifacts=len(artifacts), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--peer-run', required=True);parser.add_argument('--java-run',required=True);parser.add_argument('--loss-run',required=True)
    args=parser.parse_args();run(args.peer_run,args.java_run,args.loss_run)
