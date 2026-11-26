"""Requalify the review fixes, explicitly inherit unchanged DB paths from the sealed pre-fix gate."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid

ROOT=Path(__file__).resolve().parents[2]
BASE=ROOT/'docs/phase-2/evidence/undo-codec-gate/run_20261115_232915_d3f704e8'
PEER=ROOT/'docs/phase-2/evidence/undo-document-peer/run_20261115_235646_c842e6f1'
MSRV=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/undo-codec-refinement'/('run_'+stamp);out.mkdir(parents=True)
    print('UNDO_CODEC_REFINEMENT_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for name in ['RUSTC','RUSTDOC']:env.pop(name,None)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True)
        with p.open('xb') as f:f.write(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name,extra=None):
        value=subprocess.run([str(x) for x in args],cwd=ROOT,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,value.stdout);need(value.returncode==0,name+' failed');return value.stdout
    def packet(path):
        for row in read(path/'catalog.json')['members']:need(sha(path/row['path'])==row['sha256'],'Archive differs')
        return dict(path=str(path),catalog_sha256=sha(path/'catalog.json'))
    base_binding=packet(BASE);peer_binding=packet(PEER)
    previous={Path(x['path']):x for x in read(BASE/'source-inputs.json')}
    changed=[];same=[]
    for path in sorted(p for p in (ROOT/'crates').rglob('*') if p.is_file()):
        row=previous.get(path);need(row,'Unexpected new crate file after full gate')
        (same if sha(path)==row['sha256'] else changed).append(str(path.relative_to(ROOT)))
    expected={
        'crates/seata_at/src/undo/parser/strict_json.rs','crates/seata_at/tests/undo_jackson_document.rs',
        'crates/seata_at/tests/undo_prepared_encoding.rs','crates/seata_at/tests/undo_codec_value_bounds.rs'}
    need({p.replace('\\','/') for p in changed}==expected,'Review fix escaped declared production/test scope')
    for path in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']:need(sha(path)==previous[path]['sha256'],'Cargo changed after base gate')
    paths=set(previous)|{Path(__file__),ROOT/'tools/formal-undo-format/UndoNumberBounds.java',ROOT/'tests/fixtures/undo/.gitattributes'}
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(snapshot,path.read_bytes())
        frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    source_map={Path(x['path']):x for x in frozen};save('source-inputs.json',frozen)
    need(read(PEER/'result.json')['second_Java_runtime_MATCH']==40,'Expected final 40/40 runtime matches')
    for row in read(PEER/'source-inputs.json'):
        path=Path(row['path']);parent=source_map.get(path)
        if not parent:
            # 原Java源只作固定pin输入；独立peer已冻结并核对，补入本epoch引用而非当Rust实现变更。
            need(path.is_relative_to(ROOT.parent/'seata-java-phase09-oracle'),'Unbound peer implementation source')
            need(sha(path)==row['sha256'],'Original Java probe source drift')
        else:need(parent['sha256']==row['sha256'],'Final peer source differs from this epoch')
    save('base-and-peer.json',dict(base=base_binding,peer=peer_binding,changed_crate_files=changed,
        unchanged_crate_files=len(same),inherited_DB_cases_per_toolchain=38,
        inherited_DB_scope=['metadata9','row_images10','image_race1','execution17','mysql_types1'],
        inherited_controls=dict(pure=5,real_Java_binary_corruption=1),original_node_preflight='correct; Row has @class string node, exact40/39 regression added'))
    red=Path('D:/codex-builds/seata-codec-depth-number-red-20261115')
    for name in ['empty-depth','number-length']:
        need(b'test result: FAILED. 0 passed; 1 failed;' in (red/(name+'-behavior-red.log')).read_bytes(),'Missing actual behavior RED')
    for path in red.rglob('*'):
        if path.is_file():put('tdd/'+path.relative_to(red).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(empty_depth='Actual depth2 encode succeeded while same parser decode rejected',
        numeric_digits='1001 digit value accepted by old Rust but rejected by fixed original Jackson profile',
        node_preflight='No production defect; initial test expectation39 omitted @class node; corrected exact40/39 test passes unchanged prepared_format'))
    for package in ['seata-at','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/rustdoc.log',extra=dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-undo-codec-doc'))
    command(['cargo','check','--locked','--offline','--workspace','--all-features','--all-targets'],'all-features-check.log')
    artifacts=[];restarts=[]
    active=set(map(tuple,read(BASE/'1.99/compiled-registry-packages.json')))
    for rust,cargo,extra in [('1.99','cargo',{}),('1.85',MSRV/'cargo.exe',dict(RUSTC=str(MSRV/'rustc.exe'),
        RUSTDOC=str(MSRV/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('REFINEMENT_WORKSPACE '+rust,flush=True)
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],rust+'/version.txt',extra=extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--lib','--tests','--message-format=json','--','--nocapture'],rust+'/tests.jsonl',extra=extra)
        need(sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw)))==316,'Expected 316 final workspace tests')
        compiled=set();count=0
        for line in raw.splitlines():
            try:row=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if row.get('reason')=='compiler-artifact' and row.get('package_id','').startswith('registry+'):
                compiled.add(tuple(row['package_id'].rsplit('#',1)[1].split('@')))
            if row.get('reason')=='compiler-artifact' and row.get('profile',{}).get('test') and row.get('executable'):
                path=Path(row['executable']);snapshot='artifacts/'+rust+'/'+path.name;put(snapshot,path.read_bytes())
                artifacts.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path),rust=rust));count+=1
        need(compiled==active and count==46,'Final actual compiler closure/targets differs')
        save(rust+'/compiled-registry-packages.json',sorted(compiled))
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw)
        need(len(groups)==2,'RM recovery execution missing')
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Unexpected restart path')
            for path in directory.rglob('*'):
                if path.is_file():put('process-restarts/'+rust+'/'+phase.decode()+'/'+path.relative_to(directory).as_posix(),path.read_bytes())
            restarts.append(dict(rust=rust,phase=phase.decode(),actual_children=3))
    save('test-artifacts.json',artifacts);save('process-restarts.json',restarts)
    children=[]
    for rust in ['1.99','1.85']:
        print('REFINEMENT_REAL_MYSQL '+rust,flush=True)
        raw=command([sys.executable,ROOT/'tools/formal-undo-codec/live.py','--rust',rust],'live-'+rust+'.log')
        match=re.search(rb'MYSQL_UNDO_CODEC_RUN ([^\r\n]+)',raw);need(match,'Child missing');child=Path(match[1].decode())
        result=read(child/'result.json');need(result['succeeded'] and result['actual_live_tests']==1 and not result['cleanup_errors'],'Affected real codec did not pass')
        for row in read(child/'source-inputs.json'):
            parent=source_map.get(Path(row['path']));need(parent and parent['sha256']==row['sha256'],'Final actual live source differs')
        need(read(child/'prepared-peer/result.json')['Java_runtime_type_value_container_MATCH']==2,'Missing final real JDBC profiles')
        children.append(packet(child)|dict(rust=rust,actual_live_tests=1,actual_prepared_profiles=2,logs_per_profile=4))
    save('live-children.json',children)
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final epoch input drift')
    for row in artifacts:need(sha(Path(row['path']))==row['sha256'],'Executed artifact drift')
    save('result.json',dict(status='REVIEW_FIXES_VERIFIED_PENDING_REVIEW',base=base_binding,peer=peer_binding,
        workspace_tests_per_toolchain=316,actual_test_artifacts=len(artifacts),new_actual_RM_restart_children=12,
        new_actual_MySQL_cases_per_toolchain=1,inherited_MySQL_cases_per_toolchain=38,new_owned_MySQL_containers=2,
        actual_prepared_profiles_per_toolchain=2,logs_per_profile=4,document_runtime_MATCH=40,document_second_runtime_MATCH=40,
        original_number_constraint_probes=14,original_number_accepts=7,original_number_rejects=7,
        actual_behavior_RED_fixed=2,node_preflight_was_already_correct=True,
        inherited_pure_controls=5,inherited_real_Java_corruption_controls=1,registry_archives=185,actual_compiled_packages=150,
        actual_dependency_sources=6871,source_inputs=len(frozen),Java_source_case_parity_complete=False,SQL_Undo_atomicity_verified=False,
        public_COMMIT_available=False,TC_registration_verified=False,WP05_complete=False,full_V1_complete=False,
        external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)

if __name__=='__main__':run()
