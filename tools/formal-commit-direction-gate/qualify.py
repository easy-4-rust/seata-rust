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
PREVIOUS=ROOT/'docs/phase-2/evidence/mysql-commit-preconditions-gate/run_20261117_174713_97b962fb'
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

ROLES={'native':('live_commit_direction',12),'preconditions':('live_commit_preconditions',8),'preparation':('live_first_phase_preparation',11),'regression':('live_callback_ownership',18),'java':('live_commit_direction_java',1),**{k:('live_commit_direction_loss',1) for k in ['block_commit','drop_ok','delay_ok','block_terminal']}}
FAULTS={'block_commit':26,'drop_ok':26,'delay_ok':27,'block_terminal':25}
NATIVE_LEAVES=['positive','old-reader','waits','aliases','invalid-states','log-shapes','authority','legacy','parallel','early','schema','permission','permission-trace']
def role(result,rust,name):
    target,count=ROLES[name]
    need(result['status']=='SELECTED_COMMIT_DIRECTION_OBSERVATIONS_VERIFIED' and result['verified'] is True and result['Rust']==rust and result['selected_DB_target']==target and result['selected'] is None and result['wire_observation'] is False and result['commit_fault']==(name if name in FAULTS else None),'Role/profile/target differs')
    need(len(result['actual_test_summary'])==1 and re.match(r'test result: ok\. '+str(count)+r' passed; 0 failed; 0 ignored;',result['actual_test_summary'][0]),'Actual selected count differs')
    need(bool(result['Java_peer'])==(name=='java'),'Original Java peer role confused')
    for flag in ['public_COMMIT_available','TC_registration_gate_passed','SQL_Undo_commit_atomicity_verified','WP05_complete']:need(result[flag] is False,'Overclaimed scope')
def fault_role(name,events,observation,verified):
    need(events['mode']==observation['mode']==verified['mode']==name,'Fault role differs from actual original event mode')

def require_members(members,name,observed=()):
    required={'source-inputs.json','result.json','cleanup.json','owned-runtime.json','rust-version.txt','build.jsonl','live-tests.log','artifacts/live_rm_recovery.exe','legacy-reader-binding.json','v2-reader-binding.json','child-build.jsonl','commit-child-build.jsonl','artifacts/mysql_rm_recovery_child.exe','artifacts/mysql_commit_recovery_child.exe','server-before.txt','server-after.txt','original-undo.sql','create.log','start.log','stop.log','remove.log','mysql.log'}
    if name=='native':required.update('observations/commit-direction-'+leaf+'.json' for leaf in NATIVE_LEAVES)
    if name=='preconditions':required.update('observations/commit-check-'+leaf+'.json' for leaf in ['held','rejections','waits','drops','noop','callback-first','local-first','legacy'])
    if name=='preparation':required.update({'observations/first-phase-positive.json','observations/prepare-lock-waits.json'})
    if name=='regression':required.update({'observations/callback-early-positive.json','observations/callback-actual-old-reader.json'})
    if name in FAULTS:required.update({'mysql-wire.json','commit-wire-validation.json','commit-wire-negative-controls.json','observations/commit-direction-fault.json','observations/commit-direction-before-child.log','observations/commit-direction-fault.branches.log','observations/commit-direction-child.json','proxy-binding.json'})
    if name=='java':required.update({'observations/java-commit-direction.json','observations/java-branches.log','java-tc/RM-events.jsonl','java-tc/TM-events.jsonl','java-tc/auth-binding.json','java-tc/CallbackOwnershipAuthHandler.class','java-tc/class-load.log','java-tc/validation.json','java-tc/negative-controls.json','java-tc/cleanup.json','java-tc/owned-processes.json'})
    required.update(observed)
    need(required.issubset(members),'Consumed role evidence missing from catalog')
    return required

