"""Selected managed rollback qualification; no first-phase COMMIT or full WP05/WP06 claim."""
import copy
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

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
BASE=ROOT/'docs/phase-2/evidence/lock-key-gate/run_20261116_011215_dc9906b8'
PREVIOUS=ROOT/'docs/phase-2/evidence/undo-rollback-gate/run_20261117_111348_259e729e'
spec=importlib.util.spec_from_file_location('managed_recovery_wire',ROOT/'tools/formal-rm-mysql/validate.py')
wire_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire_module)

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_bytes())
def need(value,reason):
    if not value:raise ValueError(reason)

def verify_catalog(directory,digest):
    need(sha(directory/'catalog.json')==digest,'Authority catalog identity differs')
    rows=read(directory/'catalog.json')['members'];need(len({r['path'] for r in rows})==len(rows),'Duplicate authority member')
    for row in rows:
        member=directory/row['path']
        need(not Path(row['path']).is_absolute() and member.resolve().is_relative_to(directory.resolve()) and sha(member)==row['sha256'],'Authority member differs: '+row['path'])
    return {r['path']:r['sha256'] for r in rows}

def role(result,rust,target,count,selected=None,fault=None):
    need(result['verified'] is True and result['Rust']==rust and result['selected_DB_target']==target and result['selected']==selected and result['fault']==fault and result['status']=='SELECTED_MYSQL_RM_RECOVERY_OBSERVATIONS_VERIFIED','Selected role/target/profile differs')
    need(len(result['actual_test_summary'])==1 and re.match(r'test result: ok\. '+str(count)+r' passed; 0 failed; 0 ignored;',result['actual_test_summary'][0]),'Actual selected test count differs')

