"""Execute faulty production-source variants in an owned isolated workspace; never edit live sources."""
import datetime
import os
from pathlib import Path
import shutil
import subprocess
import uuid
from inventory_rebuilt import ROOT, need, save, sha


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-4/evidence/tcc-core'/('mutations_'+stamp)
    out.mkdir(parents=True)
    work = Path('D:/codex-builds')/('seata-tcc-mutations-'+stamp)
    work.mkdir()
    shutil.copytree(ROOT/'crates/seata_tcc', work/'crates/seata_tcc')
    manifest = (ROOT/'Cargo.toml').read_bytes().decode()
    manifest = manifest[:manifest.index('members = ')]+'members = ["crates/seata_tcc"]\n'+manifest[manifest.index('exclude = '):]
    (work/'Cargo.toml').write_bytes(manifest.encode())
    shutil.copyfile(ROOT/'Cargo.lock', work/'Cargo.lock')
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0')
    inputs = [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in sorted((ROOT/'crates/seata_tcc').rglob('*')) if p.is_file()]
    save(out/'source-inputs.json', inputs)
    base = work/'crates/seata_tcc/src'
    context = base/'api/business_action_context.rs'
    fence = base/'fence/fence_handler.rs'
    mutations = [
        ('byte_budget', context, 'MAX_CONTEXT_BYTES: usize = 1_048_576', 'MAX_CONTEXT_BYTES: usize = 2_097_152', 'context_and_fence', 'invalid_context_and_budgets_fail_before_context_can_be_used'),
        ('depth_budget', context, 'MAX_JSON_DEPTH: usize = 64', 'MAX_JSON_DEPTH: usize = 128', 'context_and_fence', 'invalid_context_and_budgets_fail_before_context_can_be_used'),
        ('floating_point', context, 'serde_json::from_str(raw.get())', 'serde_json::from_str::<serde_json::Value>(raw.get()).and_then(serde_json::from_value)', 'context_and_fence', 'original_context_values_and_identity_are_preserved_without_floating_point'),
        ('string_flag', context, 'value.get() == "true"', 'value.get() == "\\\"true\\\""', 'context_and_fence', 'original_context_values_and_identity_are_preserved_without_floating_point'),
        ('late_try', fence, '(FenceOperation::Try, Some(_)) => FenceDecision::Refuse', '(FenceOperation::Try, Some(_)) => FenceDecision::InsertAndInvoke(Tried)', 'context_and_fence', 'complete_fence_matrix_never_replays_try_or_changes_terminal_direction'),
        ('terminal_direction', fence, '(FenceOperation::Confirm, Some(Rollbacked | Suspended)) => FenceDecision::Refuse', '(FenceOperation::Confirm, Some(Rollbacked | Suspended)) => FenceDecision::AlreadyApplied', 'context_and_fence', 'complete_fence_matrix_never_replays_try_or_changes_terminal_direction'),
        ('unicode_blank', context, "&& !matches!(value, '\\u{0085}' | '\\u{00a0}' | '\\u{2007}' | '\\u{202f}')", '&& true', 'java_context', 'every_bmp_scalar_has_original_java_blank_classification'),
    ]
    records = []
    for version in ['1.99', '1.85']:
        # 源码反例使用独占目标目录，避免同包名的相对路径指纹污染正式工作区产物。
        env['CARGO_TARGET_DIR'] = str(work/('target-'+version))
        baseline = subprocess.run(['cargo', '+'+version+'.0', 'test', '-p', 'seata-tcc', '--offline'], cwd=work, env=env, capture_output=True, timeout=120, creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(version+'-baseline.stdout')).write_bytes(baseline.stdout)
        (out/(version+'-baseline.stderr')).write_bytes(baseline.stderr)
        need(baseline.returncode == 0, 'Isolated TCC baseline failed')
        for name, path, needle, replacement, target, test in mutations:
            original = path.read_bytes()
            text = original.decode()
            need(text.count(needle) == 1, 'Mutation target changed: '+name)
            try:
                changed = text.replace(needle, replacement)
                path.write_bytes(changed.encode())
                prefix = version+'-'+name
                (out/(prefix+'.rs')).write_bytes(path.read_bytes())
                argv = ['cargo', '+'+version+'.0', 'test', '-p', 'seata-tcc', '--offline', '--locked', '--test', target, test, '--', '--exact']
                result = subprocess.run(argv, cwd=work, env=env, capture_output=True, timeout=120, creationflags=subprocess.CREATE_NO_WINDOW)
                (out/(prefix+'.stdout')).write_bytes(result.stdout)
                (out/(prefix+'.stderr')).write_bytes(result.stderr)
                need(result.returncode == 101 and b'test result: FAILED. 0 passed; 1 failed;' in result.stdout and test.encode() in result.stdout, 'Mutation did not fail its behavioral test: '+prefix)
                records.append(dict(Rust=version, mutation=name, argv=argv, exit_code=result.returncode, modified_source_sha256=sha(path), behavioral_test=test))
            finally:
                path.write_bytes(original)
    for row in inputs:
        need(sha(ROOT/row['path']) == row['sha256'], 'Live TCC source changed during isolated mutation tests')
    save(out/'records.json', records)
    save(out/'result.json', dict(status='TCC_FAULTY_PRODUCTION_VARIANTS_REJECTED', variants_per_version=7, actual_behavioral_failures=14, live_source_mutated=False, TCC_gate_complete=False, whole_plan_complete=False))
    shutil.copyfile(Path(__file__), out/Path(__file__).name)
    save(out/'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(str(out))


if __name__ == '__main__':
    run()