def native(p):
    def get(leaf):return read(p/('observations/commit-direction-'+leaf+'.json'))
    positive=get('positive');need(positive==dict(branch_id=42,business='committed-fixture',business_replayed=False,callback='PhaseTwoCommitted',commit_authorized=1,commit_terminal=1,other_branch_rows=[99,100],phase=1,public_business_COMMIT=False,rollback_terminal=0),'Root/sentinel positive differs')
    old=get('old-reader');need(old==dict(commit_authorized=0,commit_terminal=0,new_exit=0,old_exit=1,old_metadata_rejected=True,old_pending_preserved=True,rollback_terminal=1,row_version=3),'Actual old reader differs')
    waits=get('waits');need(len(waits)==2 and {v['cancel_after_actual_wait'] for v in waits}=={False,True} and all(v['authorization_retained'] and v['first_callback']=='Retryable' and v['terminal_after_recovery']==v['undo_before_recovery']==1 for v in waits),'Wait/cancel recovery differs')
    aliases=get('aliases');need(len(aliases)==6 and {v['case'] for v in aliases}=={'root_case','root_accent','root_space','child_xid','child_context','root_is_child'} and all(v['rows_preserved_before_repair']==v['before'] and v['commit_terminal_after']==1 and v['after']==(0 if v['first_refused'] else v['before']) and v['explicit_fixture_repair_then_retry']==v['first_refused'] for v in aliases),'Original identity/collation protection differs')
    invalid=get('invalid-states');need(len(invalid)==9 and len({tuple(v['invalid']) for v in invalid})==9 and all(v['result']=='Unknown' and v['undo_rows']==1 for v in invalid),'Illegal states accepted')
    shapes=get('log-shapes');need(len(shapes)==4 and {v['mode'] for v in shapes}=={'normal','guard','absent','many_orphans'} and all(v['after']==3 and v['remaining']==[5000,5001,6000] and v['business_rows']==v['commit_terminal']==1 for v in shapes) and next(v for v in shapes if v['mode']=='many_orphans')['before']==133,'Original log cleanup scope differs')
    auth=get('authority');need(len(auth)==2 and {v['initial_phase'] for v in auth}=={0,1} and all(v['auth_before_callback']==0 and v['auth_after_callback']==v['terminal']==1 and v['manual_apply']==v['manual_recover']=='Unknown' for v in auth),'Manual authority accepted')
    legacy=get('legacy');need(len(legacy)==2 and {v['old_physical'] for v in legacy}=={1,2} and all(v['old_physical']==v['row_version_preserved'] and v['auth_after_migration']==0 and v['new_callback_required'] and v['old_fields_bytes_preserved'] and v['terminal']==1 for v in legacy),'Legacy migration changed facts')
    parallel=get('parallel');need(parallel['independent_recovery_instances']==parallel['actual_recovery_row_waits_at_least']==2 and parallel['deleted_root_and_children_once']==131 and parallel['business']=='parallel-fixture' and parallel['remaining_sentinel_rows']==parallel['terminal']==1,'Concurrent duplicate cleanup repeated effects')
    permission=get('permission');need(permission['actual_delete_attempts']==1 and permission['successful_delete_then_terminal_errno']==1142 and permission['undo_preserved_before_grant']==1 and permission['authorization_retained'] and permission['same_identity_after_grant']=='Applied' and permission['business_rows']==permission['terminal']==1,'Actual DELETE/terminal failure rollback differs')
    trace=get('permission-trace');need(any(v[2]==0 and v[3]==1 and ((v[0].endswith('/delete') or v[0]=='statement/com/Execute') or (v[1] or '').startswith('DELETE FROM undo_log')) for v in trace) and any(v[2]==1142 and ((v[0].endswith('/update') or v[0]=='statement/com/Prepare') or (v[1] or '').startswith('UPDATE seata_rs_at_branch_v1 SET commit_terminal')) for v in trace),'Actual successful DELETE / denied UPDATE missing')
    deletes=[v for v in trace if v[2]==0 and v[3]==1 and v[0]=='statement/com/Execute'];denied=[v for v in trace if v[2]==1142 and v[0]=='statement/com/Prepare']
    need(len(deletes)==len(denied)==1 and len(deletes[0])==len(denied[0])==6 and deletes[0][4]==denied[0][4] and deletes[0][5]<denied[0][5],'Permission rollback actual same-thread order differs')
    schema=get('schema');need(len(schema)==8 and {v['case'] for v in schema}=={'auth_default','terminal_default','auth_signed','terminal_nullable','missing_marker','extra_column','wrong_order','trigger'} and all(v['public_result']=='Unknown' and v['callback']=='FailedRetryable' and v['undo_rows']==1 and v['phase']==v['rollback_terminal']==0 for v in schema),'Physical shape matrix incomplete')
    early=get('early');need(early['actual_local_Normal_wait'] and early['durable_commit_decision_before_check'] and early['late_precommit_refused'] and isinstance(early['local_deadlock_victim'],bool) and early['business_rows']==early['undo_rows']==0 and early['terminal']==1 and early['all_unmanaged_old_attempts_fenced'] is False,'Original early callback arbitration differs')

