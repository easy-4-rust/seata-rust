"""Bounded first-phase preparation qualification; public business COMMIT and full gates remain open."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
import uuid
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
PREVIOUS=ROOT/'docs/phase-2/evidence/mysql-callback-ownership-gate/run_20261117_145639_40c3447e'
BASE=ROOT/'docs/phase-2/evidence/lock-key-gate/run_20261116_011215_dc9906b8'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(value,reason):
    if not value:raise ValueError(reason)
def module(name,p):
    spec=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def catalog(p,digest):
    need(sha(p/'catalog.json')==digest,'Catalog authority differs')
    members=read(p/'catalog.json')['members'];need(len({r['path'] for r in members})==len(members),'Duplicate catalog member')
    for r in members:
        target=(p/r['path']).resolve();need(not Path(r['path']).is_absolute() and target.is_relative_to(p.resolve()) and sha(target)==r['sha256'],'Catalog member differs: '+r['path'])
    return {r['path']:r['sha256'] for r in members}
def role(result,rust,name):
    targets={'native':('live_first_phase_preparation',11),'wire':('live_first_phase_preparation',1),'java':('live_first_phase_java',1),'regression':('live_callback_ownership',18)}
    is_fault=name.startswith('enrollment_');mode=name[len('enrollment_'):] if is_fault else None
    if is_fault:targets[name]=('live_first_phase_enrollment_loss',1)
    target,count=targets[name];selected='preparation_uses_real_images_and_positive_receipt_but_keeps_business_and_undo_uncommitted' if name=='wire' else None
    need(result['status']=='SELECTED_FIRST_PHASE_PREPARATION_OBSERVATIONS_VERIFIED' and result['verified'] is True and result['Rust']==rust and result['selected_DB_target']==target and result['selected']==selected and result['wire_observation']==(name=='wire') and result.get('enrollment_fault')==mode,'Role/profile/target differs')
    need(len(result['actual_test_summary'])==1 and re.match(r'test result: ok\. '+str(count)+r' passed; 0 failed; 0 ignored;',result['actual_test_summary'][0]),'Actual selected count differs')
    for flag in ['public_COMMIT_available','TC_registration_gate_passed','SQL_Undo_commit_atomicity_verified','WP05_complete']:need(result[flag] is False,'Overclaimed scope: '+flag)
    need(bool(result['Java_peer'])==(name=='java'),'Original Java role confused with controlled peer')
def run(manifest):
    manifest=manifest.resolve();values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both compiler profiles required')
    catalog(PREVIOUS,'07435a2698a06f42ed3e9e28c70ca007dbdc915476f9b138ec1114b24bed5db6');catalog(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    old_inputs=read(PREVIOUS/'source-inputs.json');old_manager=next(r for r in old_inputs if Path(r['path'])==ROOT/'crates/seata_db_mysql/src/data_source_manager.rs');api_red=Path('D:/codex-builds/seata-first-phase-api-red-20261117.log');need(b'prepare_transaction' not in (PREVIOUS/old_manager['snapshot']).read_bytes() and api_red.read_bytes().count(b'error[E0599]')==2,'Missing-API compiler RED not bound to pre-implementation manager')
    old_lock=next(r for r in old_inputs if Path(r['path'])==ROOT/'Cargo.lock')
    old=tomllib.loads((PREVIOUS/old_lock['snapshot']).read_text('utf-8'))['package'];new=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package']
    need([r for r in old if r.get('source')]==[r for r in new if r.get('source')] and sum(bool(r.get('source')) for r in new)==185,'Registry candidates changed')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/mysql-first-phase-preparation-gate'/('run_'+stamp);out.mkdir(parents=True);print('FIRST_PHASE_PREPARATION_GATE_RUN '+str(out),flush=True)
    cache={};paths={Path(__file__),manifest,ROOT/'docs/phase-2/mysql-first-phase-preparation-contract.md',ROOT/'docs/phase-2/mysql-first-phase-preparation-file-map.csv',ROOT/'Cargo.toml',ROOT/'Cargo.lock',api_red};runtimes=[];packets={};audits={};negative=[]
    # 仅复用已封存且当前内容摘要相同的存储字节；旧检查结果不能替代本次执行。
    storage=PREVIOUS
    stored=catalog(storage,'07435a2698a06f42ed3e9e28c70ca007dbdc915476f9b138ec1114b24bed5db6');cache.update({digest:storage/leaf for leaf,digest in stored.items()})
    wire=module('callback_independent_wire',ROOT/'tools/formal-first-phase-preparation/validate.py');java=module('callback_independent_java',ROOT/'tools/formal-first-phase-preparation/java_runtime.py')
    for rust,profile in values['profiles'].items():
        need(set(profile)=={'native','wire','java','regression','enrollment_block_commit','enrollment_drop_ok','enrollment_delay_ok'},'Missing profile role')
        for name,item in profile.items():
            p=ROOT/item['path'];need(p.resolve().is_relative_to((ROOT/'docs/phase-2/evidence').resolve()),'Packet path escaped')
            members=catalog(p,item['catalog_sha256']);result=read(p/'result.json');role(result,rust,name)
            required={'source-inputs.json','result.json','cleanup.json','owned-runtime.json','rust-version.txt','build.jsonl','live-tests.log','artifacts/live_rm_recovery.exe'}
            if name=='wire':required.update({'mysql-wire.json','wire-validation.json','wire-negative-controls.json','temporary-warning-probe.tsv','temporary-warning-probe-command.json','observations/first-phase-positive.json'})
            if name.startswith('enrollment_'):required.update({'mysql-wire.json','enrollment-wire-validation.json','enrollment-wire-negative-controls.json','observations/prepare-enrollment-fault.json'})
            if name=='java':required.update({'java-tc/RM-events.jsonl','java-tc/TM-events.jsonl','java-tc/validation.json','java-tc/negative-controls.json','java-tc/class-load.log','java-tc/auth-binding.json','java-tc/CallbackOwnershipAuthHandler.class','observations/java-first-phase-preparation.json'})
            need(required.issubset(members),'Consumed required packet evidence missing from catalog')

            cache.update({digest:p/leaf for leaf,digest in members.items()})
            for r in read(p/'source-inputs.json'):
                need(members.get(r['snapshot'])==r['sha256']==sha(Path(r['path'])),'Current live source differs: '+r['path']);paths.add(Path(r['path']))
            need((p/'rust-version.txt').read_text('utf-8').startswith('rustc '+rust+'.'),'Actual live compiler differs')
            cleanup=read(p/'cleanup.json');need(cleanup['owned_container_removed'] is True and cleanup['observer_error'] is None,'Live cleanup incomplete');runtimes.append(read(p/'owned-runtime.json'))
            packets[rust+'/'+name]=dict(path=str(p),catalog_sha256=item['catalog_sha256'])
            if name=='native':
                positive=read(p/'observations/first-phase-positive.json');need(positive['registered_requests']==1 and positive['branch_id']==9007199254740995 and positive['business_rows_after_rollback']==positive['undo_rows_after_rollback']==0 and positive['control_rows_after_rollback']==1 and positive['business_COMMIT'] is False,'Positive preparation facts differ')
                waits=read(p/'observations/prepare-lock-waits.json');need(len(waits)==4 and {(v['blocked_table'],v['cancelled']) for v in waits}=={(t,c) for t in ['undo_log','seata_rs_at_branch_v1'] for c in [True,False]},'Actual lock-wait roles differ')
                for v in waits:
                    need(v['actual_waits']>0 and v['business_rows']==v['undo_rows']==0 and v['recovery']==([0,0,0] if v['blocked_table']=='seata_rs_at_branch_v1' else [1,0,0]),'Cancelled/deadline database facts differ')
                    if not v['cancelled']:need(v['receipt_when_deadline']==42 and 1400<=v['elapsed_ms']<2000,'Original known identity/absolute deadline evidence differs')
            if name=='regression':
                optional=read(p/'observations/callback-early-positive.json');need(optional['late_enroll']=='CallbackObserved' and optional['known_None_positive_origin']==1,'Callback origin regression differs')
                children=read(p/'observations/callback-actual-old-reader.json');need(children['old_exit']!=0 and children['new_exit']==0 and children['old_pending_preserved'] is True,'Actual old/current process regression missing')
            if name=='wire':
                w=read(p/'mysql-wire.json');o=read(p/'observations/first-phase-positive.json');probe=(p/'temporary-warning-probe.tsv').read_text('utf-8');verified=wire.validate(w,o,probe);need(verified==read(p/'wire-validation.json'),'Independent MySQL original transcript differs');controls=wire.negative_controls(w,o,probe);need(controls==read(p/'wire-negative-controls.json') and controls['count']==19,'SQL negative controls differ');negative.append(dict(rust=rust,mode='original-MySQL-prepare',**controls))
            if name.startswith('enrollment_'):
                w=read(p/'mysql-wire.json');o=read(p/'observations/prepare-enrollment-fault.json');verified=wire.validate_enrollment_fault(w,o);need(verified==read(p/'enrollment-wire-validation.json'),'Enrollment original wire differs');controls=wire.enrollment_negative_controls(w,o);need(controls==read(p/'enrollment-wire-negative-controls.json') and controls['count']==(18 if name=='enrollment_delay_ok' else 17),'Enrollment wire negative controls differ');negative.append(dict(rust=rust,mode=name,**controls))
            if name=='java':
                observation=read(p/'observations/java-first-phase-preparation.json');verified=java.validate(p/'java-tc',observation);need(verified==read(p/'java-tc/validation.json')==result['Java_peer'],'Independent original Java preparation evidence differs');controls=java.negative_controls(p/'java-tc',observation);need(controls==read(p/'java-tc/negative-controls.json') and controls['count']==15,'Original Java negative controls differ');negative.append(dict(rust=rust,mode='original-Java-TC',**controls));need(not any(read(p/'java-tc/cleanup.json')['listeners'].values()),'Owned Java port remains');audits[rust]=verified
    need(len({r['container_id'] for r in runtimes})==14,'Live DB roles reused')
    red=ROOT/values['red']['path'];catalog(red,values['red']['catalog_sha256']);need(read(red/'result.json')['status']=='EXPECTED_TARGET_BEHAVIOR_RED' and 'actual error=Enrollment' in (red/'live-tests.log').read_text('utf-8'),'Target behavior RED missing')
    for directory in [ROOT/'crates',ROOT/'tools/formal-first-phase-preparation',HERE]:paths.update(p for p in directory.rglob('*') if p.is_file())
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);data=data.encode('utf-8') if isinstance(data,str) else data;digest=hashlib.sha256(data).hexdigest();canonical=cache.get(digest)
        if canonical and not p.exists() and canonical.stat().st_nlink<900 and sha(canonical)==digest:os.link(canonical,p)
        else:p.write_bytes(data);cache[digest]=p
    def save(name,value):put(name,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
    save('source-inputs.json',frozen);save('explicit-packets.json',values);save('consumed-packets.json',packets);save('wire-negative-controls.json',negative);save('actual-Java-TC.json',audits)
    rejected=[]
    for rust in values['profiles']:
        result=read(ROOT/values['profiles'][rust]['native']['path']/'result.json')
        for name,wrong_rust,wrong_name in [('wrong_profile','1.85' if rust=='1.99' else '1.99','native'),('controlled_peer_as_java',rust,'java'),('native_as_regression',rust,'regression')]:
            try:role(result,wrong_rust,wrong_name)
            except ValueError:rejected.append(dict(rust=rust,case=name));continue
            raise ValueError('Role negative accepted')
    save('role-negative-controls.json',rejected)
    env=os.environ|dict(CARGO_INCREMENTAL='0',CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1',DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    def command(args,name,extra=None):
        value=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600);put(name,value.stdout);need(value.returncode==0,name+' failed');return value.stdout
    command(['cargo','fmt','--all','--','--check'],'fmt.log');ordinary={}
    for rust in ['1.99','1.85']:
        extra={};cargo='cargo'
        if rust=='1.85':
            tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';extra=dict(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv',PATH=str(tool)+os.pathsep+env.get('PATH',''),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc')
        version=command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/rust-version.txt',extra);need(version.startswith(('rustc '+rust+'.').encode()),'Compiler profile differs')
        version=command([cargo,'clippy','--version'],rust+'/clippy-version.txt',extra);need(version.startswith(('clippy 0.1.'+rust.split('.')[1]).encode()),'Clippy compiler differs')
        command([cargo,'clippy','--workspace','--all-targets','--all-features','--locked','--offline','--','-D','warnings'],rust+'/clippy.log',extra)
        command([cargo,'doc','--workspace','--all-features','--no-deps','--locked','--offline'],rust+'/doc.log',extra|dict(RUSTDOCFLAGS='-D warnings'))
        raw=command([cargo,'test','--workspace','--all-targets','--all-features','--locked','--offline','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra)
        docs=command([cargo,'test','--workspace','--doc','--locked','--offline'],rust+'/doc-tests.log',extra)
        doc_count=sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',docs)));need(doc_count==3 and b'branch_callback_receipt.rs' in docs and b'registered_branch.rs' in docs and b'mysql_prepared_transaction.rs' in docs,'Both opaque receipts and prepared no-COMMIT documentation cases required')
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(map(int,counts));need(passed==339,'Ordinary test count differs')
        artifacts=[]
        for line in raw.splitlines():
            try:r=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(r,dict):artifacts.append(r)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')];need(len(tested)==len(counts)==71,'Actual test harness count differs')
        compiled={r['package_id'].rsplit('#',1)[1] for r in artifacts if r.get('reason')=='compiler-artifact' and r['package_id'].startswith('registry+')};need(compiled=={'@'.join(parts) for parts in read(BASE/rust/'compiled-registry.json')} and len(compiled)==150,'Compiled registry closure differs')
        receipts=[]
        for r in tested:
            p=Path(r['executable']);digest=sha(p);leaf=rust+'/artifacts/'+digest[:24]+'.exe';put(leaf,p.read_bytes());receipts.append(dict(path=str(p),sha256=digest,snapshot=leaf))
        save(rust+'/test-artifacts.json',receipts);save(rust+'/compiled-registry.json',sorted(compiled))
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw);need(len(groups)==2,'Existing RM process recovery evidence missing')
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Restart evidence path escaped')
            for p in directory.rglob('*'):
                if p.is_file():put(rust+'/rm-restarts/'+phase.decode()+'/'+p.relative_to(directory).as_posix(),p.read_bytes())
        ordinary[rust]=dict(passed=passed,harness_artifacts=len(tested),compiled_registry_packages=len(compiled),doc_tests=doc_count,existing_RM_child_processes=6)
    absent=[]
    for runtime in runtimes:
        value=subprocess.run(['docker','inspect',runtime['container_id']],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30);need(value.returncode!=0 and b'no such object' in value.stdout.lower(),'Owned DB container remains');absent.append(dict(container_id=runtime['container_id'],verified_absent=True))
    save('owned-runtime-final-audit.json',absent)
    for r in frozen:need(sha(Path(r['path']))==r['sha256'],'Final source drift')
    save('result.json',dict(status='SELECTED_FIRST_PHASE_PREPARATION_VERIFIED_FULL_COMMIT_GATES_OPEN',ordinary=ordinary,source_inputs=len(frozen),fresh_DB_executions_per_profile=33,fresh_preparation_enrollment_COMMIT_faults_per_profile=3,native_preparation_tests_per_profile=10,concrete_preparation_observations_per_profile=21,fresh_callback_ownership_regressions_per_profile=18,fresh_legacy_managed_DB_regressions_per_profile=11,fresh_original_Java_TC_cases_per_profile=1,wire_negative_controls=sum(r['count'] for r in negative),role_negative_controls=len(rejected),qualified_owned_DB_containers=14,registry_archive_candidates=185,actual_compiled_registry_packages=150,original_Java_JUnit_executed=False,new_Java_auth_fixture_compiled=True,first_phase_preparation_verified=True,API_compiler_RED_verified=True,compiler_RED_not_runtime_RED=True,exact_deadline_diagnostic_RED_retained=True,preparation_COMMIT_available=False,business_and_Normal_Undo_same_original_transaction_verified=True,public_first_phase_business_COMMIT_available=False,first_phase_SQL_Undo_commit_atomicity_verified=False,all_old_first_phase_attempts_fenced=False,commit_direction_verified=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',source_objects_state='PARTIAL'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
