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
PREVIOUS=ROOT/'docs/phase-2/evidence/mysql-first-phase-preparation-gate/run_20261117_165622_ee38f20f'
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
    target,count={'native':('live_commit_preconditions',8),'preparation':('live_first_phase_preparation',11),'regression':('live_callback_ownership',18),'java':('live_commit_check_java',1)}[name]
    need(result['status']=='SELECTED_COMMIT_PRECONDITIONS_OBSERVATIONS_VERIFIED' and result['verified'] is True and result['Rust']==rust and result['selected_DB_target']==target and result['selected'] is None and result['wire_observation'] is False and result['enrollment_fault'] is None,'Role/profile/target differs')
    need(len(result['actual_test_summary'])==1 and re.match(r'test result: ok\. '+str(count)+r' passed; 0 failed; 0 ignored;',result['actual_test_summary'][0]),'Actual selected count differs')
    need(bool(result['Java_peer'])==(name=='java'),'Original Java peer role confused')
    for flag in ['public_COMMIT_available','TC_registration_gate_passed','SQL_Undo_commit_atomicity_verified','WP05_complete']:need(result[flag] is False,'Overclaimed scope')

def require_members(members,name):
    required={'source-inputs.json','result.json','cleanup.json','owned-runtime.json','rust-version.txt','build.jsonl','live-tests.log','artifacts/live_rm_recovery.exe'}
    if name=='native':
        required.update('observations/commit-check-'+leaf+'.json' for leaf in ['held','rejections','waits','drops','noop','callback-first','local-first','legacy'])
    if name=='regression':
        required.update({'observations/callback-early-positive.json','observations/callback-actual-old-reader.json'})
    if name=='java':
        required.update({'observations/java-first-phase-preparation.json','observations/java-branches.log','java-tc/RM-events.jsonl','java-tc/TM-events.jsonl','java-tc/auth-binding.json','java-tc/CallbackOwnershipAuthHandler.class','java-tc/class-load.log','java-tc/validation.json','java-tc/negative-controls.json','java-tc/cleanup.json','java-tc/owned-processes.json'})
    need(required.issubset(members),'Consumed role evidence missing from catalog')
    return required

def native(p):
    held=read(p/'observations/commit-check-held.json')
    need(held['branch_id']==42 and held['actual_recovery_row_wait'] is True and held['same_transaction_record_lock_tables']==3 and held['business_rows']==held['undo_rows']==0 and held['public_business_COMMIT'] is False,'Original physical transaction/lock facts differ')
    need(all(isinstance(held[k],int) and held[k]>0 for k in ['actual_engine_transaction_id','actual_thread_id','actual_physical_connection_id']),'Actual engine/connection identity missing')
    rejected=read(p/'observations/commit-check-rejections.json')
    need(len(rejected)==10 and {v['case'] for v in rejected}=={'commit','rollback','terminal','application','resource','profile','lock','callback','missing','schema'},'Rejection matrix differs')
    need(all(v['receipt']==42 and v['business']==v['undo']==0 for v in rejected),'Rejected transaction escaped')
    waits=read(p/'observations/commit-check-waits.json')
    need(len(waits)==2 and {v['cancel'] for v in waits}=={False,True} and all(v['actual_recovery_wait'] is True and v['business']==v['undo']==0 for v in waits),'Actual wait roles differ')
    need(all(1400<=v['elapsed_ms']<2000 for v in waits if not v['cancel']),'Absolute deadline differs')
    drop=read(p/'observations/commit-check-drops.json');need(drop['checked_drop'] and drop['unpolled_drop'] and drop['business']==drop['undo']==0,'Drop behavior differs')
    noop=read(p/'observations/commit-check-noop.json');need(noop['empty'] and noop['zero_row'] and noop['branch_requests']==noop['control_rows']==0,'No-op behavior differs')
    callback=read(p/'observations/commit-check-callback-first.json');need(callback['prior_phase']==2 and callback['actual_Undo_wait'] and callback['checked'] is False and callback['guard']==callback['terminal']==1,'Prior callback arbitration differs')
    local=read(p/'observations/commit-check-local-first.json');need(local['actual_recovery_wait'] and local['callback_after_local_rollback'] and local['guard']==local['terminal']==1,'Local-first callback behavior differs')
    legacy=read(p/'observations/commit-check-legacy.json');need(len(legacy)==2 and {v['legacy_eleven_columns'] for v in legacy}=={True,False} and all(v['row_version']==1 and v['business']==v['undo']==0 and v['control']==[1,0,0] for v in legacy),'Legacy compatibility differs')

