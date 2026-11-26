"""Bind the executed RM component and original Java gates; weaken observers only in isolated D: copies."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/formal-rm-java'
RUNTIME = ROOT / 'docs/phase-1/evidence/rm-runtime/run_20261114_003214_ee6afbb3'
JAVA = ROOT / 'docs/phase-1/evidence/rm-java/run_20261114_014916_69ccac54'
MSRV = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(condition, reason):
    if not condition:
        raise ValueError(reason)


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-1/evidence/rm-gate' / ('run_' + stamp)
    work = Path('D:/codex-builds') / ('seata_rm_gate_' + stamp)
    out.mkdir(parents=True)
    work.mkdir()
    print('RM_GATE_RUN ' + str(out), flush=True)
    env = os.environ | dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',
        CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production', TEMP='D:/Temp', TMP='D:/Temp',
        LOCALAPPDATA='D:/Temp/seata-localappdata', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1')

    def put(name, data):
        path = out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data.encode('utf-8') if isinstance(data, str) else data)

    def save(name, data):
        put(name, json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def command(args, name, cwd=ROOT, extra=None, failure=False):
        result = subprocess.run([str(v) for v in args], cwd=cwd, env=env | (extra or {}),
            creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=240)
        put(name, result.stdout)
        if failure:
            need(result.returncode == 1 and b'AssertionError: ValueError not raised' in result.stdout
                and b'FAILED (failures=1)' in result.stdout and b'ERROR:' not in result.stdout,
                'Observer control must import/execute and fail the target assertion: ' + name)
        else:
            need(result.returncode == 0, 'Command failed: ' + name)
        return result.stdout

    bindings = []
    current = {}
    for child, expected in [(RUNTIME, '66e1711307ef6a78c94ed1bde0acdd9e1ce1492bb035d6fbf133d9599490cad4'),
                            (JAVA, 'c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2')]:
        need(sha(child / 'catalog.json') == expected, 'Executed child catalog differs')
        catalog = json.loads((child / 'catalog.json').read_bytes())
        members = catalog['members'] if isinstance(catalog, dict) else catalog
        seen = set()
        for row in members:
            relative = Path(row['path'])
            need(not relative.is_absolute() and '..' not in relative.parts and row['path'] not in seen, 'Invalid child member')
            seen.add(row['path'])
            path = child / relative
            need(path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], 'Child member differs: ' + str(path))
        manifest = child / ('source-inputs-final.json' if child == JAVA else 'source-inputs.json')
        inputs = json.loads(manifest.read_bytes())
        verified = 0
        for row in inputs:
            path = Path(row['path'])
            need(sha(child / row['snapshot']) == row['sha256'], 'Child source snapshot differs')
            compiled = path.is_relative_to(ROOT / 'crates') and (path.suffix == '.rs' or path.name in ['Cargo.toml', 'Cargo.lock'])
            dependency = '/registry/' in path.as_posix()
            # 历史文档保持其历史内容；所有实际编译输入、依赖和本次Java驱动输入必须仍是同一份。
            if child == JAVA or compiled or dependency or path in [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock']:
                need(sha(path) == row['sha256'], 'Current executed input drift: ' + str(path))
                if path in current:
                    need(current[path] == row['sha256'], 'Children bound different compiled inputs')
                current[path] = row['sha256']
                verified += 1
        label = 'runtime' if child == RUNTIME else 'java'
        put(label + '/catalog.json', (child / 'catalog.json').read_bytes())
        put(label + '/result.json', (child / 'result.json').read_bytes())
        bindings.append(dict(path=str(child), catalog_sha256=expected, members_verified=len(members),
            snapshots_verified=len(inputs), current_inputs_verified=verified))

    runtime_result = json.loads((RUNTIME / 'result.json').read_bytes())
    need(runtime_result['tests_per_toolchain'] == 234 and runtime_result['rm_tests_per_toolchain'] == 45
        and runtime_result['mutation_controls'] == 11 and runtime_result['actual_process_children_total'] == 12,
        'Runtime execution coverage differs')
    artifacts = json.loads((RUNTIME / 'test-artifacts.json').read_bytes())
    need(len(artifacts) == 52, 'Missing actual component test artifacts')
    deps = json.loads((RUNTIME / 'dependencies.json').read_bytes())
    need(len(deps) == 22 and sum(d['active_on_windows_msvc'] for d in deps) == 18
        and sum(d['unpacked_files_verified'] for d in deps) == 1835, 'Dependency closure differs')
    jresult = json.loads((JAVA / 'result.json').read_bytes())
    cases = [mode + '-' + op for mode in ['basic', 'duplicate', 'reconnect'] for op in ['commit', 'rollback']]
    need(jresult['succeeded'] is True and jresult['all_six_Java_RM_cases_verified'] is True and jresult['selected'] == cases
        and not jresult['cleanup_errors'] and len(jresult['actual_TC_process_launches']) == 6
        and len(jresult['actual_proxy_process_launches']) == 12, 'Actual Java gate is incomplete')
    jar = Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle/server/target/seata-server.jar')
    need(sha(jar) == jresult['jar_sha256'], 'Actual Java JAR differs')
    command(['git', '-C', jar.parents[2], 'rev-parse', 'HEAD'], 'java-head.log')
    need((out / 'java-head.log').read_text('utf-8').strip() == jresult['java_pin'], 'Java pin differs')

    # 原始raw bytes在父门禁再次由当前校验器检查；不把已有verified.json当作行为证据。
    spec = importlib.util.spec_from_file_location('combined_gate_observer', HERE / 'validate.py')
    validator = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(validator)
    observed = []
    for name in cases:
        case = JAVA / name
        owner = json.loads((case / 'owned-processes.json').read_bytes())
        text = (case / 'client.log').read_text('utf-8')
        rows = [json.loads(line) for line in (case / 'events.jsonl').read_text('utf-8').splitlines()]
        tm_rows = [json.loads(line) for line in (case / 'tm-events.jsonl').read_text('utf-8').splitlines()]
        result = validator.validate(name, text, rows, owner['nonce'], (case / 'branches.log').read_bytes())
        result['independent_TM'] = validator.validate_tm(result, text, tm_rows, rows, (case / 'tc-stdout.log').read_bytes(), owner['nonce'])
        need(owner['observer_identity_given_to_client'] is False and len(json.loads((case / 'class-origins.json').read_bytes())) == 9,
            'Observer isolation or actual class origins differ')
        observed.append(result)
    save('independently-reverified-java-cases.json', observed)

    paths = set(current) | {Path(__file__).resolve()}
    frozen = []
    for path in sorted(paths):
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:24] + '.bin'
        put(snapshot, path.read_bytes())
        frozen.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot))
    save('source-inputs.json', frozen)
    green = command([sys.executable, '-m', 'unittest', 'discover', '-s', HERE, '-p', 'test_*.py', '-v'], 'python-tests.log')
    need(b'Ran 38 tests' in green and b'\nOK' in green, 'Expected 38 actually executed observer/proxy tests')
    source = (HERE / 'validate.py').read_text('utf-8')
    controls = [
        ('recovery-counter', 'and effects==1 and recovered==0', 'and effects==1', 'test_validate.TemporalTests.test_recovery_counter_cannot_claim_unobserved_recoveries'),
        ('request-id-history', 'and key not in used_ids', 'and key not in pending', 'test_validate.TemporalTests.test_completed_request_ids_are_not_reused_for_later_lock_queries'),
        ('report-order', "need(registration['index']<report['original']['index']<report['index']<locks[0]['original']['index']<locks[0]['index']<locks[1]['original']['index']<locks[1]['index']<first_callback,", 'need(True,', 'test_validate.TemporalTests.test_report_ack_cannot_be_moved_after_callback'),
        ('final-lock-order', "need(delivered and max(delivered)<locks[2]['original']['index']<locks[2]['index'],", 'need(True,', 'test_validate.TemporalTests.test_final_lock_result_cannot_be_moved_before_callback_completion'),
        ('closed-session', "need(row['index']<close_indices[row['connection']],", 'need(True,', 'test_validate.TemporalTests.test_final_business_cannot_return_to_closed_old_connection'),
        ('end-status-wire', "need(len(final_end)==1 and final_end[0]['result_code']==1 and end_lines[0][2]==str(final_end[0]['status']),", 'need(True,', 'test_tm_validate.TmBindingTests.test_end_status_cannot_be_changed_to_another_allowed_domain_value'),
        ('clock-resolution', "need(clock.get('name')=='perf_counter_ns' and clock.get('monotonic') is True and clock.get('adjustable') is False and type(resolution) in [int,float] and 0<resolution<=1e-6,", 'need(True,', 'test_tm_validate.TmBindingTests.test_low_resolution_capture_cannot_claim_strict_cross_process_order'),
        ('tc-original-ends', "need(received_ends==[(b'Commit' if operation=='commit' else b'Rollback',xid.encode()),(b'Rollback',contender.encode())] and len(re.findall(rb'receive msg\\[single\\]: Global(?:Commit|Rollback)Request',tc_log))==2,", 'need(True,', 'test_tm_validate.TmBindingTests.test_extra_begin_or_opposite_end_in_original_tc_log_is_rejected'),
        ('contender-terminal', "need(len(contender_end)==1 and contender_end[0]['result_code']==1 and contender_end[0]['status']==11,", 'need(True,', 'test_tm_validate.TmBindingTests.test_contender_rollback_cannot_be_mislabeled_committed'),
        ('journal-crc', "need(zlib.crc32(raw[:-4])==struct.unpack('>I',raw[-4:])[0],", 'need(True,', 'test_validate.VerifyTests.test_journal_crc_sequence_and_completed_identity_are_checked'),
    ]
    results = []
    for name, before, after, target in controls:
        need(source.count(before) == 1, 'Observer mutation anchor differs: ' + name)
        directory = work / name
        directory.mkdir()
        for path in HERE.glob('*.py'):
            shutil.copy2(path, directory / path.name)
        changed = source.replace(before, after, 1)
        compile(changed, str(directory / 'validate.py'), 'exec')
        (directory / 'validate.py').write_text(changed, encoding='utf-8')
        command([sys.executable, '-m', 'unittest', target, '-v'], 'controls/' + name + '.log', directory, failure=True)
        # Windows write_text可能转换换行；归档实际执行文件的原字节，不能拿逻辑字符串替代。
        put('controls/' + name + '.py', (directory / 'validate.py').read_bytes())
        results.append(dict(name=name, target=target, module_compiled=True, imports_executed=True,
            actual_target_assertion_failed=True, source_sha256=sha(directory / 'validate.py')))
        print('CONTROL_PASS ' + name, flush=True)
    save('controls.json', results)

    # 根组件234项双版本结果只在编译输入完全一致时继承；独立网络驱动的两项测试另补最低Rust版本。
    manifest = HERE / 'rust-client/Cargo.toml'
    helper_artifacts = []
    for label, cargo, extra in [('rust-1-99', 'cargo', {}), ('rust-1-85', MSRV / 'cargo.exe',
        dict(RUSTC=str(MSRV / 'rustc.exe'), RUSTDOC=str(MSRV / 'rustdoc.exe'), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        raw = command([cargo, 'test', '--offline', '--locked', '--manifest-path', manifest, '--message-format=json', '--', '--nocapture'], label + '/fixture-tests.jsonl', extra=extra)
        need(sum(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed;', raw)) == 2, 'Expected two driver tests on ' + label)
        entries = []
        for line in raw.splitlines():
            try:
                row = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            if row.get('reason') == 'compiler-artifact' and row.get('profile', {}).get('test') and row.get('executable') and row['target']['name'] == 'seata-rm-live-validation':
                path = Path(row['executable'])
                snapshot = 'artifacts/' + label + '-rm-live-tests.exe'
                put(snapshot, path.read_bytes())
                entries.append(dict(path=str(path), sha256=sha(path), snapshot=snapshot, tests_executed=2, rust=label))
        need(len(entries) == 1, 'Missing actual helper test executable')
        helper_artifacts.extend(entries)
    save('test-artifacts.json', helper_artifacts)

    check = command(['powershell', '-NoProfile', '-Command',
        "$ErrorActionPreference='Stop'; $left=Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $_.LocalPort -in @(38091,38092,38093) }; if($left){ throw 'RM test listener remains' }; $owned=Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*seata-formal-rm-20261114_014916_69ccac54*' -and $_.Name -in @('java.exe','python.exe') }; if($owned){ throw 'RM test service remains' }; 'OWNED_RM_SERVICES_CLEAN'"], 'cleanup.log')
    need(b'OWNED_RM_SERVICES_CLEAN' in check, 'Cleanup verification missing')
    for row in frozen:
        need(sha(Path(row['path'])) == row['sha256'], 'Input changed during combined gate')
    save('bindings.json', bindings)
    save('result.json', dict(status='verified_WP04_initial_RM_profile_pending_readonly_review',
        WP04_initial_profile_verified=True, formal_workspace_tests_per_toolchain_inherited=234,
        formal_RM_tests_per_toolchain_inherited=45, inherited_rust_versions=['1.99', '1.85'],
        inherited_component_artifacts=52, inherited_library_assertion_controls=11,
        inherited_forced_restart_children=12, driver_tests_per_toolchain=2, driver_test_artifacts=2,
        Python_tests=38, observer_assertion_controls=len(results), actual_Java_cases=6,
        actual_TC_launches=6, actual_proxy_launches=12, critical_class_origins=54,
        source_inputs=len(frozen), registry_archives=22, active_dependency_sources=1835,
        original_business_replay=False, current_inputs_verified=True, services_clean=True,
        SQL_Undo_complete=False, production_auth_complete=False, WP00_R_complete=False,
        original_go_default_compatibility_complete=False, full_V1_complete=False,
        release_authorized=False, external_tc_risk_accepted=False, shared_lock_issue='deferred_by_user'))
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path), bytes=path.stat().st_size)
        for path in sorted(out.rglob('*')) if path.is_file()]
    save('catalog.json', dict(members=members))
    print(json.dumps(dict(run=str(out), members=len(members), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
