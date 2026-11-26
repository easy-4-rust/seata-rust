"""从同一基线逐项构造隔离回归负控，验证实际断言；不改正式源码。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root,out,env):
    candidate=Path('D:/codex-builds')/('seata-pool-controls-'+uuid.uuid4().hex[:12]);candidate.mkdir()
    for leaf in ['Cargo.toml','Cargo.lock']:shutil.copyfile(root/leaf,candidate/leaf)
    for leaf in ['seata_protocol','seata_transport']:shutil.copytree(root/'crates'/leaf,candidate/'crates'/leaf)
    paths={leaf:candidate/'crates/seata_transport/src/netty'/leaf for leaf in ['pool_handle.rs','reconnect_backoff.rs','connection_pool.rs','connection_handle.rs']}
    baseline={leaf:path.read_text(encoding='utf-8') for leaf,path in paths.items()}
    cases=[
        ('hidden_business_replay','pool_handle.rs','        handle.request_until(body, deadline).await',
         '        let result=handle.request_until(body.clone(),deadline).await;\n        if result.is_err() {self.acquire_until(deadline).await?.request_until(body,deadline).await} else {result}',
         ['--test','connection_pool'],'disconnect_replaces_generation_but_never_replays_lost_begin'),
        ('flapping_connection_resets_backoff','reconnect_backoff.rs','if lifetime >= self.policy.stable_after {','if !lifetime.is_zero() {',
         ['--lib'],'netty::reconnect_backoff::tests::backoff_caps_without_wrap_and_short_lived_connections_do_not_reset'),
        ('owner_drop_does_not_synchronously_close','connection_pool.rs',
         'impl Drop for ConnectionPool {\n    fn drop(&mut self) {\n        self.handle.core.stop(TransportError::PoolClosed);',
         'impl Drop for ConnectionPool {\n    fn drop(&mut self) {',
         ['--test','connection_pool'],'owner_drop_closes_active_requests_and_all_pool_handles'),
        ('acquisition_time_restarts_request_timeout','pool_handle.rs','        handle.request_until(body, deadline).await','        handle.request(body, timeout).await',
         ['--lib'],'netty::pool_handle::tests::acquisition_time_is_part_of_one_absolute_request_budget'),
        ('quota_error_leaves_generation_open','connection_handle.rs',
         '                if matches!(reason, TransportError::RequestIdRotationRequired { .. }) {\n                    self.core.terminate(reason.clone());\n                }','',
         ['--test','connection_pool'],'exhausted_request_id_quota_rotates_pool_generation_without_replaying_rejected_call'),
    ]
    results=[]
    try:
        for name,leaf,old,new,target,test in cases:
            for entry,path in paths.items():path.write_text(baseline[entry],encoding='utf-8',newline='\n')
            original=baseline[leaf]
            if original.count(old)!=1:raise ValueError('Nonunique mutation anchor: '+name)
            modified=original.replace(old,new);paths[leaf].write_text(modified,encoding='utf-8',newline='\n')
            command=['cargo','test','--offline','--locked','-p','seata-transport',*target,test,'--','--exact']
            result=subprocess.run(command,cwd=candidate,env=env|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-pool-controls'),
                stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=60)
            (out/(name+'.log')).write_bytes(result.stdout)
            if result.returncode!=101 or b'1 failed' not in result.stdout or (test+' ... FAILED').encode() not in result.stdout:raise ValueError('Behavior mutation not rejected: '+name)
            results.append(dict(name=name,candidate=str(candidate),source=leaf,source_sha256=hashlib.sha256(modified.encode()).hexdigest(),
                                patch=dict(old=old,new=new),test=test,exit_code=result.returncode,actual_runtime_assertion_failed=True,original_product_defect=False))
    finally:
        for leaf,path in paths.items():path.write_text(baseline[leaf],encoding='utf-8',newline='\n')
    (out/'mutation-controls.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return results
