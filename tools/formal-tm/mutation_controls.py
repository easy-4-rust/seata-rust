"""TM隔离行为负控；不修改正式源，以实际目标断言失败识别回归。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root,out,env):
    candidate=Path('D:/codex-builds')/('seata-tm-controls-'+uuid.uuid4().hex[:12]);candidate.mkdir()
    for leaf in ['Cargo.toml','Cargo.lock']:shutil.copyfile(root/leaf,candidate/leaf)
    shutil.copytree(root/'crates',candidate/'crates')
    base=candidate/'crates/seata_tm/src'
    leaves=['exception/tm_error.rs','api/global_transaction.rs','api/transactional_template.rs','api/transaction_manager.rs','model/transaction_config.rs','model/recovery_token.rs']
    baseline={leaf:(base/leaf).read_text(encoding='utf-8') for leaf in leaves}
    gt=baseline[leaves[1]];start=gt.index('        // 每次查询');end=gt.index('        self.last_status = outcome.status();',start)
    error=baseline[leaves[0]];start_error=error.index('        } else if may_have_effect {');end_error=error.index('        } else {',start_error)
    old_unknown=error[start_error:end_error]
    finished_anchor='''            self.state = if matches!(
                self.last_status,
                GlobalStatus::Committed'''
    cases=[
        ('written_timeout_falsely_local',leaves[0],old_unknown,old_unknown.replace('Self::UnknownOutcome','Self::LocalFailureBeforeSend').replace('                recovery: None,\n',''),'manager_calls','lost_begin_response_keeps_unknown_without_returned_xid_or_second_begin'),
        ('participant_sends_global_second_phase',leaves[1],'if self.role == GlobalTransactionRole::Participant {','if false && self.role == GlobalTransactionRole::Participant {','manager_calls','participant_never_sends_global_begin_commit_or_rollback'),
        ('failed_status_retains_stale_retry_permission',leaves[1],gt[start:end],'        let outcome = self.manager.get_status(&self.xid).await?;\n','manager_calls','failed_status_retry_timeout_revokes_prior_same_operation_retry_permission'),
        ('joined_scope_keeps_launcher_role',leaves[2],'Some(context.into_participant())','Some(context)','scoped_execution','required_nested_scope_participates_without_rewriting_outer_launcher_or_deadline'),
        ('finished_erases_original_commit_uncertainty',leaves[1],finished_anchor,finished_anchor.replace('GlobalStatus::Committed','GlobalStatus::Finished | GlobalStatus::Committed'),'manager_calls','lost_commit_requires_same_xid_status_before_explicit_retry_and_finished_stays_unknown'),
        ('failed_begin_discards_reported_optional_fields',leaves[3],'reported_begin: Some(Box::new(value)),','reported_begin: None,','manager_calls','failed_begin_preserves_reported_optional_fields_without_binding_a_launcher'),
        ('builder_debug_exposes_business_name',leaves[4],'.field("name_bytes", &self.name.len())\n            .field("timeout", &self.timeout)','.field("name", &self.name)\n            .field("timeout", &self.timeout)','context_config','config_builder_keeps_tc_timeout_exact_and_defaults_required'),
        ('recovered_end_skips_required_status_query',leaves[1],'transaction.state = EndState::Uncertain {\n            operation,\n            reconciled: false,\n        };','transaction.state = EndState::Open;','scoped_execution','unknown_scoped_end_can_recover_original_intent_without_starting_a_new_transaction'),
        ('recovery_ignores_issuing_manager',leaves[5],'Arc::ptr_eq(&self.authority, authority)','true','scoped_execution','unknown_scoped_end_can_recover_original_intent_without_starting_a_new_transaction'),
        ('valid_business_deadline_is_inverted',leaves[2],'if deadline <= Instant::now() {','if deadline > Instant::now() {','scoped_execution','required_nested_scope_participates_without_rewriting_outer_launcher_or_deadline'),
    ]
    results=[]
    try:
        for name,leaf,old,new,target,test in cases:
            for key,source in baseline.items():(base/key).write_text(source,encoding='utf-8',newline='\n')
            if baseline[leaf].count(old)!=1:raise ValueError('Nonunique mutation anchor:'+name)
            changed=baseline[leaf].replace(old,new);(base/leaf).write_text(changed,encoding='utf-8',newline='\n')
            result=subprocess.run(['cargo','test','--offline','--locked','-p','seata-tm','--test',target,test,'--','--exact'],cwd=candidate,
                env=env|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-tm-controls'),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=90)
            (out/(name+'.log')).write_bytes(result.stdout)
            if result.returncode!=101 or b'1 failed' not in result.stdout or (test+' ... FAILED').encode() not in result.stdout:raise ValueError('No actual target assertion failure:'+name)
            results.append(dict(name=name,candidate=str(candidate),source=leaf,source_sha256=hashlib.sha256(changed.encode()).hexdigest(),patch=dict(old=old,new=new),
                target=target,test=test,exit_code=result.returncode,actual_runtime_assertion_failed=True,original_product_defect=False))
    finally:
        for leaf,source in baseline.items():(base/leaf).write_text(source,encoding='utf-8',newline='\n')
    (out/'mutation-controls.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return results
