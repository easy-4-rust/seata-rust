"""只在D盘独立副本制造错误；实际目标断言失败才算验收负控通过。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root, out, env):
    candidate = Path('D:/codex-builds') / ('seata-loss-controls-' + uuid.uuid4().hex[:12])
    candidate.mkdir()
    for leaf in ['Cargo.toml', 'Cargo.lock']:
        shutil.copyfile(root / leaf, candidate / leaf)
    for package in ['seata_protocol', 'seata_transport']:
        shutil.copytree(root / 'crates' / package, candidate / 'crates' / package)
    base = candidate / 'crates/seata_transport'
    leaves = ['src/netty/write_receipt.rs', 'src/netty/connection_handle.rs',
              'src/netty/tcp_connection.rs', 'examples/support/simulated_branch.rs',
              'examples/support/uncertain_commit.rs']
    baseline = {leaf: (base / leaf).read_text(encoding='utf-8') for leaf in leaves}
    receipt = baseline[leaves[0]]
    start = receipt.index('            if stats.closed {', receipt.index('pub(crate) fn written'))
    end = receipt.index('            stats.responses_written', start)
    runtime_guard = '''        if tokio::runtime::Handle::try_current().is_err() {
            return Err(local(TransportError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            }));
        }
'''
    cases = [
        ('closed_generation_publishes_write_success', leaves[0], receipt[start:end], '',
         ['--lib'], 'netty::write_receipt::tests::closure_and_full_write_arbitrate_before_unpolled_caller_observes_result'),
        ('reply_queues_without_runtime_guard', leaves[1], runtime_guard, '',
         ['--test', 'inbound_response'], 'reply_without_tokio_context_is_typed_local_failure_before_queue_or_write'),
        ('reply_ignores_branch_identity', leaves[1],
         ' || !ResponseExpectation::reply_identity_matches(original, &body)', '',
         ['--test', 'inbound_response'], 'wrong_family_or_branch_identity_is_rejected_without_writing'),
        ('expired_or_cancelled_response_is_written', leaves[2],
         'if !response.receipt.begin() {', 'if false && !response.receipt.begin() {',
         ['--lib'], 'netty::tcp_connection::tests::response_queue_budget_and_prewrite_expiry_or_cancel_skip_all_reply_bytes'),
        ('response_prefix_is_published_as_complete_frame', leaves[2],
         '&response.bytes\n', '&response.bytes[..8]\n',
         ['--lib'], 'netty::tcp_connection::tests::response_partial_write_survives_deadline_or_cancellation_and_closure_never_reuses_stream'),
        ('duplicate_callback_repeats_simulated_effect', leaves[3],
         '        if let Some(cached) = &self.cached {\n            return Ok(cached.clone());\n        }\n', '',
         ['--example', 'transport_loss_live'], 'simulated_branch::tests::pending_identity_is_learned_only_from_exact_callback_and_duplicate_action_is_cached'),
        ('callback_learns_branch_with_wrong_operation_nonce', leaves[3],
         '            || data != Some(self.data.as_str())\n', '',
         ['--example', 'transport_loss_live'], 'simulated_branch::tests::mismatches_neither_learn_branch_nor_execute_simulated_action'),
        ('finished_falsely_confirms_historical_commit', leaves[4],
         'if status == GlobalStatus::Committed {',
         'if status == GlobalStatus::Committed || status == GlobalStatus::Finished {',
         ['--example', 'transport_loss_live'], 'uncertain_commit::tests::finished_never_creates_commit_confirmation_and_does_not_erase_prior_authority'),
    ]
    results = []
    try:
        for name, leaf, old, new, target, test in cases:
            for key, original in baseline.items():
                (base / key).write_text(original, encoding='utf-8', newline='\n')
            if baseline[leaf].count(old) != 1:
                raise ValueError('Nonunique mutation anchor: ' + name)
            modified = baseline[leaf].replace(old, new)
            (base / leaf).write_text(modified, encoding='utf-8', newline='\n')
            result = subprocess.run(['cargo', 'test', '--offline', '--locked', '-p', 'seata-transport',
                                     *target, test, '--', '--exact'], cwd=candidate,
                                    env=env | dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-loss-controls'),
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    creationflags=subprocess.CREATE_NO_WINDOW, timeout=90)
            (out / (name + '.log')).write_bytes(result.stdout)
            if result.returncode != 101 or b'1 failed' not in result.stdout or (test + ' ... FAILED').encode() not in result.stdout:
                raise ValueError('Mutation did not fail an actual target assertion: ' + name)
            results.append(dict(name=name, candidate=str(candidate), source=leaf,
                                source_sha256=hashlib.sha256(modified.encode()).hexdigest(),
                                patch=dict(old=old, new=new), test=test, exit_code=result.returncode,
                                actual_runtime_assertion_failed=True, original_product_defect=False))
    finally:
        for leaf, original in baseline.items():
            (base / leaf).write_text(original, encoding='utf-8', newline='\n')
    (out / 'mutation-controls.json').write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return results
