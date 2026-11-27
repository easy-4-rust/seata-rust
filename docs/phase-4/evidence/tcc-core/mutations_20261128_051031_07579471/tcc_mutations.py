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
    shutil.copytree(ROOT/'crates/seata_datasource', work/'crates/seata_datasource')
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
    equality = base/'api/action_context_value_equality.rs'
    util = base/'api/business_action_context_util.rs'
    action_util = base/'api/action_context_util.rs'
    mutations = [
        ('byte_budget', context, 'MAX_CONTEXT_BYTES: usize = 1_048_576', 'MAX_CONTEXT_BYTES: usize = 2_097_152', 'context_and_fence', 'invalid_context_and_budgets_fail_before_context_can_be_used'),
        ('depth_budget', context, 'MAX_JSON_DEPTH: usize = 64', 'MAX_JSON_DEPTH: usize = 128', 'context_and_fence', 'invalid_context_and_budgets_fail_before_context_can_be_used'),
        ('floating_point', context, 'serde_json::from_str(raw.get())', 'serde_json::from_str::<serde_json::Value>(raw.get()).and_then(serde_json::from_value)', 'context_and_fence', 'original_context_values_and_identity_are_preserved_without_floating_point'),
        ('string_flag', context, 'value.get() == "true"', 'value.get() == "\\\"true\\\""', 'context_and_fence', 'original_context_values_and_identity_are_preserved_without_floating_point'),
        ('late_try', fence, '(FenceOperation::Try, Some(_)) => FenceDecision::Refuse', '(FenceOperation::Try, Some(_)) => FenceDecision::InsertAndInvoke(Tried)', 'context_and_fence', 'complete_fence_matrix_never_replays_try_or_changes_terminal_direction'),
        ('terminal_direction', fence, '(FenceOperation::Confirm, Some(Rollbacked | Suspended)) => FenceDecision::Refuse', '(FenceOperation::Confirm, Some(Rollbacked | Suspended)) => FenceDecision::AlreadyApplied', 'context_and_fence', 'complete_fence_matrix_never_replays_try_or_changes_terminal_direction'),
        ('unicode_blank', context, "&& !matches!(value, '\\u{0085}' | '\\u{00a0}' | '\\u{2007}' | '\\u{202f}')", '&& true', 'java_context', 'every_bmp_scalar_has_original_java_blank_classification'),
    ]
    mutations += [
        ('tracking_off',context,'self.tracking_enabled = true;','self.tracking_enabled = false;','context_mutation','pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match'),
        ('null_put_not_dirty',context,'if self.tracking_enabled && changed {','if self.tracking_enabled && changed && value.get() != "null" {','context_mutation','pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match'),
        ('add_null_overwrites',context,'value.filter(|v| v.get() != "null")','value','context_mutation','pinned_java_value_change_and_type_comparisons_match_without_reencoding_values'),
        ('remove_null_not_dirty',context,'if self.tracking_enabled && previous.is_some() {','if self.tracking_enabled && previous.as_ref().is_some_and(|v|v.get() != "null") {','context_mutation','pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match'),
        ('empty_clear_dirty',context,'if !self.action_context.is_empty() {','if true {','context_mutation','pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match'),
        ('set_map_dirty',context,'self.action_context = values;','self.action_context = values; self.updated = Some(true);','context_mutation','pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match'),
        ('mutation_budget_ignored',context,'Self::validate_mapping(&candidate)?;','let _ = Self::validate_mapping(&candidate);','context_mutation','failed_mutations_preserve_map_tracking_and_original_numeric_bytes'),
        ('float_zero_collapsed',equality,'left.to_bits() == right.to_bits()','left == right','context_mutation','pinned_java_value_change_and_type_comparisons_match_without_reencoding_values'),
        ('object_values_ignored',equality,'if left == right {',"if left == right || left.starts_with('{') {",'context_mutation','pinned_java_value_change_and_type_comparisons_match_without_reencoding_values'),
        ('untracked_add_not_dirty',context,'if changed {','if changed && self.tracking_enabled {','context_mutation','explicit_add_marks_updated_before_tracking_but_direct_put_does_not'),
    ]
    mutations += [
        ('util_delay_ignored',util,'if context.get_delay_report() == Some(true) {','if false {','context_util','original_java_util_nineteen_state_and_report_observations_match'),
        ('util_same_value_reports',util,'if !changed {\n            return Ok(false);','if !changed {\n            return Self::report_context(context, transaction).await;','context_util','original_java_util_nineteen_state_and_report_observations_match'),
        ('util_batch_null_overwrites',util,'if value.get() == "null" {\n                continue;','if value.get() == "null" {\n                candidate.put_action_context(key,value)?; changed=true; continue;','context_util','original_java_util_nineteen_state_and_report_observations_match'),
        ('util_clear_before_ack',util,'let reported = transaction.report_context(context).await?;','context.set_updated(None); let reported = transaction.report_context(context).await?;','context_util','original_java_util_nineteen_state_and_report_observations_match'),
        ('util_composite_not_string',action_util,"if matches!(value.get().as_bytes().first(), Some(b'{' | b'[')) {",'if false {','context_util','original_java_util_nineteen_state_and_report_observations_match'),
        ('util_partial_batch_updates',util,'changed |= candidate.add_action_context(key, Some(&value))?;','let result = candidate.add_action_context(key, Some(&value)); *context = candidate.clone(); changed |= result?;','context_util','batch_budget_rejection_is_atomic_and_preserves_raw_composite_precision'),
        ('util_noop_clears_dirty',util,'if reported {','if true {','context_util','only_confirmed_report_clears_updated_and_borrowed_contexts_remain_isolated'),
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
    save(out/'result.json', dict(status='TCC_FAULTY_PRODUCTION_VARIANTS_REJECTED', variants_per_version=24, actual_behavioral_failures=48, live_source_mutated=False, TCC_gate_complete=False, whole_plan_complete=False))
    shutil.copyfile(Path(__file__), out/Path(__file__).name)
    save(out/'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(str(out))


if __name__ == '__main__':
    run()
