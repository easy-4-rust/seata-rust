"""Selected bounded JSON codec gate; full source-case parity, Undo persistence and COMMIT remain open."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import uuid

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'
CACHE=Path('D:/codex-builds/seata-rust-cargo-home')
MSRV=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
NATIVE=ROOT/'docs/phase-2/evidence/mysql-type-gate/run_20261115_214918_18929a67'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/undo-codec-gate'/('run_'+stamp)
    work=Path('D:/codex-builds')/('seata_undo_codec_gate_'+stamp)
    out.mkdir(parents=True);work.mkdir()
    print('UNDO_CODEC_GATE_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME=str(CACHE),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    for name in ['RUSTC','RUSTDOC']:env.pop(name,None)
    def put(name,data):
        path=out/name;path.parent.mkdir(parents=True,exist_ok=True)
        with path.open('xb') as f:f.write(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name,cwd=ROOT,extra=None,expected=0):
        result=subprocess.run([str(x) for x in args],cwd=cwd,env=env|(extra or {}),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
        put(name,result.stdout);need(result.returncode==expected,name+' unexpected exit '+str(result.returncode));return result.stdout
    def verify_packet(path):
        members=read(path/'catalog.json')['members']
        for row in members:need(sha(path/row['path'])==row['sha256'],'Child archived member differs')
        return dict(path=str(path),catalog_sha256=sha(path/'catalog.json'),members=len(members))
    raw=command(['cargo','tree','--locked','--offline','--workspace','--target','x86_64-pc-windows-msvc','-e','normal,build,dev',
        '--prefix','none','--format','{p}','--no-dedupe'],'actual-tree.txt')
    active={tuple(line.split()[:2]) for line in raw.decode().splitlines() if len(line.split())>1 and not line.startswith('seata-')}
    active={(name,version[1:]) for name,version in active};need(len(active)==150,'Expected measured 150 actual registry packages')
    command(['cargo','metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],'metadata.json')
    lock=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))
    old_row=next(row for row in read(NATIVE/'source-inputs.json') if Path(row['path'])==ROOT/'Cargo.lock')
    old_lock=tomllib.loads((NATIVE/old_row['snapshot']).read_text('utf-8'))
    registry=lambda data:[x for x in data['package'] if x.get('source','').startswith('registry+')]
    need(registry(lock)==registry(old_lock),'Registry lock versions, checksums or edges changed')
    paths={ROOT/'Cargo.toml',ROOT/'Cargo.lock',Path(__file__),JAVA/'script/client/at/db/mysql.sql'}
    for directory in ['crates','tests/fixtures/protocol/v1','tests/fixtures/undo','tests/fixtures/mysql/java_git_assets',
        'tools/formal-undo-codec','tools/formal-undo-format','tools/formal-mysql-foundation']:
        paths.update(p for p in (ROOT/directory).rglob('*') if p.is_file())
    paths.update((ROOT/'docs/phase-2').glob('*contract.md'))
    for name in ['java-resource-provenance-correction.json','undo-source-probe-assets.json']:paths.add(ROOT/'docs/phase-2'/name)
    paths.update([BUILD/'build.json',BUILD/'catalog.json'])
    paths.update(p for p in Path('D:/codex-builds/seata-connector8-source-20261115').iterdir() if p.is_file())
    dependencies=[]
    for package in registry(lock):
        name=package['name']+'-'+package['version'];archive=CACHE/'registry/cache/index.crates.io-1949cf8c6b5b557f'/(name+'.crate')
        need(sha(archive)==package['checksum'],'Locked archive checksum differs')
        used=(package['name'],package['version']) in active;count=0
        with tarfile.open(archive,'r:gz') as bundle:
            for member in bundle.getmembers():
                if used and member.isfile():
                    relative=Path(member.name).relative_to(name);need('..' not in relative.parts,'Unsafe crate member')
                    path=CACHE/'registry/src/index.crates.io-1949cf8c6b5b557f'/name/relative
                    with bundle.extractfile(member) as source:need(path.read_bytes()==source.read(),'Actual dependency source differs')
                    paths.add(path);count+=1
        paths.add(archive);dependencies.append(dict(name=package['name'],version=package['version'],checksum=package['checksum'],active=used,actual_sources=count))
    need(len(dependencies)==185,'Expected unchanged 185 locked registry archives');save('dependencies.json',dependencies)
    frozen=[]
    for path in sorted(paths):
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(snapshot,path.read_bytes())
        frozen.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path)))
    source_map={Path(x['path']):x for x in frozen};save('source-inputs.json',frozen)
    # 已发生的目标断言RED保留原执行源码；编译API缺失单独分类。
    for family,base in [('default-budget',Path('D:/codex-builds/seata-default-document-budget-red-20261115')),
        ('decimal-scale',Path('D:/codex-builds/seata-codec-decimal-scale-red-20261115'))]:
        need(b'test result: FAILED. 0 passed; 1 failed;' in (base/'behavior-red.log').read_bytes(),'Missing actual behavior RED')
        for path in base.rglob('*'):
            if path.is_file():put('tdd/'+family+'/'+path.relative_to(base).as_posix(),path.read_bytes())
    save('tdd/classification.json',dict(default_budget='Actual {} expansion bypassed node/scalar limits',
        decimal_scale='Actual original Java rejected values accepted by old Rust decoder; target assertion RED reproduced',
        JDBC_diagnostic='Historical Java setObject(BigInteger) truncated to signed long; explicitly bound exact BigDecimal, no driver patch',
        source_assets='Historical Windows checkout copies differ from literal Git blobs only by line endings; canonical copies separately stored'))
    for package in ['seata-at','seata-db-mysql']:
        command(['cargo','fmt','-p',package,'--','--check'],package+'/fmt.log')
        command(['cargo','clippy','--locked','--offline','-p',package,'--all-targets','--','-D','warnings'],package+'/clippy.log')
        command(['cargo','doc','--locked','--offline','-p',package,'--no-deps'],package+'/rustdoc.log',extra=dict(RUSTDOCFLAGS='-D warnings',CARGO_TARGET_DIR='D:/cargo-target/seata-undo-codec-doc'))
    artifacts=[];restarts=[]
    for label,cargo,extra in [('1.99','cargo',{}),('1.85',MSRV/'cargo.exe',dict(RUSTC=str(MSRV/'rustc.exe'),
        RUSTDOC=str(MSRV/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'))]:
        print('WORKSPACE '+label,flush=True)
        command([extra.get('RUSTC','rustc'),'--version','--verbose'],label+'/version.txt',extra=extra)
        raw=command([cargo,'test','--locked','--offline','--workspace','--lib','--tests','--message-format=json','--','--nocapture'],label+'/tests.jsonl',extra=extra)
        need(sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed;',raw)))==316,'Expected 297 old +19 codec tests')
        compiled=set();count=0
        for line in raw.splitlines():
            try:row=json.loads(line)
            except (ValueError,UnicodeDecodeError):continue
            if row.get('reason')=='compiler-artifact' and row.get('package_id','').startswith('registry+'):
                compiled.add(tuple(row['package_id'].rsplit('#',1)[1].split('@')))
            if row.get('reason')=='compiler-artifact' and row.get('profile',{}).get('test') and row.get('executable'):
                path=Path(row['executable']);snapshot='artifacts/'+label+'/'+path.name;put(snapshot,path.read_bytes())
                artifacts.append(dict(path=str(path),snapshot=snapshot,sha256=sha(path),rust=label,target=row['target']['name']));count+=1
        need(compiled==active,'Actual compiler artifact registry set differs from tree');need(count==46,'Expected 46 actual workspace targets')
        save(label+'/compiled-registry-packages.json',sorted(compiled))
        groups=re.findall(rb'process-restart-evidence=(D:/codex-builds/\S+) phase=(Commit|Rollback) apply=1 recover=1\s+children=3 terminated=true',raw)
        need(len(groups)==2,'Actual RM recovery groups missing')
        for directory,phase in groups:
            directory=Path(directory.decode());need(directory.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Unexpected restart path')
            for path in directory.rglob('*'):
                if path.is_file():put('process-restarts/'+label+'/'+phase.decode()+'/'+path.relative_to(directory).as_posix(),path.read_bytes())
            restarts.append(dict(rust=label,phase=phase.decode(),actual_children=3))
    save('test-artifacts.json',artifacts);save('process-restarts.json',restarts)
    def isolated(name):
        directory=work/name;directory.mkdir()
        for item in ['Cargo.toml','Cargo.lock']:shutil.copyfile(ROOT/item,directory/item)
        for item in ['crates','tests/fixtures']:shutil.copytree(ROOT/item,directory/item)
        return directory
    controls=[]
    for name,file,before,after,target,test_file in [
        ('default-budget','jackson_undo_log_parser.rs','StrictJson::validate(&value, self.limits)?;\n        }','// mutated control\n        }','default_branch_obeys_structure_limits_while_its_input_remains_two_bytes','undo_jackson_document'),
        ('decimal-scale','value_format.rs','"java.math.BigDecimal" => Self::decimal(payload),','"java.math.BigDecimal" if payload.is_number() => Ok(()),','big_decimal_exponent_and_scale_follow_the_original_java_rejections','undo_jackson_document'),
        ('decoded-binary','value_format.rs','if decoded.len() > 1_048_576 {','if false {','text_and_decoded_binary_allow_exactly_one_mib_and_reject_the_next_byte','undo_codec_value_bounds'),
        ('duplicate-key','strict_json.rs','if object.contains_key(&key) {','if false {','duplicate_escaped_keys_unknown_classes_and_trailing_content_are_rejected','undo_jackson_document'),
        ('unsigned-wrapper','prepared_format.rs','MySqlType::Int if unsigned => (4, ValueEncoding::Typed("java.lang.Long")),','MySqlType::Int if unsigned => (4, ValueEncoding::Natural),','unsigned_and_bigint_wrappers_follow_actual_connector8_types_even_for_small_values','undo_prepared_encoding')]:
        print('ASSERTION_CONTROL '+name,flush=True);directory=isolated(name)
        path=directory/'crates/seata_at/src/undo/parser'/file;text=path.read_text('utf-8');need(text.count(before)==1,'Control anchor differs: '+name)
        path.write_text(text.replace(before,after,1),encoding='utf-8')
        raw=command(['cargo','test','--locked','--offline','-p','seata-at','--test',test_file,target,'--','--exact','--nocapture'],
            'controls/'+name+'.log',cwd=directory,extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-undo-codec-controls'),expected=101)
        need(b'test result: FAILED. 0 passed; 1 failed;' in raw and b'panicked at crates' in raw and b'error[E' not in raw,'Control must compile and fail exact target assertion')
        put('controls/'+name+'.rs',path.read_bytes());controls.append(dict(name=name,source=str(path),snapshot='controls/'+name+'.rs',sha256=sha(path),target=target,target_assertion_failed=True))
    save('controls.json',controls)
    children=[]
    for rust in ['1.99','1.85']:
        for suite,expected in [('metadata',9),('row_images',10),('image_race',1),('execution',17),('mysql_types',1),('undo_codec',1)]:
            print('REAL_MYSQL '+rust+' '+suite,flush=True)
            tool=ROOT/('tools/formal-undo-codec/live.py' if suite=='undo_codec' else 'tools/formal-mysql-foundation/live.py')
            raw=command([sys.executable,tool,'--suite',suite,'--rust',rust],'live-'+rust+'-'+suite+'.log')
            match=re.search(rb'MYSQL_(?:METADATA|ROW_IMAGES|IMAGE_RACE|EXECUTION|TYPE_IDENTITY|UNDO_CODEC)_RUN ([^\r\n]+)',raw);need(match,'Child path missing')
            child=Path(match[1].decode());result=read(child/'result.json')
            need(result['succeeded'] and result['actual_live_tests']==expected and not result['cleanup_errors'],'Missing real test or cleanup')
            bound=0
            for row in read(child/'source-inputs.json'):
                parent=source_map.get(Path(row['path']));need(parent and parent['sha256']==row['sha256'],'Child source not parent frozen: '+row['path']);bound+=1
            children.append(verify_packet(child)|dict(rust=rust,suite=suite,actual_live_tests=expected,parent_bound_inputs=bound))
            if suite=='undo_codec':need(read(child/'prepared-peer/result.json')['Java_runtime_type_value_container_MATCH']==2,'Missing actual JDBC differential')
    save('live-children.json',children)
    # 独立修改真实编码后的二进制字节；Rust本地DML仍通过，必须由原Java值比较发现。
    print('REAL_MYSQL_ASSERTION_CONTROL reverse-binary',flush=True);directory=isolated('live-reverse-binary')
    for item in ['tools/formal-undo-codec','tools/formal-undo-format','tools/formal-mysql-foundation']:shutil.copytree(ROOT/item,directory/item)
    (directory/'docs/phase-2').mkdir(parents=True)
    for path in (ROOT/'docs/phase-2').glob('*contract.md'):shutil.copyfile(path,directory/'docs/phase-2'/path.name)
    path=directory/'crates/seata_at/src/undo/parser/prepared_format.rs';text=path.read_text('utf-8')
    before='FieldValue::Binary(value) => Value::String(STANDARD.encode(value))';need(text.count(before)==1,'Binary control anchor differs')
    path.write_text(text.replace(before,'FieldValue::Binary(value) => Value::String(STANDARD.encode(value.iter().rev().copied().collect::<Vec<_>>()))',1),encoding='utf-8')
    raw=command([sys.executable,directory/'tools/formal-undo-codec/live.py'], 'live-control/wrapper.log',cwd=directory,
        extra=dict(SEATA_MYSQL_TARGET_DIR='D:/cargo-target/seata-undo-codec-live-controls',SEATA_JAVA_ORACLE_ROOT=str(JAVA),SEATA_UNDO_ORACLE_BUILD=str(BUILD)),expected=1)
    match=re.search(rb'MYSQL_UNDO_CODEC_RUN ([^\r\n]+)',raw);need(match,'Negative child missing');child=Path(match[1].decode())
    result=read(child/'result.json');need(not result['succeeded'] and result['passed_live_tests']==1 and result['failed_live_tests']==0 and not result['cleanup_errors'],'Expected successful Rust DML followed by failed Java differential')
    need(b'Java runtime type/value/container differential failed: default' in (child/'prepared-peer/actual-JDBC-Rust-comparison.stdout.log').read_bytes(),'Original Java did not detect corrupted binary')
    mutated=0
    for row in read(child/'source-inputs.json'):
        source=Path(row['path']);need(sha(source)==row['sha256'] and sha(child/row['snapshot'])==row['sha256'],'Negative source drift')
        parent=source_map.get(ROOT/source.relative_to(directory) if source.is_relative_to(directory) else source);need(parent,'Unbound negative source')
        if source==path:need(parent['sha256']!=row['sha256'],'Missing mutation');mutated+=1
        else:need(parent['sha256']==row['sha256'],'Undeclared mutation')
    need(mutated==1,'Expected exactly one changed file')
    for entry in child.rglob('*'):
        if entry.is_file():put('live-control/child/'+entry.relative_to(child).as_posix(),entry.read_bytes())
    save('live-control/result.json',dict(Rust_DML_assertions_passed=1,original_Java_value_differential_failed=True,declared_mutated_files=1,owned_container_cleaned=True))
    print('DOCUMENT_PEER',flush=True)
    raw=command([sys.executable,ROOT/'tools/formal-undo-format/document_peer.py'],'document-peer.log')
    match=re.search(rb'UNDO_DOCUMENT_PEER_RUN ([^\r\n]+)',raw);need(match,'Document peer missing');peer=Path(match[1].decode())
    need(read(peer/'result.json')['second_Java_runtime_MATCH']==40,'Expected 40 bidirectional runtime matches');save('document-peer.json',verify_packet(peer))
    source_inventory=ROOT/'docs/phase-2/evidence/undo-source-inventory/run_20261115_231536_dbc7bfc1'
    save('source-case-inventory.json',verify_packet(source_inventory)|dict(status='1135_DECLARATIONS_MISSING_runtime_expansion_unresolved'))
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Source/dependency drift')
    for row in artifacts:need(sha(Path(row['path']))==row['sha256'],'Workspace artifact drift')
    save('result.json',dict(status='selected_codec_verified_pending_review',Rust_versions=['1.99','1.85'],workspace_tests_per_toolchain=316,
        actual_test_artifacts=len(artifacts),pure_assertion_controls=len(controls),real_Java_differential_controls=1,
        actual_RM_restart_children=12,source_inputs=len(frozen),registry_archives=len(dependencies),actual_compiled_registry_packages=len(active),
        actual_dependency_sources=sum(x['actual_sources'] for x in dependencies),actual_live_tests_per_toolchain=39,
        actual_owned_MySQL_containers=13,document_Java_runtime_MATCH=40,document_second_Java_runtime_MATCH=40,
        actual_prepared_profiles_per_toolchain=2,actual_prepared_logs_per_profile=4,registry_lock_entries_unchanged=True,
        original_JUnit_tests_executed=False,Java_source_case_parity_complete=False,SQL_Undo_atomicity_verified=False,
        TC_registration_verified=False,public_COMMIT_available=False,WP05_complete=False,full_V1_complete=False,
        release_authorized=False,external_tc_risk_accepted=False,shared_lock_issue='deferred_by_user'))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save('catalog.json',dict(members=members));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'),members=len(members))),flush=True)

if __name__=='__main__':run()