def run(manifest):
    values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both profiles required')
    previous=catalog(PREVIOUS,'2d705af619ab9bd0ea68eb3c22ed2eaffc10f2f68086dc313a735a188facba03')
    catalog(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    old_inputs=read(PREVIOUS/'source-inputs.json')
    old_prepared=next(v for v in old_inputs if Path(v['path'])==ROOT/'crates/seata_db_mysql/src/mysql_prepared_transaction.rs')
    api_red=Path('D:/codex-builds/seata-commit-preconditions-api-red-20261117.log')
    need(b'lock_commit_preconditions' not in (PREVIOUS/old_prepared['snapshot']).read_bytes() and api_red.read_bytes().count(b'error[E0599]')==1,'Compiler API RED not bound to old source')
    old_lock=next(v for v in old_inputs if Path(v['path'])==ROOT/'Cargo.lock')
    before=tomllib.loads((PREVIOUS/old_lock['snapshot']).read_text('utf-8'))['package'];after=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package']
    need([v for v in before if v.get('source')]==[v for v in after if v.get('source')] and sum(bool(v.get('source')) for v in after)==185,'Registry versions changed')
    out=ROOT/'docs/phase-2/evidence/mysql-commit-preconditions-gate'/('run_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print('COMMIT_PRECONDITIONS_GATE_RUN '+str(out),flush=True)
    cache={digest:PREVIOUS/leaf for leaf,digest in previous.items()}
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',api_red,ROOT/'docs/phase-2/mysql-commit-preconditions-contract.md',ROOT/'docs/phase-2/mysql-commit-preconditions-file-map.csv'}
    maintenance=values.get('data_maintenance',[]);need(len(maintenance)==8,'Stopped data maintenance evidence missing')
    for item in maintenance:
        p=ROOT/item['path'];need(p.resolve().is_relative_to((ROOT/'docs/phase-2/evidence/mysql-commit-preconditions').resolve()) and sha(p)==item['sha256'],'Maintenance audit differs')
        audit=read(p);need(audit['confirmed_container_absent'] is True and audit['reparse_objects_excluded'] is True and audit['business_validation'] is False and all(v['sha256']==v['post_sha256'] for v in audit['regular_files']),'Stopped-data byte preservation differs')
        paths.add(p)
    java=module('commit_check_java_validation',ROOT/'tools/formal-commit-preconditions/java_runtime.py')
    packets={};runtimes=[];negative=[];audits={};member_rejections=[]
    for rust,profile in values['profiles'].items():
        need(set(profile)=={'native','preparation','regression','java'},'Missing actual role')
        for name,item in profile.items():
            p=ROOT/item['path'];need(p.resolve().is_relative_to((ROOT/'docs/phase-2/evidence/mysql-commit-preconditions').resolve()),'Packet escaped')
            members=catalog(p,item['catalog_sha256']);result=read(p/'result.json');role(result,rust,name)
            required=require_members(members,name)
            for leaf in sorted(required):
                weakened=dict(members);weakened.pop(leaf)
                try:require_members(weakened,name)
                except ValueError:member_rejections.append(dict(rust=rust,role=name,missing=leaf));continue
                raise ValueError('Uncatalogued consumed member accepted')
            actual_summaries=re.findall(r'test result: .*',(p/'live-tests.log').read_text('utf-8'))
            need(actual_summaries==result['actual_test_summary'],'Actual executable stdout differs from claimed summary')
            built=[]
            for line in (p/'build.jsonl').read_bytes().splitlines():
                try:value=json.loads(line)
                except ValueError:continue
                if value.get('reason')=='compiler-artifact' and value['target']['name']==result['selected_DB_target'] and value.get('executable'):built.append(value)
            need(len(built)==1 and built[0]['profile']['test'] is True,'Exact actual live target artifact missing')

            cache.update({digest:p/leaf for leaf,digest in members.items()})
            for v in read(p/'source-inputs.json'):
                need(members.get(v['snapshot'])==v['sha256']==sha(Path(v['path'])),'Current packet source drift: '+v['path']);paths.add(Path(v['path']))
            need((p/'rust-version.txt').read_text('utf-8').startswith('rustc '+rust+'.'),'Actual live compiler differs')
            cleanup=read(p/'cleanup.json');need(cleanup['owned_container_removed'] is True and cleanup['observer_error'] is None,'Cleanup incomplete')
            runtimes.append(read(p/'owned-runtime.json'));packets[rust+'/'+name]=dict(path=str(p),catalog_sha256=item['catalog_sha256'])
            if name=='native':native(p)
            if name=='regression':
                origin=read(p/'observations/callback-early-positive.json');need(origin['known_None_positive_origin']==1 and origin['late_enroll']=='CallbackObserved','Registration knowledge changed')
                children=read(p/'observations/callback-actual-old-reader.json');need(children['old_exit']!=0 and children['new_exit']==0 and children['old_pending_preserved'],'Actual old/current reader proof missing')
            if name=='java':
                observation=read(p/'observations/java-first-phase-preparation.json');need(observation['commit_preconditions_locked'] is True,'Java case omitted original commit arbitration')
                verified=java.validate(p/'java-tc',observation);need(verified==read(p/'java-tc/validation.json')==result['Java_peer'],'Original Java evidence differs')
                controls=java.negative_controls(p/'java-tc',observation);need(controls==read(p/'java-tc/negative-controls.json') and controls['count']==15,'Java negative controls differ')
                negative.append(dict(rust=rust,**controls));audits[rust]=verified
    need(len({v['container_id'] for v in runtimes})==8,'Live database roles reused')
    for directory in [ROOT/'crates',ROOT/'tools/formal-commit-preconditions',HERE]:
        paths.update(p for p in directory.rglob('*') if p.is_file())
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);data=data.encode('utf-8') if isinstance(data,str) else data;digest=hashlib.sha256(data).hexdigest();canonical=cache.get(digest)
        if canonical and not p.exists() and canonical.stat().st_nlink<900 and sha(canonical)==digest:os.link(canonical,p)
        else:p.write_bytes(data);cache[digest]=p
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
    save('source-inputs.json',frozen);save('explicit-packets.json',values);save('consumed-packets.json',packets);save('wire-negative-controls.json',negative);save('actual-Java-TC.json',audits);save('missing-member-negative-controls.json',member_rejections)
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
        doc_count=sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',docs)));need(doc_count==4 and b'branch_callback_receipt.rs' in docs and b'registered_branch.rs' in docs and b'mysql_prepared_transaction.rs' in docs and b'mysql_commit_checked_transaction.rs' in docs,'Opaque receipts, prepared and checked no-COMMIT documentation cases required')
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(map(int,counts));need(passed==339,'Ordinary test count differs')
        artifacts=[]
        for line in raw.splitlines():
            try:r=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(r,dict):artifacts.append(r)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')];need(len(tested)==len(counts)==73,'Actual test harness count differs')
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

    save('result.json',dict(status='SELECTED_COMMIT_PRECONDITIONS_VERIFIED_FULL_COMMIT_GATES_OPEN',ordinary=ordinary,source_inputs=len(frozen),fresh_DB_executions_per_profile=37,native_commit_check_tests_per_profile=8,concrete_commit_check_observations_per_profile=21,fresh_preparation_regressions_per_profile=10,fresh_callback_ownership_regressions_per_profile=18,fresh_original_Java_TC_cases_per_profile=1,wire_negative_controls=sum(v['count'] for v in negative),role_negative_controls=len(rejected),missing_member_negative_controls=len(member_rejections),qualified_owned_DB_containers=8,registry_archive_candidates=185,actual_compiled_registry_packages=150,original_Java_JUnit_executed=False,new_Java_auth_fixture_compiled=True,commit_precondition_arbitration_verified=True,same_actual_engine_transaction_record_locks_verified=True,API_compiler_RED_verified=True,compiler_RED_not_runtime_RED=True,public_first_phase_business_COMMIT_available=False,first_phase_SQL_Undo_commit_atomicity_verified=False,all_old_first_phase_attempts_fenced=False,commit_direction_verified=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',source_objects_state='PARTIAL',previous_first_phase_preparation_evidence=str(PREVIOUS),previous_wire_and_enrollment_fault_observations='inherited; not reexecuted by this gate'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]))
    print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
