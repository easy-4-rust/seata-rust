"""D盘独立Python副本逐项削弱验收器；实际反例断言失败才计入门禁。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root,out,env):
    candidate=Path('D:/codex-builds')/('seata-tm-observer-controls-'+uuid.uuid4().hex[:12]);candidate.mkdir()
    for relative in ['tools/formal-tm-loss/validate.py','tools/formal-tm-loss/test_validate.py','tools/formal-transport-loss/live.py','tools/formal-transport-loss/loss_proxy.py','tools/formal-registration/live.py']:
        path=candidate/relative;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/relative,path)
    source=candidate/'tools/formal-tm-loss/validate.py';baseline=source.read_text(encoding='utf-8')
    cases=[
        ('duplicate_original_response_is_ignored',[("require(key not in completed,'An original request has more than one actual response')","pass")],'test_original_response_is_unique_and_not_also_forwarded'),
        ('old_inflight_query_authorizes_recovery',[("require(drop_index<query_index<index<end_limit and query['kind']==15 and query['xid']==target,'Recovery status request was already in flight before the original response loss')","pass")],'test_inflight_status_request_cannot_be_used_as_post_loss_reconciliation'),
        ('client_status_and_unknown_claims_are_ignored',[("require(actual==expected,'Client uncertainty/identity/status/business claim differs from actual raw responses')","pass")],'test_finished_cannot_be_rewritten_as_confirmed_commit_in_client_claim'),
        ('same_xid_retry_changes_operation',[("require(len(original_ends) in [1,2] and all(row['kind']==operation for _,row in original_ends),'Recovery switched operation or replayed more than one explicit duplicate')","pass"),("row['kind']==operation+1 and row['matched_request_index']==original_ends[1][0]","row['matched_request_index']==original_ends[1][0]")],'test_retry_cannot_switch_the_original_operation'),
        ('callback_operation_nonce_is_unchecked',[(" and cb['data']==identity['data']",'')],'test_callback_nonce_and_original_connection_cannot_be_substituted'),
        ('callback_moves_to_unregistered_connection',[("require(row['connection'] in [identities['TM'][0],identities['RM'][0]],'Frame migrated to an unregistered connection')","pass"),(" and callback['connection']==roles['RM'][0]",'')],'test_callback_nonce_and_original_connection_cannot_be_substituted'),
        ('lock_query_uses_different_resource_or_key',[("(lock_identity['resource'],lock_identity['lock'],lock_identity['branch_type'])==(identity['resource'],identity['lock'],0)",'True')],'test_lock_occupation_and_release_use_original_resource_and_key'),
        ('native_identity_rejection_is_accepted',[("require(response[2:3]==b'\\x01','Native identity rejected')","pass")],'test_rollback_keeps_business_error_and_native_identity_must_be_accepted'),
        ('finished_response_is_accepted_as_original_success',[("require(drop['global_status']==terminal,'Withheld original end response was not actual success')","pass")],'test_only_successful_original_end_response_can_be_withheld'),
    ]
    results=[]
    try:
        for name,patches,test in cases:
            changed=baseline
            for old,new in patches:
                if changed.count(old)!=1:raise ValueError('Nonunique observer mutation anchor:'+name)
                changed=changed.replace(old,new)
            source.write_text(changed,encoding='utf-8',newline='\n')
            result=subprocess.run(['python','-m','unittest','test_validate.ValidatorControls.'+test,'-v'],cwd=source.parent,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
                creationflags=subprocess.CREATE_NO_WINDOW,timeout=30)
            (out/(name+'.log')).write_bytes(result.stdout)
            if result.returncode!=1 or b'AssertionError: ValueError not raised' not in result.stdout or b'FAILED (failures=' not in result.stdout or b'ERROR:' in result.stdout:raise ValueError('Observer mutation did not fail an actual rejection assertion:'+name)
            results.append(dict(name=name,candidate=str(candidate),source_sha256=hashlib.sha256(changed.encode()).hexdigest(),patches=[dict(old=old,new=new) for old,new in patches],test=test,
                exit_code=1,actual_runtime_assertion_failed=True,original_product_defect=False))
    finally:source.write_text(baseline,encoding='utf-8',newline='\n')
    (out/'observer-controls.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return results
