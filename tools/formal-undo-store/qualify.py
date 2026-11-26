"""Bind explicit finished live packets to fresh workspace verification; all broader AT/TC gates stay open."""
import datetime
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
BASE=ROOT/'docs/phase-2/evidence/lock-key-gate/run_20261116_011215_dc9906b8'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(value,reason):
    if not value:raise ValueError(reason)
READY='connection_proxy::tests::ready_undo_publication_after_deadline_keeps_poison_and_rolls_back_original_transaction'
RED_CASES={READY,'inbound_foreign_key_on_undo_table_rejects_flush_and_rolls_back_business','original_undo_row_and_business_share_one_uncommitted_transaction_then_rollback'}
def profile_keys(profiles):need(set(profiles)=={'1.99','1.85'},'Exactly both qualified toolchains are required')
def profile_roles(rust,proofs):
    store=proofs['store']['result'];ready=proofs['ready_handoff']['result'];legacy=proofs['legacy_database']
    need(store['verified'] is True and store['Rust']==rust and store['selected'] is None and store['status']=='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED','Store toolchain/role differs')
    need(ready['verified'] is True and ready['Rust']==rust and ready['selected']==READY and ready['status']=='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED','Ready handoff toolchain/role differs')
    need(len(legacy)==5 and len({p['path'] for p in legacy})==5 and len({p['result']['suite'] for p in legacy})==5,'Legacy database roles are duplicated or incomplete')
    need(sum(p['result']['actual_live_tests'] for p in legacy)==38,'Expected full38 legacy database cases differ')
def red_roles(red):
    need(len(red)==3 and len({p['path'] for p in red})==3 and {p['result']['selected'] for p in red}==RED_CASES,'Exact three distinct RED scenarios are required')
    for proof in red:
        r=proof['result'];need(r['verified'] is True and r['Rust']=='1.99' and r['status']=='EXPECTED_MISSING_STORE_RED' and any('test result: FAILED. 0 passed; 1 failed;' in s for s in r['actual_test_summary']),'RED target failure state differs')
def native_role(rust,proof,details=None):
    r=proof['result'];need(r['succeeded'] is True and r['rust']==rust and r['suite']=='undo_codec' and r['selected_case'] is None and r['actual_live_tests']==1 and r['passed_live_tests']==1 and r['failed_live_tests']==0,'Native codec/lock toolchain/role differs')
    details=read(ROOT/proof['path']/'lock-peer/result.json') if details is None else details
    need(details['status']=='ACTUAL_JDBC_RUST_LOCK_KEYS_MATCH' and len(details['profiles'])==2 and {p['profile'] for p in details['profiles']}=={'default','numeric_tinyint'},'Native codec/lock profile set differs')
    need(all(p['logs']==9 and p['Java_Rust_MATCH'] is True and p['prepared_codec_logs']==4 and p['actual_prepared_codec_MATCH'] is True for p in details['profiles']),'Native codec/lock per-profile comparison differs')