def run(manifest):
    manifest=manifest.resolve();values=read(manifest)
    need(set(values['profiles'])=={'1.99','1.85'},'Both qualified compiler profiles required')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/mysql-rm-recovery-gate'/('run_'+stamp);out.mkdir(parents=True)
    print('MYSQL_RM_RECOVERY_GATE_RUN '+str(out),flush=True)
    cache_root=ROOT/'docs/phase-2/evidence/mysql-rm-recovery/run_20261117_120200_cf61fbb6'
    cache={r['sha256']:cache_root/r['path'] for r in read(cache_root/'catalog.json')['members']}
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);data=data.encode('utf-8') if isinstance(data,str) else data
        digest=hashlib.sha256(data).hexdigest();canonical=cache.get(digest)
        if not p.exists() and canonical and canonical.resolve().is_relative_to((ROOT/'docs/phase-2/evidence').resolve()) and canonical.stat().st_nlink<900 and sha(canonical)==digest:os.link(canonical,p)
        else:p.write_bytes(data);cache[digest]=p
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def packet(item,current=True):
        p=ROOT/item['path'];need(p.resolve().is_relative_to(ROOT/'docs/phase-2/evidence'),'Evidence path escapes repository')
        members=verify_catalog(p,item['catalog_sha256'])
        need({'source-inputs.json','result.json','cleanup.json','owned-runtime.json','rust-version.txt','build.jsonl','live-tests.log','artifacts/live_rm_recovery.exe'}.issubset(members),'Consumed live evidence omitted from catalog')
        inputs=read(p/'source-inputs.json')
        for row in inputs:
            need(row['snapshot'] in members and members[row['snapshot']]==row['sha256']==sha(p/row['snapshot']),'Frozen source differs')
            if current:need(sha(Path(row['path']))==row['sha256'],'Current consumed source differs: '+row['path'])
        return p,read(p/'result.json'),inputs
    verify_catalog(PREVIOUS,'cb577013e36b5bf13def0a35718e650c34e6a35763cf76e37b2ea9abe102b8a7')
    verify_catalog(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    inherited_inputs=read(PREVIOUS/'source-inputs.json')
    registry=[r for r in inherited_inputs if '/registry/' in r['path'].replace('\\','/')]
    need(registry and all(sha(Path(r['path']))==r['sha256']==sha(PREVIOUS/r['snapshot']) for r in registry),'Qualified dependency source/archive drift')
    old_lock=next(r for r in inherited_inputs if Path(r['path'])==ROOT/'Cargo.lock')
    old=tomllib.loads((PREVIOUS/old_lock['snapshot']).read_text(encoding='utf-8'))['package'];new=tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'))['package']
    need([r for r in old if r.get('source')]==[r for r in new if r.get('source')],'Registry lock closure changed')
    need(len([r for r in new if r.get('source')])==185,'Registry candidate denominator differs')
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-rm-recovery-contract.md'}
    for directory in [ROOT/'crates',ROOT/'tools/formal-rm-mysql',HERE]:paths.update(p for p in directory.rglob('*') if p.is_file())
    paths.update(Path(r['path']) for r in registry)
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
    save('source-inputs.json',frozen);save('explicit-packets.json',values)
    packets=[];runtimes=[];wire_controls=[];roles=[];live={};child_pids=[]
    for rust,profile in values['profiles'].items():
        need(set(profile)=={'native','faults','denial','concurrency','fence','rollback'},'Profile roles differ')
        need(set(profile['faults'])=={'drop_ok','block_commit','delay_ok'},'Actual fault set differs')
        result_set={}
        for name,item in [(k,v) for k,v in profile.items() if k!='faults']+list(profile['faults'].items()):
            p,result,inputs=packet(item);packets.append(str(p));runtimes.append(read(p/'owned-runtime.json'))
            need((p/'rust-version.txt').read_text(encoding='utf-8').startswith('rustc '+rust+'.'),'Recorded compiler differs')
            need(read(p/'cleanup.json')['owned_container_removed'] is True and read(p/'cleanup.json').get('observer_error') is None,'Live cleanup failed')
            if name=='native':
                role(result,rust,'live_rm_recovery',11)
                child=read(p/'observations/child-recovery.json');cancel=read(p/'observations/cancelled-dispatch-child.json')
                need(child['before']=='Pending' and child['after']=='Completed' and child['branch_status']=='PhaseTwoRollbacked' and child['pid']>0 and cancel['exit_code']==0 and cancel['actual_waits']>0,'Actual independent-process Pending recovery missing')
                child_pids.append(child['pid'])
                need((p/'artifacts/mysql_rm_recovery_child.exe').stat().st_size>0,'Exact recovery child binary missing')
            elif name in ('fence','rollback'):role(result,rust,'live_undo_'+name,9 if name=='fence' else 13)
            else:
                mode=name if name in profile['faults'] else None
                selected='lost_terminal_commit_recovers_without_replaying_compensation_or_business' if mode else ('terminal_write_denial_rolls_back_already_executed_compensation_and_undo_delete' if name=='denial' else 'concurrent_and_delayed_attempts_observe_one_durable_terminal')
                role(result,rust,'live_rm_recovery_loss' if mode else 'live_rm_recovery',1,selected,mode)
                wire=read(p/'mysql-wire.json');leaf='rm-wire-observation.json' if mode else ('rm-denial-observation.json' if name=='denial' else 'rm-concurrent-observation.json')
                observation=read(p/'observations'/leaf)
                validator=wire_module.validate if mode else (wire_module.validate_denial if name=='denial' else wire_module.validate_concurrency)
                validated=validator(wire,observation);need(validated==read(p/'wire-validation.json'),'Independent wire replay differs')
                controls=wire_module.negative_controls(wire,observation) if mode else wire_module.observer_negative_controls(wire,observation,validator)
                need(len(controls)==(12 if mode else 5),'Wire negative-control denominator differs')
                wire_controls.extend(dict(Rust=rust,role=name,**r) for r in controls)
                if mode:need(controls==read(p/'wire-negative-controls.json'),'Recorded fault negatives differ')
                save('replayed/'+rust+'-'+name+'.json',dict(result=validated,negative_controls=controls))
            result_set[name]=dict(path=item['path'],catalog_sha256=item['catalog_sha256'],actual=result['actual_test_summary'])
        live[rust]=result_set
        original=packet(profile['native'])[1]
        def reject(label,rust_value,target,count,selected=None,fault=None):
            try:role(original,rust_value,target,count,selected,fault)
            except ValueError:roles.append(dict(profile=rust,mutation=label,status='REJECTED'))
            else:raise ValueError('Wrong live role accepted')
        reject('wrong_compiler','1.85' if rust=='1.99' else '1.99','live_rm_recovery',11)
        reject('single_as_full',rust,'live_rm_recovery',1,'another_single_case')
        reject('wrong_target',rust,'live_undo_fence',11)
    need(len(packets)==len(set(packets))==16 and len(set(child_pids))==2,'Live roles or recovery children reused')
    save('wire-negative-controls.json',wire_controls);save('role-negative-controls.json',roles)
    need(set(values['red'])=={'reserved_table','alias_guard'},'Target RED set differs')
    for name,item in values['red'].items():
        p,result,_=packet(item,False);need(result['verified'] is True and result['Rust']=='1.99' and any('test result: FAILED. 0 passed; 1 failed;' in s for s in result['actual_test_summary']),'Actual target RED missing')
        text=(p/'live-tests.log').read_text(encoding='utf-8')
        need(('collation alias guard must not become Applied' if name=='alias_guard' else 'reserved recovery table must be rejected before business DML') in text,'RED target assertion differs')
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1',DOCKER_CONFIG='D:/Temp/seata-docker-config')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    def command(args,name,extra=None):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,v.stdout);need(v.returncode==0,name+' failed');return v.stdout
    command(['cargo','fmt','--all','--','--check'],'fmt.log')
    ordinary={}
    for rust in ('1.99','1.85'):
        extra={};cargo='cargo'
        if rust=='1.85':
            tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';extra=dict(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv',PATH=str(tool)+os.pathsep+env.get('PATH',''),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc')
        version=command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/rust-version.txt',extra)
        need(version.startswith(('rustc '+rust+'.').encode()),'Actual workspace compiler differs')
        clippy_version=command([cargo,'clippy','--version'],rust+'/clippy-version.txt',extra)
        need(clippy_version.startswith(('clippy 0.1.'+rust.split('.')[1]).encode()),'Clippy and compiler profile differ')
        command([cargo,'clippy','--workspace','--all-targets','--all-features','--locked','--offline','--','-D','warnings'],rust+'/clippy.log',extra)
        command([cargo,'doc','--workspace','--all-features','--no-deps','--locked','--offline'],rust+'/doc.log',extra|dict(RUSTDOCFLAGS='-D warnings'))
        raw=command([cargo,'test','--workspace','--all-targets','--locked','--offline','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra)
        doc_tests=command([cargo,'test','--workspace','--doc','--locked','--offline'],rust+'/doc-tests.log',extra)
        doc_count=sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',doc_tests)))
        need(doc_count>=1 and b'registered_branch.rs' in doc_tests,'Registered receipt documentation compile-failure example not executed')
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(map(int,counts));need(passed==336,'Actual ordinary test denominator differs')
        artifacts=[]
        for line in raw.splitlines():
            try:r=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(r,dict):artifacts.append(r)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')]
        need(len(tested)==len(counts)==64,'Actual test harness artifact denominator differs')
        compiled={r['package_id'].rsplit('#',1)[1] for r in artifacts if r.get('reason')=='compiler-artifact' and r['package_id'].startswith('registry+')}
        need(compiled=={'@'.join(parts) for parts in read(BASE/rust/'compiled-registry.json')} and len(compiled)==150,'Actual compiled registry closure differs')
        receipts=[]
        for r in tested:
            p=Path(r['executable']);digest=sha(p);leaf=rust+'/artifacts/'+digest[:24]+'.exe';put(leaf,p.read_bytes());receipts.append(dict(path=str(p),sha256=digest,snapshot=leaf))
        save(rust+'/test-artifacts.json',receipts);save(rust+'/compiled-registry.json',sorted(compiled))
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw);need(len(groups)==2,'Existing RM recovery child evidence missing')
        children=[]
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Owned RM child evidence escapes D')
            for p in directory.rglob('*'):
                if p.is_file():
                    leaf=rust+'/rm-restarts/'+phase.decode()+'/'+p.relative_to(directory).as_posix();put(leaf,p.read_bytes());children.append(dict(path=str(p),sha256=sha(p),snapshot=leaf))
        save(rust+'/RM-child-evidence.json',children)
        ordinary[rust]=dict(passed=passed,lib_integration_tests=330,existing_example_tests=6,harness_artifacts=len(tested),compiled_registry_packages=len(compiled),doc_tests=doc_count,existing_RM_child_processes=6,new_actual_MySQL_recovery_children=1)
    audits=[]
    for runtime in runtimes:
        v=subprocess.run(['docker','inspect',runtime['container_id']],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30)
        need(v.returncode!=0 and b'no such object' in v.stdout.lower(),'Qualified owned container still exists');audits.append(dict(container=runtime['container_id'],verified_absent=True))
    save('owned-runtime-final-audit.json',audits)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final qualified source drift')
    save('result.json',dict(status='SELECTED_MANAGED_MYSQL_ROLLBACK_RECOVERY_VERIFIED_FULL_WP05_WP06_OPEN',live=live,ordinary=ordinary,source_inputs=len(frozen),fresh_DB_test_executions_per_toolchain=38,distinct_DB_test_roles_per_toolchain=36,fresh_legacy_fence_tests_per_toolchain=9,fresh_legacy_normal_undo_tests_per_toolchain=13,new_actual_MySQL_child_processes=2,existing_RM_child_processes=12,wire_negative_controls=len(wire_controls),role_negative_controls=len(roles),qualified_owned_DB_containers=len(runtimes),registry_archive_candidates=185,actual_compiled_registry_packages=150,previous_Java_observations_per_toolchain_inherited_not_fresh=24,original_Java_JUnit_executed=False,selected_rollback_reconciliation_verified=True,formal_Java_TC_SQL_end_to_end_verified=False,public_first_phase_business_COMMIT_available=False,first_phase_SQL_Undo_commit_atomicity_verified=False,all_old_first_phase_attempts_fenced=False,commit_direction_verified=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',source_objects_state='PARTIAL'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]))
    print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),source_inputs=len(frozen))),flush=True)

if __name__=='__main__':run(Path(sys.argv[1]))