def run(manifest):
    values=read(manifest);need(set(values['profiles'])=={'1.99','1.85'},'Both profiles required')
    previous=catalog(PREVIOUS,'6e8267381250dba51f680a162287cf7dbd62b32b46f40cc23bf085ce375b708d')
    catalog(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    old_inputs=read(PREVIOUS/'source-inputs.json')
    old_lock=next(v for v in old_inputs if Path(v['path'])==ROOT/'Cargo.lock')
    before=tomllib.loads((PREVIOUS/old_lock['snapshot']).read_text('utf-8'))['package'];after=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package']
    need([v for v in before if v.get('source')]==[v for v in after if v.get('source')] and sum(bool(v.get('source')) for v in after)==185,'Registry versions changed')
    out=ROOT/'docs/phase-2/evidence/mysql-commit-direction-gate'/('run_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print('COMMIT_DIRECTION_GATE_RUN '+str(out),flush=True)
    cache={digest:PREVIOUS/leaf for leaf,digest in previous.items()}
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-commit-direction-contract.md',ROOT/'docs/phase-2/mysql-commit-direction-file-map.csv'}
    red_packets=[]
    for directory,digest,needle in [('run_20261117_180349_ef325b1c','db17e72450b3a099a5c14572fa02134a50d5b131fd28fa99415a3fb3c2133d56',b'Unsupported'),('run_20261117_181811_05ec12b2','4c22d84c8e813d385b7c8d1edc517624e1db641503a807e4dc70d2ca89f85915',b'PhaseTwoCommitFailedUnretryable')]:
        packet=ROOT/'docs/phase-2/evidence/mysql-commit-direction'/directory;members=catalog(packet,digest)
        need({'result.json','source-inputs.json','build.jsonl','live-tests.log','cleanup.json','owned-runtime.json','artifacts/live_rm_recovery.exe'}.issubset(members),'Runtime RED evidence uncatalogued')
        result=read(packet/'result.json');need(result['status']=='EXPECTED_TARGET_BEHAVIOR_RED' and result['verified'] and needle in (packet/'live-tests.log').read_bytes() and b'test result: FAILED. 0 passed; 1 failed;' in (packet/'live-tests.log').read_bytes(),'Actual target-behavior RED missing')
        need(read(packet/'cleanup.json')['owned_container_removed'] is True,'RED runtime remains')
        for leaf in ['catalog.json','result.json','source-inputs.json','build.jsonl','live-tests.log','cleanup.json','owned-runtime.json']:paths.add(packet/leaf)
        red_packets.append(dict(path=str(packet),catalog_sha256=digest,actual_runtime_RED=True))
    maintenance=values.get('data_maintenance',[]);need(len(maintenance)==18,'Stopped data maintenance evidence missing')
    for item in maintenance:
        p=ROOT/item['path'];need(sha(p)==item['sha256'],'Maintenance audit differs');audit=read(p)
        need(audit['confirmed_container_absent'] and audit['reparse_objects_excluded'] and audit['business_validation'] is False and all(v['sha256']==v['post_sha256'] for v in audit['regular_files']),'Stopped data bytes differ');paths.add(p)
    java=module('commit_direction_java_validation',ROOT/'tools/formal-commit-direction/java_runtime.py')
    wire=module('commit_direction_wire_validation',ROOT/'tools/formal-commit-direction/validate.py')
    oldgate=module('commit_precondition_native_validation',ROOT/'tools/formal-commit-preconditions-gate/qualify.py')
    paths.add(ROOT/'tools/formal-commit-preconditions-gate/qualify.py')
    need(sha(ROOT/'tools/formal-commit-direction/approved-sql.json')=='5c06a8135b4671f49f9f6ec9baad85388f7d6ae2837f121188e41e4c5a67e40c','Fixed reviewed instrumentation SQL differs')
    rejected_attempt=values.get('rejected_observer_attempt');need(rejected_attempt and rejected_attempt['Rust']=='1.85' and rejected_attempt['role']=='block_terminal','Explicit conservative observer rejection missing')
    rejected_packet=ROOT/rejected_attempt['path'];need(rejected_packet.resolve().is_relative_to((ROOT/'docs/phase-2/evidence/mysql-commit-direction').resolve()),'Rejected packet escaped')
    rejected_required={'mysql-wire.json','observations/commit-direction-fault.json','observations/commit-direction-before-child.log','observations/commit-direction-fault.branches.log','observations/commit-direction-child.json','live-tests.log','cleanup.json','owned-runtime.json','source-inputs.json','build.jsonl','artifacts/live_rm_recovery.exe','proxy-binding.json'}
    need(rejected_required.issubset(rejected_attempt['files']),'Rejected observer attempt consumed member omitted from explicit frozen manifest')
    for leaf,digest in rejected_attempt['files'].items():
        source=rejected_packet/leaf;need(source.resolve().is_relative_to(rejected_packet.resolve()) and sha(source)==digest,'Rejected attempt evidence drift');paths.add(source)
    need(b'test result: ok. 1 passed; 0 failed;' in (rejected_packet/'live-tests.log').read_bytes() and read(rejected_packet/'cleanup.json')['owned_container_removed'] is True,'Rejected observer attempt confused with failed DB test')
    for row in read(rejected_packet/'source-inputs.json'):
        snapshot=(rejected_packet/row['snapshot']).resolve();need(not Path(row['snapshot']).is_absolute() and snapshot.is_relative_to(rejected_packet.resolve()) and sha(snapshot)==row['sha256']==sha(Path(row['path'])),'Rejected observer source snapshot differs');paths.update([snapshot,Path(row['path'])])
    rejected_artifacts=[]
    for line in (rejected_packet/'build.jsonl').read_bytes().splitlines():
        try:value=json.loads(line)
        except ValueError:continue
        if value.get('reason')=='compiler-artifact' and value['target']['name']=='live_commit_direction_loss' and value.get('executable'):rejected_artifacts.append(value)
    need(len(rejected_artifacts)==1 and sha(Path(rejected_artifacts[0]['executable']))==sha(rejected_packet/'artifacts/live_rm_recovery.exe'),'Rejected attempt actual executable differs')
    try:wire.validate(read(rejected_packet/'mysql-wire.json'),read(rejected_packet/'observations/commit-direction-fault.json'),(rejected_packet/'observations/commit-direction-before-child.log').read_bytes(),(rejected_packet/'observations/commit-direction-fault.branches.log').read_bytes())
    except ValueError as error:need(str(error)=='COMMIT not bound to original post-mutation request','Conservative observer rejection differs')
    else:raise ValueError('Previously rejected observer attempt was accepted')
    runtime_audit=ROOT/'docs/phase-2/evidence/mysql-commit-direction/all-attempt-runtime-audit-20261117.json';need(runtime_audit.is_file(),'All-attempt read-only audit missing');paths.add(runtime_audit)
    all_attempts=read(runtime_audit);need(all_attempts['owned_containers_verified_absent']>=19 and all(v['verified_absent'] for v in all_attempts['containers']) and all(v['verified_owned_processes_absent'] for v in all_attempts['java']),'Recorded owned runtime remains')
    packets={};runtimes=[];negative=[];audits={};member_rejections=[]
    for rust,profile in values['profiles'].items():
        need(set(profile)==set(ROLES),'Missing actual role')
        for name,item in profile.items():
            p=ROOT/item['path'];need(p.resolve().is_relative_to((ROOT/'docs/phase-2/evidence/mysql-commit-direction').resolve()),'Packet escaped')
            members=catalog(p,item['catalog_sha256']);result=read(p/'result.json');role(result,rust,name)
            observed={v.relative_to(p).as_posix() for v in (p/'observations').rglob('*') if v.is_file()}
            required=require_members(members,name,observed)
            for leaf in sorted(required):
                weakened=dict(members);weakened.pop(leaf)
                try:require_members(weakened,name,observed)
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
            need(sha(Path(built[0]['executable']))==members['artifacts/live_rm_recovery.exe'],'Exact executed target differs from actual compiler artifact')
            for bind,expected_parent,expected_sha in [('legacy-reader-binding.json','6362c678ce4ee62f5185ed0eeca96e9b128038c5571b3e3493c2837c44d068ac','a3a7bbcbb56db5e05a514767c79a16f490af68d18a01adc2d80671041db80321'),('v2-reader-binding.json','90d2d8b0fef55c0639344267553320db91585445d173400a05c1b847cd14ff1b','d50627ebf5b0cc8c5a9b1ca48b8563bdecafe95dccd229a6597fe99f248a094f')]:
                binding=read(p/bind);need(binding['parent_catalog_sha256']==expected_parent and binding['sha256']==expected_sha==sha(Path(binding['source_artifact']))==sha(Path(binding['executed_artifact'])),'Actual old reader identity differs')

            cache.update({digest:p/leaf for leaf,digest in members.items()})
            for v in read(p/'source-inputs.json'):
                need(members.get(v['snapshot'])==v['sha256']==sha(Path(v['path'])),'Current packet source drift: '+v['path']);paths.add(Path(v['path']))
            need((p/'rust-version.txt').read_text('utf-8').startswith('rustc '+rust+'.'),'Actual live compiler differs')
            cleanup=read(p/'cleanup.json');need(cleanup['owned_container_removed'] is True and cleanup['observer_error'] is None,'Cleanup incomplete')
            runtime=read(p/'owned-runtime.json');need(runtime['image']=='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902' and re.fullmatch(r'[0-9a-f]{32}',runtime['label_owner']) and runtime['database']=='undo_'+runtime['label_owner'][:12] and Path(runtime['runtime']).resolve().is_relative_to(Path('D:/codex-builds').resolve()) and Path(runtime['runtime']).name.startswith('seata_commit_direction_') and Path(cleanup['D_data_retained']).resolve()==(Path(runtime['runtime'])/'data').resolve() and Path(cleanup['D_data_retained']).is_dir(),'Actual owned runtime identity or retained data differs')
            runtimes.append(runtime);packets[rust+'/'+name]=dict(path=str(p),catalog_sha256=item['catalog_sha256'])
            if name=='native':native(p)
            if name=='preconditions':oldgate.native(p)
            if name=='preparation':
                waits=read(p/'observations/prepare-lock-waits.json');need(len(waits)==4 and {(v['blocked_table'],v['cancelled']) for v in waits}=={(table,cancel) for table in ['undo_log','seata_rs_at_branch_v1'] for cancel in [False,True]} and all(v['actual_waits']>0 and v['business_rows']==v['undo_rows']==0 for v in waits),'Actual preparation wait matrix differs')
            if name in FAULTS:
                observation=read(p/'observations/commit-direction-fault.json');before=(p/'observations/commit-direction-before-child.log').read_bytes();after=(p/'observations/commit-direction-fault.branches.log').read_bytes();events=read(p/'mysql-wire.json')
                verified=wire.validate(events,observation,before,after);fault_role(name,events,observation,verified);need(verified==read(p/'commit-wire-validation.json'),'Original SQL/journal evidence differs')
                controls=wire.negative_controls(events,observation,before,after);need(controls==read(p/'commit-wire-negative-controls.json') and controls['count']==FAULTS[name],'Wire counterexamples differ')
                need(read(p/'observations/commit-direction-child.json')==observation['actual_commit_child'],'Actual independent child result differs')
                negative.append(dict(rust=rust,role=name,**controls))
            if name=='regression':
                origin=read(p/'observations/callback-early-positive.json');need(origin['known_None_positive_origin']==1 and origin['late_enroll']=='CallbackObserved','Registration knowledge changed')
                children=read(p/'observations/callback-actual-old-reader.json');need(children['old_exit']!=0 and children['new_exit']==0 and children['old_pending_preserved'],'Actual old/current reader proof missing')
            if name=='java':
                observation=read(p/'observations/java-commit-direction.json');need(observation['commit_preconditions_locked'] is True,'Java case omitted original commit arbitration')
                verified=java.validate(p/'java-tc',observation);need(verified==read(p/'java-tc/validation.json')==result['Java_peer'],'Original Java evidence differs')
                controls=java.negative_controls(p/'java-tc',observation);need(controls==read(p/'java-tc/negative-controls.json') and controls['count']==18,'Java negative controls differ')
                negative.append(dict(rust=rust,**controls));audits[rust]=verified
    need(len({v['container_id'] for v in runtimes})==18,'Live database roles reused')
    for directory in [ROOT/'crates',ROOT/'tools/formal-commit-direction',HERE]:
        paths.update(p for p in directory.rglob('*') if p.is_file())
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);data=data.encode('utf-8') if isinstance(data,str) else data;digest=hashlib.sha256(data).hexdigest();canonical=cache.get(digest)
        if canonical and not p.exists() and canonical.stat().st_nlink<900 and sha(canonical)==digest:os.link(canonical,p)
        else:
            p.write_bytes(data);cache[digest]=p
            if len(data)>=1048576:
                value=subprocess.run(['C:/Windows/System32/compact.exe','/C','/I','/Q',str(p)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=60)
                need(value.returncode==0 and sha(p)==digest,'Lossless owned evidence compression differs')
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
    save('rejected-observer-attempt.json',rejected_attempt);save('runtime-RED-packets.json',red_packets);save('source-inputs.json',frozen);save('explicit-packets.json',values);save('consumed-packets.json',packets);save('wire-negative-controls.json',negative);save('actual-Java-TC.json',audits);save('missing-member-negative-controls.json',member_rejections)
    rejected=[]
    for rust in values['profiles']:
        result=read(ROOT/values['profiles'][rust]['native']['path']/'result.json')
        for name,wrong_rust,wrong_name in [('wrong_profile','1.85' if rust=='1.99' else '1.99','native'),('controlled_peer_as_java',rust,'java'),('native_as_regression',rust,'regression')]:
            try:role(result,wrong_rust,wrong_name)
            except ValueError:rejected.append(dict(rust=rust,case=name));continue
            raise ValueError('Role negative accepted')
    for rust in values['profiles']:
        for original,wrong in [('block_commit','drop_ok'),('drop_ok','block_commit')]:
            packet=ROOT/values['profiles'][rust][original]['path']
            try:fault_role(wrong,read(packet/'mysql-wire.json'),read(packet/'observations/commit-direction-fault.json'),read(packet/'commit-wire-validation.json'))
            except ValueError:rejected.append(dict(rust=rust,case=original+'_as_'+wrong));continue
            raise ValueError('Actual original fault role substitution accepted')
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
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')];need(len(tested)==len(counts)==77,'Actual test harness count differs')
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

    save('result.json',dict(status='SELECTED_MANAGED_COMMIT_CLEANUP_VERIFIED_FULL_COMMIT_GATES_OPEN',ordinary=ordinary,source_inputs=len(frozen),fresh_DB_executions_per_profile=53,DB_count_scope='qualified explicit manifest roles only',additional_rejected_observer_qualification_attempts=1,native_commit_cleanup_tests_per_profile=12,concrete_commit_cleanup_observations_per_profile=38,fresh_precondition_regressions_per_profile=8,fresh_commit_fault_roles_per_profile=4,fresh_preparation_regressions_per_profile=10,fresh_callback_ownership_regressions_per_profile=18,fresh_original_Java_TC_cases_per_profile=1,wire_negative_controls=sum(v['count'] for v in negative),role_negative_controls=len(rejected),missing_member_negative_controls=len(member_rejections),qualified_owned_DB_containers=18,registry_archive_candidates=185,actual_compiled_registry_packages=150,original_Java_JUnit_executed=False,new_Java_auth_fixture_compiled=True,commit_precondition_arbitration_verified=True,same_actual_engine_transaction_record_locks_verified=True,managed_commit_cleanup_verified=True,selected_commit_direction_verified=True,actual_target_behavior_runtime_RED_verified=True,public_first_phase_business_COMMIT_available=False,first_phase_SQL_Undo_commit_atomicity_verified=False,all_old_first_phase_attempts_fenced=False,commit_direction_verified=False,TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',source_objects_state='PARTIAL',previous_commit_preconditions_evidence=str(PREVIOUS),previous_wire_and_enrollment_fault_observations='inherited; not reexecuted by this gate'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]))
    print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
