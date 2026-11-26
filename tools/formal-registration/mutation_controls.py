"""同一独立D副本逐项破坏登记门禁，实际断言失败才计负控通过。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root,out,env):
    candidate=Path('D:/codex-builds')/('seata-registration-controls-'+uuid.uuid4().hex[:12]);candidate.mkdir()
    for leaf in ['Cargo.toml','Cargo.lock']:shutil.copyfile(root/leaf,candidate/leaf)
    for package in ['seata_protocol','seata_transport']:shutil.copytree(root/'crates'/package,candidate/'crates'/package)
    paths={name:candidate/'crates/seata_transport/src/netty'/name for name in ['session_registration.rs','pool_core.rs']}
    baseline={name:path.read_text(encoding='utf-8') for name,path in paths.items()}
    core=baseline['pool_core.rs']
    first=core.index('        if let Some(registration) = &self.registration {',core.index('pub(crate) fn snapshot'))
    last=core.index('        snapshot\n',first)
    snapshot_guard=core[first:last]
    first=core.index('            let selectable = match revision {')
    last=core.index('            if !selectable {',first)
    selection_guard=core[first:last]
    cases=[
        ('identified_false_is_accepted','session_registration.rs','RegistrationStatus::Rejected','RegistrationStatus::Accepted',
         ['--test','registered_pool'],'explicit_identity_denial_is_rejected_without_resource_or_business_attempt'),
        ('initializing_handle_is_selectable','pool_core.rs',selection_guard,'            let selectable=true;\n',
         ['--test','registered_pool'],'withheld_tm_identity_prevents_business_write_until_identified'),
        ('stale_revision_retains_ready_snapshot','pool_core.rs',snapshot_guard,'',
         ['--test','registered_pool'],'adding_resource_invalidates_old_revision_until_live_session_is_registered_again'),
        ('required_resources_silently_truncated','session_registration.rs','.resources()\n                .iter()','.resources()\n                .iter().take(1)',
         ['--test','registered_pool'],'replacement_session_repeats_identity_and_all_cached_resources_before_status'),
        ('each_resource_restarts_initialization_deadline','session_registration.rs','.attempt(handle, Some(entry.resource.resource_id()), deadline)',
         '.attempt(handle, Some(entry.resource.resource_id()), Instant::now()+self.timeout)',
         ['--lib'],'netty::session_registration::tests::initialization_steps_share_one_deadline_and_leave_later_resources_unattempted'),
        ('rm_adds_empty_resource_handshake','session_registration.rs','            for (index, entry) in report.resources.iter_mut().enumerate() {',
         '            report.identity=self.attempt(handle,None,deadline).await;\n            if report.identity.status!=RegistrationStatus::Accepted {return Ok(report);}\n            for (index, entry) in report.resources.iter_mut().enumerate() {',
         ['--test','registered_pool'],'rm_first_registration_includes_resource_for_identity_and_resource_auth_policy'),
    ]
    results=[]
    try:
        for name,leaf,old,new,target,test in cases:
            for key,path in paths.items():path.write_text(baseline[key],encoding='utf-8',newline='\n')
            if baseline[leaf].count(old)!=1:raise ValueError('Nonunique mutation anchor: '+name)
            modified=baseline[leaf].replace(old,new);paths[leaf].write_text(modified,encoding='utf-8',newline='\n')
            result=subprocess.run(['cargo','test','--offline','--locked','-p','seata-transport',*target,test,'--','--exact'],cwd=candidate,
                env=env|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-registration-controls'),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=60)
            (out/(name+'.log')).write_bytes(result.stdout)
            if result.returncode!=101 or b'1 failed' not in result.stdout or (test+' ... FAILED').encode() not in result.stdout:raise ValueError('Mutation did not fail an actual target assertion: '+name)
            results.append(dict(name=name,candidate=str(candidate),source=leaf,source_sha256=hashlib.sha256(modified.encode()).hexdigest(),patch=dict(old=old,new=new),test=test,
                                exit_code=result.returncode,actual_runtime_assertion_failed=True,original_product_defect=False))
    finally:
        for leaf,path in paths.items():path.write_text(baseline[leaf],encoding='utf-8',newline='\n')
    (out/'mutation-controls.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return results
