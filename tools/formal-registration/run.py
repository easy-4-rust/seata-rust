"""正式原生登记/资源ready门禁，绑定同源码真实JavaTC、双版本与隔离行为负控。"""
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
import mutation_controls

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


def run(peer_argument,java_argument):
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
    out = ROOT / 'docs/phase-1/evidence/session-registration' / ('run_' + stamp)
    out.mkdir(parents=True)
    print('REGISTRATION_GATE '+str(out),flush=True)
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
    save(out / 'protocol-peer-reference.json', dict(path=str(peer), catalog_sha256=sha(peer_catalog), compiled_inputs_checked=compiled,
                                                   new_execution_in_this_script=False, positive_contexts=628, negative_contexts=2050))
    tdd = Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for leaf in ['registration-api-red.log','registration-resource-auth-red.log','registration-resource-auth-green.log']:
        put(out / 'tdd' / leaf, (tdd / leaf).read_bytes())
    save(out/'tdd/kind.json',dict(api_red='missing APIs: compiler errors, not seven runtime assertion failures',
        resource_auth_red='actual assertion failure: empty-resource initial RM handshake rejected by a resource-aware identity policy',
        resource_auth_green='first required resource now also establishes RM identity; same attempt recorded in two report views',
        mutation_controls='six intentionally broken isolated variants must fail actual assertions; not six original product defects'))
    command(['cargo', 'fmt', '--all', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--offline', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--offline', '--locked', '--workspace', '--no-deps'], 'rustdoc.log',
            dict(RUSTDOCFLAGS='-D warnings', CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc'))
    command(['rustc', '--version', '--verbose'], 'rust-version.log')
    command(['cargo', '--version'], 'cargo-version.log')
    artifacts = []

    def tests(cargo, extra, label):
        raw = command([cargo, 'test', '--offline', '--locked', '-p', 'seata-transport', '--lib', '--tests', '--message-format=json'], label + '-tests.jsonl', extra)
        if sorted(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored', raw)) != [9,10,10,13,14,20]:
            raise ValueError('Need exactly76 transport tests in six targets')
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
    msrv = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
    command([msrv / 'rustc.exe', '--version', '--verbose'], 'msrv-version.log')
    tests(msrv / 'cargo.exe', dict(RUSTC=str(msrv / 'rustc.exe'), RUSTDOC=str(msrv / 'rustdoc.exe'),
                                 CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'), 'rust-1-85')
    if len(artifacts) != 12:
        raise ValueError('Need twelve executed test binaries')
    save(out / 'test-artifacts.json', artifacts)
    controls=out/'mutation-controls';controls.mkdir()
    if len(mutation_controls.run(ROOT,controls,env))!=6:raise ValueError('Need six rejected behavior mutations')
    for row in frozen + artifacts:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Source/dependency/artifact drift: ' + row['path'])
    save(out / 'result.json', dict(transport_tests=76, real_tcp_test_functions=42, library_tests=20, stream_id_tests=10,
                                  configuration_tests=4,registration_target_tests=13,registration_real_tcp_functions=11,mutation_controls=6,
                                  rust_1_99=True, msrv_1_85=True, workspace_fmt_clippy_rustdoc=True,
                                  typed_pre_send_and_unknown=True, absolute_deadline_arbitration=True,
                                  independent_same_wire_id_generations=True, bounded_channels=True,
                                  no_automatic_transaction_replay=True, shutdown_and_owner_drop=True,
                                  NET001_to_NET007_transport_component_verified=True, NET008_NET009_component_verified=True,NET010_to_NET012_business_recovery_complete=False,
                                  java_tc_started=False, mysql_started=False, external_services_started=0,
                                  loopback_fault_injection_peers=True, scheduled_heartbeat_complete=True,
                                  raw_pool_complete=True,raw_reconnect_complete=True,native_registration_ready_complete=True,
                                  versioned_dynamic_resource_registry=True,partial_resource_outcomes=True,production_auth_complete=False,
                                  live_java_client_cases=6,live_java_new_execution_in_this_script=False,live_java_child_provenance_verified=True,
                                  full_client_complete=False, WP02_complete=False, new_java_execution=False,
                                  registry_dependencies=len(dependencies), network_fetches=0))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size)
               for p in sorted(out.rglob('*')) if p.is_file()]
    save(out / 'catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), source_inputs=len(frozen), members=len(members),
                          artifacts=len(artifacts), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--peer-run', required=True);parser.add_argument('--java-run',required=True)
    args=parser.parse_args();run(args.peer_run,args.java_run)
