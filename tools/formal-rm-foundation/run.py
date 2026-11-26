"""Freeze actual RM foundation inputs, run two Rust versions and isolated failing controls."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[2]
CACHE = Path('D:/codex-builds/seata-rust-cargo-home')
MSRV = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(*, runtime_component=False, extra_controls=(), source_inputs_extra=()):
    expected_total,expected_rm,expected_binaries=(234,45,26) if runtime_component else (212,23,23)
    profile='rm-runtime' if runtime_component else 'rm-foundation'
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-1/evidence' / profile / ('run_' + stamp)
    runtime = Path('D:/codex-builds') / ('formal_'+profile.replace('-','_')+'_' + stamp)
    out.mkdir(parents=True)
    runtime.mkdir()
    print('RM_FOUNDATION_RUN ' + str(out), flush=True)
    env = os.environ | dict(CARGO_HOME=str(CACHE), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
                            TEMP='D:/Temp', TMP='D:/Temp', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')

    def put(relative, data):
        path = out / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data.encode('utf-8') if isinstance(data, str) else data)

    def save(relative, data):
        put(relative, json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def command(args, log, extra=None, cwd=ROOT, expect_failure=False):
        value = subprocess.run([str(v) for v in args], cwd=cwd, env=env | (extra or {}),
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               creationflags=subprocess.CREATE_NO_WINDOW, timeout=240)
        put(log, value.stdout)
        if expect_failure:
            if value.returncode != 101 or b'test result: FAILED.' not in value.stdout or b'panicked at' not in value.stdout or b'error[E' in value.stdout:
                raise ValueError('Control must actually compile and fail an assertion: ' + log)
        elif value.returncode:
            raise ValueError(log + ': exit ' + str(value.returncode))
        return value.stdout

    paths = {ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve(),
             ROOT / 'docs/phase-0/tm-tc-rm-call-chains.md', ROOT / 'docs/phase-0/v1-implementation-acceptance-checklist.md',
             ROOT / 'docs/adr/0008-wire-optionality-and-unknown-outcomes.md'}
    paths.update(source_inputs_extra)
    save('runner-profile.json',dict(profile=profile,expected_total=expected_total,expected_rm=expected_rm,expected_binaries=expected_binaries,real_Java_TC_execution=False))
    paths.update(path for path in (ROOT / 'crates').rglob('*') if path.is_file())
    paths.update(path for path in (ROOT / 'tests/fixtures/protocol/v1').rglob('*') if path.is_file())
    dependencies = []
    dependency_sources = {}
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], 'metadata.json'))
    active_ids = {row['id'] for row in metadata['resolve']['nodes']}
    active_registry = {(row['name'], row['version']) for row in metadata['packages'] if row['id'] in active_ids and row.get('source')}
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))['package']:
        if not package.get('source', '').startswith('registry+'):
            continue
        name = package['name'] + '-' + package['version']
        archive = CACHE / 'registry/cache/index.crates.io-1949cf8c6b5b557f' / (name + '.crate')
        if sha(archive.read_bytes()) != package['checksum']:
            raise ValueError('Registry archive differs from lock: ' + name)
        source = CACHE / 'registry/src/index.crates.io-1949cf8c6b5b557f' / name
        count = 0
        active = (package['name'], package['version']) in active_registry
        with tarfile.open(archive, 'r:gz') as bundle:
            for member in bundle.getmembers():
                if not active or not member.isfile():
                    continue
                relative = Path(member.name).relative_to(name)
                if '..' in relative.parts:
                    raise ValueError('Unsafe dependency archive path')
                with bundle.extractfile(member) as stream:
                    expected = stream.read()
                    actual_source = source / relative
                    if actual_source.read_bytes() != expected:
                        raise ValueError('Unpacked dependency differs from pinned archive: ' + name)
                    paths.add(actual_source)
                    dependency_sources[actual_source] = sha(expected)
                count += 1
        paths.add(archive)
        dependencies.append(dict(name=package['name'], version=package['version'], sha256=package['checksum'], active_on_windows_msvc=active, unpacked_files_verified=count,
                                 other_target_archive_only=not active))
    save('dependencies.json', dependencies)
    frozen = []
    for path in sorted(paths):
        data = path.read_bytes()
        if path in dependency_sources and sha(data) != dependency_sources[path]:
            raise ValueError('Dependency changed before snapshot: ' + str(path))
        snapshot = 'sources/' + sha(str(path).encode())[:24] + '.bin'
        put(snapshot, data)
        frozen.append(dict(path=str(path), sha256=sha(data), snapshot=snapshot))
    save('source-inputs.json', frozen)
    red = Path('D:/codex-builds/seata-rust-protocol-start-20261113')
    for name in ['rm-journal-api-red.log', 'rm-dispatch-api-red.log', 'rm-cancelled-disk-behavior-red.log']:
        put('tdd/' + name, (red / name).read_bytes())
    save('tdd/classification.json', dict(journal_api_red='missing implementation module compiler failure',
        dispatch_api_red='missing public API compiler failure', cancelled_disk_red='actual assertion failure before guard/ordering fix',
        initial_budget_fixture_correction='200 bytes fit both records; corrected to 160 and asserted the premise; not a product bug'))
    if runtime_component:
        for name in ['rm-native-api-red.log','rm-shutdown-panic-behavior-red.log']:
            put('tdd/'+name,(red/name).read_bytes())
        save('tdd/runtime-classification.json',dict(native_api_red='missing public API compiler failure',shutdown_panic_red='actual assertion: cleanup panic discarded as successful shutdown',
             deadline_and_borrowed_bounds='source review fix plus two executed boundary tests; no pre-fix runtime RED claimed',
             first_native_failure='test folder clock collision; Atomic counter correction, not production failure',
             dynamic_resource_trial='Initial test wrongly waited for callback before registration ACK, contradicting approved only-after-ready contract. Trial inbound pumping reverted byte-for-byte. Final PING/PONG test preserves original readiness and queued callbacks; trial is not claimed as production fix.'))
    command(['git', 'rev-parse', 'HEAD'], 'head.log')
    command(['cargo', 'fmt', '-p', 'seata-rm', '--', '--check'], 'fmt.log')
    command(['cargo', 'clippy', '--offline', '--locked', '-p', 'seata-rm', '--all-targets', '--', '-D', 'warnings'], 'clippy.log')
    command(['cargo', 'doc', '--offline', '--locked', '-p', 'seata-rm', '--no-deps'], 'rustdoc.log',
            dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc', RUSTDOCFLAGS='-D warnings'))
    artifacts, executions = [], []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
         dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        command([extra.get('RUSTC', 'rustc'), '--version', '--verbose'], label + '/version.log', extra)
        raw = command([cargo, 'test', '--locked', '--offline', '--workspace', '--lib', '--tests', '--message-format=json', '--', '--nocapture'], label + '/tests.jsonl', extra)
        passed = sum(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw))
        if passed != expected_total:
            raise ValueError('Expected 74 protocol +87 transport +28 TM +'+str(expected_rm)+' RM ='+str(expected_total)+' executed tests; got '+str(passed))
        folders = re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1 children=3 terminated=true', raw)
        if len(folders) != 2:
            raise ValueError('Need two actual forced-process-restart parent observations')
        for folder, phase in folders:
            folder = Path(folder.decode())
            if (folder / 'apply.audit').read_bytes() != b'1\n' or (folder / 'recover.audit').read_bytes() != b'1\n':
                raise ValueError('Repeated resource action or missing recovery')
            for path in sorted(folder.iterdir()):
                if path.is_file():
                    put(label + '/processes/' + phase.decode() + '/' + path.name, path.read_bytes())
        count = 0
        for line in raw.splitlines():
            if not line.startswith(b'{'):
                continue
            row = json.loads(line)
            if row.get('reason') == 'compiler-artifact' and row.get('executable') and row.get('profile', {}).get('test'):
                path = Path(row['executable'])
                snapshot = 'artifacts/' + label + '/' + row['target']['name'] + '.exe'
                data = path.read_bytes()
                put(snapshot, data)
                artifacts.append(dict(path=str(path), sha256=sha(data), snapshot=snapshot))
                count += 1
        if count != expected_binaries:
            raise ValueError('Need '+str(expected_binaries)+' actually executed workspace test binaries per version; got '+str(count))
        executions.append(dict(toolchain=label, formal_tests=passed, rm_tests=expected_rm, test_binaries=count, process_children=6,
                               helper_entry_ignored_in_parent=True, helper_actually_invoked_in_owned_children=True))
        print(label+':'+str(expected_total)+' tests,'+str(count)+' binaries,6 owned children passed',flush=True)
    save('executions.json', executions)
    save('test-artifacts.json', artifacts)

    changes = [
        ('crc', 'store/file_branch_store.rs', 'if crc32fast::hash(&record[..checksum_offset]) != expected {', 'if false {', 'journal', 'damaged_version_checksum_or_partial_tail'),
        ('identity', 'store/file_branch_store.rs', 'Some(value) if value.identity != *identity || value.phase != phase => {', 'Some(value) if false => {', 'journal', 'original_identity_and_phase'),
        ('pending', 'manager/branch_dispatcher.rs', 'lease.mark_pending().await?;', '', 'dispatch', 'cancelled_apply_leaves_pending'),
        ('recovery', 'manager/branch_dispatcher.rs', 'resource.recover(&identity, phase).await?', 'BranchResolution::NotApplied', 'dispatch', 'pending_unknown_is_queried'),
        ('cached-completion', 'manager/branch_dispatcher.rs', 'return Ok(status(phase, BranchResolution::Applied))', 'return Ok(status(phase, BranchResolution::Unknown))', 'dispatch', 'exact_duplicate_uses_completed_record'),
    ]
    changes.extend(extra_controls)
    controls = []
    for name, leaf, before, after, test, case in changes:
        copied = runtime / name
        shutil.copytree(ROOT / 'crates', copied / 'crates')
        shutil.copy2(ROOT / 'Cargo.toml', copied / 'Cargo.toml')
        shutil.copy2(ROOT / 'Cargo.lock', copied / 'Cargo.lock')
        target = copied / 'crates/seata_rm/src' / leaf
        original = target.read_text('utf-8')
        if original.count(before) != 1:
            raise ValueError('Mutation anchor must be unique: ' + name)
        changed = original.replace(before, after)
        target.write_text(changed, encoding='utf-8', newline='\n')
        put('controls/' + name + '.rs', changed)
        selection=['--lib'] if test=='__lib__' else ['--test',test]
        command(['cargo', 'test', '--locked', '--offline', '-p', 'seata-rm']+selection+[case], 'controls/' + name + '.log',
                dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-rm-negative'), cwd=copied, expect_failure=True)
        controls.append(dict(name=name, test=test, case=case, original_sha256=sha(original.encode()), mutated_sha256=sha(changed.encode()), actual_compiled_assertion_failure=True))
        print('assertion control:' + name + ' detected', flush=True)
    save('controls.json', controls)
    for row in frozen:
        if sha(Path(row['path']).read_bytes()) != row['sha256']:
            raise ValueError('Source changed during run: ' + row['path'])
    save('result.json', dict(status='verified_RM_runtime_component_synthetic_TC_only' if runtime_component else 'verified_RM_foundation_only', source_inputs=len(frozen), dependencies=len(dependencies),
        tests_per_toolchain=expected_total, rm_tests_per_toolchain=expected_rm, artifacts=len(artifacts), forced_process_tests_per_toolchain=2,
        actual_process_children_total=12, mutation_controls=len(controls), source_drift=False,
        WP04_complete=False, owned_RM_runtime_synthetic_TC_verified=runtime_component, formal_outbound_RM_calls_synthetic_TC_verified=runtime_component,
        callback_batches_synthetic_TC_verified=runtime_component, real_Java_RM_gate_complete=False, SQL_Undo_complete=False,
        full_V1_complete=False, release_authorized=False, external_tc_risk_accepted=False, shared_lock_issue='deferred_by_user'))
    members = [dict(path=str(path.relative_to(out)).replace('\\', '/'), sha256=sha(path.read_bytes()), bytes=path.stat().st_size)
               for path in sorted(out.rglob('*')) if path.is_file()]
    save('catalog.json', members)
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha((out/'catalog.json').read_bytes()), members=len(members)), ensure_ascii=False), flush=True)


if __name__ == '__main__':
    run()
