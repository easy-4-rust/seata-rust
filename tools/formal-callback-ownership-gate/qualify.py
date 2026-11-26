"""Bounded callback-origin qualification; all full V1 and first-phase gates remain open."""
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
PREVIOUS=ROOT/'docs/phase-2/evidence/mysql-rm-recovery-gate/run_20261117_125220_94d33cc1'
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
    target={'native':'live_callback_ownership','faults':'live_callback_ownership_loss','java':'live_callback_java'}[name]
    count=18 if name=='native' else 1;summaries=3 if name=='faults' else 1
    need(result['status']=='SELECTED_CALLBACK_OWNERSHIP_OBSERVATIONS_VERIFIED' and result['verified'] is True and result['Rust']==rust and result['selected_DB_target']==target and result['selected'] is None and result['fault']==('all' if name=='faults' else None),'Role/profile/target differs')
    need(len(result['actual_test_summary'])==summaries and all(re.match(r'test result: ok\. '+str(count)+r' passed; 0 failed; 0 ignored;',s) for s in result['actual_test_summary']),'Actual selected count differs')
    for flag in ['public_COMMIT_available','TC_registration_gate_passed','SQL_Undo_commit_atomicity_verified','WP05_complete']:need(result[flag] is False,'Overclaimed scope: '+flag)
    need(bool(result['Java_peer'])==(name=='java'),'Original Java role confused with controlled peer')
