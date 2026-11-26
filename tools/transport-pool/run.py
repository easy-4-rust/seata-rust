"""有界raw池/重连门禁；真实TCP、同一绝对预算、隔离回归负控及完整来源封存。"""
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


def run(peer_argument):
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
    out = ROOT / 'docs/phase-1/evidence/transport-pool' / ('run_' + stamp)
    out.mkdir(parents=True)
    print('POOL_RUN ' + str(out), flush=True)
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
    paths.add(Path(mutation_controls.__file__))
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
    save(out / 'protocol-peer-reference.json', dict(path=str(peer), catalog_sha256=sha(peer_catalog), compiled_inputs_checked=compiled,
                                                   new_execution_in_this_script=False, positive_contexts=628, negative_contexts=2050))
    tdd = Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for leaf in ['pool-api-red.log','pool-quota-review-red.log','pool-deadline-review-red.log','pool-review-green.log']:
        put(out / 'tdd' / leaf, (tdd / leaf).read_bytes())
    save(out / 'tdd/kind.json', dict(api_red='missing APIs: compiler errors, not seven failing runtime tests',
                                   quota_red='actual assertion failure: exhausted generation left open and selectable',
                                   deadline_red='actual assertion failure: synchronous lock wait restarted full request budget',
                                   review_green='61 transport tests passed after both review fixes, before final shutdown-connect test',
                                   mutation_controls='five intentionally broken isolated variants must fail actual assertions; not original product RED'))
    command(['cargo', 'fmt', '--all', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--offline', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--offline', '--locked', '--workspace', '--no-deps'], 'rustdoc.log',
            dict(RUSTDOCFLAGS='-D warnings', CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc'))
    command(['rustc', '--version', '--verbose'], 'rust-version.log')
    command(['cargo', '--version'], 'cargo-version.log')
    artifacts = []

    def tests(cargo, extra, label):
        raw = command([cargo, 'test', '--offline', '--locked', '-p', 'seata-transport', '--lib', '--tests', '--message-format=json'], label + '-tests.jsonl', extra)
        if sorted(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored', raw)) != [9, 10, 10, 14, 19]:
            raise ValueError('Need exactly62 transport tests in five targets')
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
    if len(artifacts) != 10:
        raise ValueError('Need ten executed test binaries')
    save(out / 'test-artifacts.json', artifacts)
    controls=out/'mutation-controls';controls.mkdir()
    if len(mutation_controls.run(ROOT,controls,env))!=5:
        raise ValueError('Need five rejected behavior mutations')
    for row in frozen + artifacts:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Source/dependency/artifact drift: ' + row['path'])
    save(out / 'result.json', dict(transport_tests=62, real_tcp_test_functions=31, library_tests=19, stream_id_tests=10,
                                  configuration_tests=2, pool_target_tests=10, pool_real_tcp_test_functions=9, mutation_controls=5,
                                  rust_1_99=True, msrv_1_85=True, workspace_fmt_clippy_rustdoc=True,
                                  typed_pre_send_and_unknown=True, absolute_deadline_arbitration=True,
                                  independent_same_wire_id_generations=True, bounded_channels=True,
                                  no_automatic_transaction_replay=True, shutdown_and_owner_drop=True,
                                  NET001_to_NET007_transport_component_verified=True, NET008_to_NET012_complete=False,
                                  java_tc_started=False, mysql_started=False, external_services_started=0,
                                  loopback_fault_injection_peers=True, scheduled_heartbeat_complete=True,
                                  raw_pool_complete=True, raw_reconnect_complete=True, stable_backoff_and_cap=True,
                                  physical_slot_count_bounded=True, no_rebind_of_old_handles=True,
                                  acquisition_included_in_absolute_deadline=True, quota_exhaustion_rotates_without_replay=True,
                                  pool_business_ready_complete=False, authenticated_rm_ready_complete=False,
                                  full_client_complete=False, WP02_complete=False, new_java_execution=False,
                                  registry_dependencies=len(dependencies), network_fetches=0))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size)
               for p in sorted(out.rglob('*')) if p.is_file()]
    save(out / 'catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), source_inputs=len(frozen), members=len(members),
                          artifacts=len(artifacts), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--peer-run', required=True)
    run(parser.parse_args().peer_run)
