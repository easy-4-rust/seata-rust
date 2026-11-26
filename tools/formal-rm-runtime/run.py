"""Execute the full owned RM runtime component profile; real Java RM gate remains separate."""
import importlib.util
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('rm_runtime_evidence_support',ROOT/'tools/formal-rm-foundation/run.py')
support=importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)

if __name__=='__main__':
    manager=(ROOT/'crates/seata_rm/src/manager/resource_manager.rs').read_text('utf-8')
    before='''    while let Some(result) = workers.join_next().await {
        if result.is_err_and(|error| !error.is_cancelled()) {
            failed = true;
        }
    }'''
    assert manager.count(before)==1
    callback=(ROOT/'crates/seata_rm/src/manager/callback_handler.rs').read_text('utf-8')
    deadline='''        if Instant::now() >= deadline {
            return Err(RmError::Unavailable {
                reason: "callback validation exceeded deadline",
            });
        }'''
    assert callback.count(deadline)==1
    controls=[
      ('failed-is-not-success','manager/resource_client.rs','if prefix.result_code == ResultCode::Failed {','if false {','native_calls','failed_lock_query_keeps_true_field'),
      ('issued-id-validation','manager/resource_client.rs','if matches!(&reply.body,Some(Message::BranchRegisterResponse(v)) if v.branch_id<=0) {','if false {','native_edge_cases','successful_registration_without_positive_issued_id'),
      ('sent-marker','exception/rm_call_error.rs','let may_have_effect = source.is_unknown_outcome();','let may_have_effect = false;','native_calls','wrong_response_family_is_protocol_uncertainty'),
      ('cleanup-panic','manager/resource_manager.rs',before,'    while workers.join_next().await.is_some() {}','native_edge_cases','shutdown_reports_resource_cleanup_panic'),
      ('batch-phase-conflict','manager/callback_handler.rs','if previous != &identity || *direction != phase {','if false {','native_callbacks','contradictory_directions_inside_one_batch'),
      ('validation-deadline','manager/callback_handler.rs',deadline,'','__lib__','validation_uses_the_original_absolute_deadline'),
    ]
    support.run(runtime_component=True,extra_controls=controls,source_inputs_extra=[Path(__file__).resolve(),ROOT/'docs/phase-0/registration-v1-wire-contract.md'])