def run(manifest):
    manifest=manifest.resolve();values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both compiler profiles required')
    catalog(PREVIOUS,'76eb459a59986e96d29fc51cfa21d4029f9b969014f6f76326f3b69b5a9310e5');catalog(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    old_inputs=read(PREVIOUS/'source-inputs.json');old_lock=next(r for r in old_inputs if Path(r['path'])==ROOT/'Cargo.lock')
    old=tomllib.loads((PREVIOUS/old_lock['snapshot']).read_text('utf-8'))['package'];new=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package']
    need([r for r in old if r.get('source')]==[r for r in new if r.get('source')] and sum(bool(r.get('source')) for r in new)==185,'Registry candidates changed')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/mysql-callback-ownership-gate'/('run_'+stamp);out.mkdir(parents=True);print('CALLBACK_OWNERSHIP_GATE_RUN '+str(out),flush=True)
    cache={};paths={Path(__file__),manifest,ROOT/'docs/phase-2/mysql-callback-ownership-contract.md',ROOT/'docs/phase-2/mysql-callback-ownership-file-map.csv',ROOT/'Cargo.toml',ROOT/'Cargo.lock'};runtimes=[];packets={};audits={};negative=[]
    # 仅复用已封存且当前内容摘要相同的存储字节；旧检查结果不能替代本次执行。
    storage=ROOT/'docs/phase-2/evidence/mysql-callback-ownership-gate/run_20261117_143145_bd51c8f6'
    stored=catalog(storage,'79115eaee1a8ab6b468e0c8a365044bf9a7f40f9b49451b7ed4e37b76eff75f6');cache.update({digest:storage/leaf for leaf,digest in stored.items()})
    wire=module('callback_independent_wire',ROOT/'tools/formal-callback-ownership/validate.py');java=module('callback_independent_java',ROOT/'tools/formal-callback-ownership/java_runtime.py')
    for rust,profile in values['profiles'].items():
        need(set(profile)=={'native','faults','java'},'Missing profile role')
        for name,item in profile.items():
            p=ROOT/item['path'];need(p.resolve().is_relative_to((ROOT/'docs/phase-2/evidence').resolve()),'Packet path escaped')
            members=catalog(p,item['catalog_sha256']);result=read(p/'result.json');role(result,rust,name)
            cache.update({digest:p/leaf for leaf,digest in members.items()})
            for r in read(p/'source-inputs.json'):
                need(members.get(r['snapshot'])==r['sha256']==sha(Path(r['path'])),'Current live source differs: '+r['path']);paths.add(Path(r['path']))
            need((p/'rust-version.txt').read_text('utf-8').startswith('rustc '+rust+'.'),'Actual live compiler differs')
            cleanup=read(p/'cleanup.json');need(cleanup['owned_container_removed'] is True and cleanup['observer_error'] is None,'Live cleanup incomplete');runtimes.append(read(p/'owned-runtime.json'))
            packets[rust+'/'+name]=dict(path=str(p),catalog_sha256=item['catalog_sha256'])
            if name=='native':
                optional=read(p/'observations/callback-early-positive.json');need(optional['late_enroll']=='CallbackObserved' and optional['origin']==2 and optional['known_None_positive_origin']==1 and optional['known_None_positive_phase']==0 and optional['known_None_duplicate_enrollment']=='accepted','Known None and unknown callback lock knowledge not independently exercised')
                facts=read(p/'observations/callback-actual-old-reader.json');need(facts['old_exit']!=0 and facts['new_exit']==0 and facts['old_pending_preserved'] is True and facts['new_child']['before']=='Pending' and facts['new_child']['after']=='Completed','Actual old/new reader evidence missing')
                legacy=read(p/'observations/child-recovery.json');need(legacy['before']=='Pending' and legacy['after']=='Completed','Legacy child regression missing')
                need(read(p/'legacy-reader-binding.json')['sha256']=='a3a7bbcbb56db5e05a514767c79a16f490af68d18a01adc2d80671041db80321','Archived old executable differs')
            if name=='faults':
                for mode in ['block_commit','drop_ok','delay_ok']:
                    prefix=p/'faults'/mode;w=read(prefix/'mysql-wire.json');o=read(prefix/'observations/callback-origin-wire-observation.json');verified=wire.validate(w,o);need(verified==read(prefix/'wire-validation.json'),'Independent wire result differs');n=wire.negative_controls(w,o);need(n==read(prefix/'wire-negative-controls.json') and n['count']==(16 if mode=='delay_ok' else 14),'Negative controls differ');negative.append(dict(rust=rust,mode=mode,**n))
            if name=='java':
                verified=java.validate(p/'java-tc',read(p/'observations/java-callback-ownership.json'));need(verified==read(p/'java-tc/validation.json')==result['Java_peer'],'Independent original Java evidence differs')
                controls=java.negative_controls(p/'java-tc',read(p/'observations/java-callback-ownership.json'));need(controls==read(p/'java-tc/negative-controls.json') and controls['count']==15,'Original Java negative controls differ');negative.append(dict(rust=rust,mode='original-Java-TC',**controls))
                need(not any(read(p/'java-tc/cleanup.json')['listeners'].values()),'Owned Java port not released')
                audits[rust]=verified
    need(len({r['container_id'] for r in runtimes})==6,'Live DB roles reused')
    red=ROOT/values['red']['path'];catalog(red,values['red']['catalog_sha256']);need(read(red/'result.json')['status']=='EXPECTED_TARGET_BEHAVIOR_RED' and 'validated callback must recover lost registration without a fabricated success receipt' in (red/'live-tests.log').read_text('utf-8'),'Target behavior RED missing')
    for directory in [ROOT/'crates',ROOT/'tools/formal-callback-ownership',HERE]:paths.update(p for p in directory.rglob('*') if p.is_file())
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
        for name,wrong_rust,wrong_name in [('wrong_profile','1.85' if rust=='1.99' else '1.99','native'),('controlled_peer_as_java',rust,'java'),('native_as_fault',rust,'faults')]:
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
        doc_count=sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',docs)));need(doc_count==2 and b'branch_callback_receipt.rs' in docs and b'registered_branch.rs' in docs,'Both opaque receipt compile-failure tests required')
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(map(int,counts));need(passed==339,'Ordinary test count differs')
        artifacts=[]
        for line in raw.splitlines():
            try:r=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(r,dict):artifacts.append(r)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')];need(len(tested)==len(counts)==68,'Actual test harness count differs')
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
    save('result.json',dict(status='SELECTED_CALLBACK_OWNERSHIP_RECOVERY_VERIFIED_FULL_GATES_OPEN',ordinary=ordinary,source_inputs=len(frozen),fresh_DB_executions_per_profile=22,fresh_legacy_managed_DB_regressions_per_profile=11,fresh_original_Java_TC_cases_per_profile=1,wire_negative_controls=sum(r['count'] for r in negative),role_negative_controls=len(rejected),qualified_owned_DB_containers=6,registry_archive_candidates=185,actual_compiled_registry_packages=150,original_Java_JUnit_executed=False,new_Java_auth_fixture_compiled=True,callback_unknown_registration_handoff_verified=True,public_first_phase_business_COMMIT_available=False,first_phase_SQL_Undo_commit_atomicity_verified=False,all_old_first_phase_attempts_fenced=False,commit_direction_verified=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',source_objects_state='PARTIAL'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