def run(manifest):
    manifest=manifest.resolve()
    values=read(manifest);stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    profile_keys(values['profiles'])
    out=ROOT/'docs/phase-2/evidence/undo-store-gate'/('run_'+stamp);out.mkdir(parents=True);print('UNDO_STORE_GATE_RUN '+str(out),flush=True)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def packet(raw,expected=None):
        p=Path(raw);p=p if p.is_absolute() else ROOT/p
        need(p.is_relative_to(ROOT/'docs/phase-2/evidence'),'Packet outside selected repository evidence')
        if expected:need(sha(p/'catalog.json')==expected,'Packet authority digest differs')
        members=read(p/'catalog.json')['members'];need({r['path'] for r in members}=={q.relative_to(p).as_posix() for q in p.rglob('*') if q.is_file() and q!=p/'catalog.json'},'Packet member set differs')
        for row in members:need(sha(p/row['path'])==row['sha256'],'Packet member digest differs')
        return dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=sha(p/'catalog.json'),members=len(members),result=read(p/'result.json'))
    packet(BASE,'3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07')
    old={Path(r['path']):r for r in read(BASE/'source-inputs.json')}
    for p in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:need(sha(p)==old[p]['sha256'],'Root dependency edges/versions changed')
    paths={Path(__file__),manifest,ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/phase-2/mysql-undo-store-contract.md',ROOT/'tools/formal-undo-format/runtime_binding.py'}
    paths.add(ROOT/'docs/phase-2/mysql-undo-store-file-map.csv')
    need((ROOT/'crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs').is_file() and not (ROOT/'crates/seata_db_mysql/src/undo/mysql_undo_log_manager.rs').exists(),'MySQL source package mapping differs')
    for folder in [ROOT/'crates',HERE]:paths.update(p for p in folder.rglob('*') if p.is_file())
    for p,row in old.items():
        if '/registry/' in str(p).replace('\\','/'):
            need(sha(p)==row['sha256'],'Qualified registry archive/source drift');paths.add(p)
    changes=[]
    for p,row in old.items():
        if p.is_relative_to(ROOT/'crates') and '/src/' in p.as_posix() and sha(p)!=row['sha256']:changes.append(p.relative_to(ROOT).as_posix())
    expected_changes=['crates/seata_db_mysql/src/connection_proxy.rs','crates/seata_db_mysql/src/db_error.rs','crates/seata_db_mysql/src/mysql/mod.rs','crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs','crates/seata_transport/src/netty/pending_requests_tests.rs']
    need(set(changes)==set(expected_changes),'Unexpected existing crate source changes')
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
    save('source-inputs.json',frozen);save('explicit-packets.json',values)
    live={};fresh_counts={}
    for rust,profile in values['profiles'].items():
        need(rust in ['1.99','1.85'],'Unknown actual toolchain')
        live[rust]={key:packet(profile[key]) for key in ['store','ready_handoff','native_codec_locks']}
        store=live[rust]['store'];need(store['result']['verified'] and '10 passed; 0 failed' in ' '.join(store['result']['actual_test_summary']),'Final store test count differs')
        need(store['result']['Rust']==rust,'Store toolchain differs')
        live[rust]['Java_peer']=packet(store['result']['Java_peer']['run'],store['result']['Java_peer']['catalog_sha256'])
        need(live[rust]['Java_peer']['result']['comparisons']==4,'Original manager comparison count differs')
        identity_controls=read(ROOT/live[rust]['Java_peer']['path']/'identity-negative-controls.json');need(len(identity_controls)==2,'Child identity control count differs')
        need('1 passed; 0 failed' in ' '.join(live[rust]['ready_handoff']['result']['actual_test_summary']),'Actual Ready handoff test missing')
        legacy=[packet(raw) for raw in profile['legacy_database']];need({r['result']['suite'] for r in legacy}=={'metadata','row_images','image_race','execution','mysql_types'},'Legacy DB matrix incomplete')
        need(all(r['result']['succeeded'] and r['result']['rust']==rust and r['result']['failed_live_tests']==0 for r in legacy),'Legacy DB failure/toolchain differs')
        live[rust]['legacy_database']=legacy
        profile_roles(rust,live[rust])
        native_role(rust,live[rust]['native_codec_locks'])
        for proof in [store,live[rust]['ready_handoff'],live[rust]['native_codec_locks'],*legacy]:
            p=ROOT/proof['path'];need(proof['result'].get('cleanup_errors',[])==[],'Owned live cleanup failure')
            for row in read(p/'source-inputs.json'):
                path=Path(row['path']);need(sha(path)==row['sha256'],'Final consumed live source/runtime drift')
        fresh_counts[rust]=dict(store=10,ready_handoff=1,legacy=sum(r['result']['actual_live_tests'] for r in legacy),native_codec_locks=1)
    peers={name:packet(path) for name,path in values['peers'].items()};red=[packet(p) for p in values['red']]
    need(peers['document']['result']['Java_runtime_MATCH']==40 and peers['document']['result']['second_Java_runtime_MATCH']==40,'Document differential incomplete')
    need(peers['lock']['result']['Java_Rust_MATCH']==1029 and peers['tc_source']['result']['source_scenarios']==15,'Selected lock source differential/observations incomplete')
    red_roles(red)
    need(len({live[rust][role]['path'] for rust in live for role in ['store','ready_handoff','native_codec_locks']})==6,'Toolchain/role packets reused')
    controls=[]
    def rejected(name,call,expected):
        try:call()
        except ValueError as error:need(str(error)==expected,'Role control missed target rejection');controls.append(dict(control=name,actual_rejection=str(error),actual_packet_copy_only=True))
        else:raise ValueError(name+' accepted wrong actual packet role')
    changed=copy.deepcopy(live['1.85']);changed['ready_handoff']=live['1.99']['ready_handoff'];rejected('reuse199ReadyFor185',lambda:profile_roles('1.85',changed),'Ready handoff toolchain/role differs')
    changed=copy.deepcopy(live['1.99']);changed['ready_handoff']=packet('docs/phase-2/evidence/mysql-undo-store/run_20261116_022129_a8097d59');rejected('substituteDifferentSingleGreen',lambda:profile_roles('1.99',changed),'Ready handoff toolchain/role differs')
    changed=copy.deepcopy(live['1.99']);changed['ready_handoff']['result']['verified']=False;rejected('unverifiedReady',lambda:profile_roles('1.99',changed),'Ready handoff toolchain/role differs')
    rejected('omit185Profile',lambda:profile_keys({'1.99':live['1.99']}),'Exactly both qualified toolchains are required')
    changed=copy.deepcopy(live['1.99']);changed['legacy_database'].append(changed['legacy_database'][0]);rejected('duplicateLegacySuite',lambda:profile_roles('1.99',changed),'Legacy database roles are duplicated or incomplete')
    changed=copy.deepcopy(red);changed[1]=changed[0];rejected('duplicateRedScenario',lambda:red_roles(changed),'Exact three distinct RED scenarios are required')
    changed=copy.deepcopy(red);changed[0]['result']['status']='SELECTED_MYSQL_UNDO_STORE_OBSERVATIONS_VERIFIED';rejected('greenStateMasqueradesAsRed',lambda:red_roles(changed),'RED target failure state differs')
    race=next(p for p in live['1.99']['legacy_database'] if p['result']['suite']=='image_race');rejected('raceSingleCaseMasqueradesAsNativeCodec',lambda:native_role('1.99',race),'Native codec/lock toolchain/role differs')
    changed=copy.deepcopy(read(ROOT/live['1.99']['native_codec_locks']['path']/'lock-peer/result.json'));changed['profiles'][0]['logs']=8;rejected('missingNinthNativeLog',lambda:native_role('1.99',live['1.99']['native_codec_locks'],changed),'Native codec/lock per-profile comparison differs')
    changed=copy.deepcopy(read(ROOT/live['1.99']['native_codec_locks']['path']/'lock-peer/result.json'));changed['profiles'].pop();rejected('missingSecondNativeProfile',lambda:native_role('1.99',live['1.99']['native_codec_locks'],changed),'Native codec/lock profile set differs')
    save('packet-role-negative-controls.json',controls)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for key in ['RUSTC','RUSTDOC']:env.pop(key,None)
    def command(args,name,extra=None):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,v.stdout);need(v.returncode==0,name+' failed');return v.stdout
    command(['cargo','fmt','--all','--','--check'],'fmt.log');command(['cargo','clippy','--locked','--offline','--workspace','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','check','--locked','--offline','--workspace','--all-targets','--all-features'],'all-features.log')
    command(['cargo','doc','--locked','--offline','-p','seata-db-mysql','--no-deps'],'doc.log',dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-undo-store-doc'))
    ordinary={}
    for rust in ['1.99','1.85']:
        extra={};cargo='cargo'
        if rust=='1.85':
            tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=tool/'cargo.exe';extra=dict(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv')
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/rust-version.txt',extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--all-targets','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra)
        counts=re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw);passed=sum(int(v) for v in counts);need(passed==331,'Expected325 existing lib/integration +6 existing example tests differs')
        artifacts=[]
        for line in raw.splitlines():
            try:item=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if isinstance(item,dict):artifacts.append(item)
        tested=[r for r in artifacts if r.get('reason')=='compiler-artifact' and r['profile'].get('test') and r.get('executable')]
        compiled={r['package_id'].rsplit('#',1)[1] for r in artifacts if r.get('reason')=='compiler-artifact' and r['package_id'].startswith('registry+')}
        need(len(counts)==len(tested),'Actual harness summaries differ from compiled test artifact count')
        receipts=[]
        for item in tested:
            path=Path(item['executable']);digest=sha(path);snapshot=rust+'/artifacts/'+digest[:24]+'.exe';put(snapshot,path.read_bytes());receipts.append(dict(path=str(path),sha256=digest,snapshot=snapshot))
        save(rust+'/test-artifacts.json',receipts);save(rust+'/compiled-registry-packages.json',sorted(compiled));need(len(compiled)==150,'Actual compiled dependency closure changed')
        expected_compiled={'@'.join(parts) for parts in read(BASE/rust/'compiled-registry.json')};need(compiled==expected_compiled,'Actual compiled dependency set differs from qualified baseline')
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw);need(len(groups)==2,'Actual RM recovery child process groups missing')
        children=[]
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'RM recovery directory escapes D workspace')
            for path in directory.rglob('*'):
                if path.is_file():
                    snapshot=rust+'/rm-restarts/'+phase.decode()+'/'+path.relative_to(directory).as_posix();put(snapshot,path.read_bytes());children.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
        save(rust+'/RM-child-evidence.json',children)
        ordinary[rust]=dict(passed=passed,existing_lib_integration_tests=325,existing_example_tests=6,test_artifacts=len(tested),compiled_registry_packages=len(compiled),actual_RM_children=6)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final candidate source drift')
    save('result.json',dict(status='SELECTED_UNDO_WRITE_AND_ROLLBACK_COMPONENT_VERIFIED_FULL_WP05_OPEN',live=live,peers=peers,red=red,ordinary=ordinary,fresh_DB_cases=fresh_counts,
        registry_archives=185,actual_compiled_registry_packages=150,existing_source_changes=changes,full_original_manager_migrated=False,original_JUnit_executed=False,compression_profile='NONE',
        SQL_Undo_commit_atomicity_verified=False,TC_registration_gate_passed=False,TC_safety_gate_passed=False,public_COMMIT_available=False,WP05_complete=False,WP06_complete=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),source_inputs=len(frozen))),flush=True)
if __name__=='__main__':run(Path(sys.argv[1]))
