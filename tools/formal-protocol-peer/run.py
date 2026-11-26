"""固定Java证据、正式Rust和显式Go0001–0006的全语料字段/字节比较；不启动服务。"""
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
import uuid

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
GO=Path('D:/codex-builds/sif_20261112_232026_e565657b')


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as stream:stream.write(data.encode('utf-8') if isinstance(data,str) else data)
def save(path,value):put(path,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec)
    sys.modules[name]=value;spec.loader.exec_module(value);return value


def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-1/evidence/protocol-peer-profile'/('run_'+stamp)
    runtime=Path('D:/codex-builds')/('formal_peer_'+stamp)
    out.mkdir(parents=True);runtime.mkdir()
    print('PEER_RUN '+str(out),flush=True)
    env=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
    def command(args,log,cwd=ROOT,extra=None):
        value=subprocess.run([str(v) for v in args],cwd=cwd,env=env|(extra or {}),stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
        put(out/log,value.stdout)
        if value.returncode:raise RuntimeError(log+': exit '+str(value.returncode))
        return value.stdout
    # 首先保留自己将加载的工具源；corpus.read还在解析前保留每次实际读取的原字节。
    initial={p:p.read_bytes() for p in HERE.iterdir() if p.is_file() and p.suffix in ['.py','.go']}
    corpus=module('formal_peer_corpus',HERE/'corpus.py')
    support=module('formal_peer_support',ROOT/'tools/java-global-status-contract/run.py')
    audit=module('formal_peer_audit',ROOT/'tools/go-java-v1-inflight/audit.py')
    sys.path.insert(0,str(ROOT/'tools/protocol-fixtures'))
    validators=[module('formal_peer_'+name,ROOT/'tools/protocol-fixtures'/('verify_formal_'+name+'_oracles.py')) for name in ['payload','merge','frame']]
    validation=[v.verify() for v in validators]
    original_go=audit.sdk.c.audit_go();candidate=audit.run(GO)
    save(out/'go-original-before.json',original_go);save(out/'go-candidate-before.json',candidate)
    cases,read_inputs=corpus.build()
    if (sum(r['accepted'] for r in cases),sum(not r['accepted'] for r in cases))!=(628,2050):raise ValueError('Corpus scope changed')
    goenv=audit.sdk.c.GO_ENV|dict(GOPROXY='off',GOSUMDB='off')
    paths=set(initial)|set(read_inputs)|{ROOT/'Cargo.toml',ROOT/'Cargo.lock'}
    paths.update(p for p in (ROOT/'crates/seata_protocol').rglob('*') if p.is_file())
    paths.update((ROOT/'crates').rglob('Cargo.toml'))
    paths.update(p for p in (ROOT/'tests/fixtures/protocol/v1').rglob('*') if p.is_file())
    paths.update(p for p in (ROOT/'tools/protocol-fixtures').iterdir() if p.is_file() and p.suffix in ['.py','.java'])
    paths.update(support.python_supports(support,audit,*validators,*tuple(sys.modules.values())))
    paths.update(GO/row['path'] for group in candidate.values() for row in group)
    patch_dir=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498'
    paths.update(p for p in patch_dir.iterdir() if p.suffix in ['.patch','.json'])
    paths.update([ROOT/'docs/phase-0/v1-implementation-acceptance-checklist.md',ROOT/'docs/phase-0/proposals/v1-protocol-allowlist.md',
        ROOT/'docs/phase-0/proposals/approval-status.json',ROOT/'docs/adr/0008-wire-optionality-and-unknown-outcomes.md'])
    for value in validation:
        paths.update(Path(p) for p in value.get('source_files',[]))
    frozen=[]
    for path in sorted(paths):
        data=initial.get(path,read_inputs.get(path,path.read_bytes()))
        if path.read_bytes()!=data:raise ValueError('Input drift before execution: '+str(path))
        snapshot='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin'
        put(out/snapshot,data);frozen.append(dict(path=str(path),sha256=hashlib.sha256(data).hexdigest(),snapshot=snapshot))
    save(out/'source-inputs.json',frozen)
    java_paths=[p for p in read_inputs if p.is_relative_to(support.ORACLE)]
    if command(['git','-C',support.ORACLE,'rev-parse','HEAD'],'java-head.log').decode().strip()!='cba73e180fd8896a99df6415b859d275122feaa7':raise ValueError('Java HEAD differs')
    command(['git','-C',support.ORACLE,'diff','--exit-code','HEAD','--']+[p.relative_to(support.ORACLE) for p in java_paths],'java-source-clean.log')
    jar=support.ORACLE/'server/target/seata-server.jar'
    jar_sha=sha(jar)
    if jar_sha!=audit.sdk.h.JAR_HASH:raise ValueError('Java JAR differs')
    save(out/'java-authority.json',dict(jar=str(jar),sha256=jar_sha,new_java_execution=False,source_pin='cba73e180fd8896a99df6415b859d275122feaa7'))
    save(out/'cases.json',cases)
    tsv=''.join(r['id']+'\t'+r['kind']+'\t'+r['hex']+'\n' for r in cases)
    put(out/'input.tsv',tsv);put(runtime/'input.tsv',tsv)
    for leaf in ['peer-comparator-red.log','peer-encode-error-red.log']:
        put(out/('tdd/'+leaf),(Path('D:/codex-builds/seata-rust-protocol-start-20261113')/leaf).read_bytes())
    save(out/'tdd/kind.json',dict(comparator_red='missing-module import failure, not runtime assertion failure',
        rust_helper_red='one real runtime assertion: injected encode failure was classified as rejected',production_codec_red_claimed=False))
    command(['python','-m','unittest','discover','-s','tools/formal-protocol-peer','-p','test_corpus.py','-v'],'python-tests.log')
    command(['cargo','fmt','--package','seata-protocol','--','--check'],'fmt.log')
    if command(['gofmt','-d',HERE/'peer_probe.go'],'gofmt.log'):raise ValueError('Go helper not formatted')
    command(['cargo','clippy','--offline','--locked','--package','seata-protocol','--all-targets','--','-D','warnings'],'clippy.log')
    command(['cargo','doc','--offline','--locked','--package','seata-protocol','--no-deps'],'rustdoc.log',extra=dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-doc',RUSTDOCFLAGS='-D warnings'))
    command(['rustc','--version','--verbose'],'rust-version.log');command(['cargo','--version'],'cargo-version.log')
    artifacts=[]
    def rust_tests(cargo,extra,label):
        raw=command([cargo,'test','--offline','--locked','--package','seata-protocol','--lib','--tests','--message-format=json'],label+'-tests.jsonl',extra=extra)
        counts=sorted(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored',raw))
        if counts!=[1,3,5,7,8,8,12,30]:raise ValueError('Need exactly74 formal tests')
        helper=command([cargo,'test','--offline','--locked','--package','seata-protocol','--example','peer_profile_probe','--message-format=json'],label+'-helper-tests.jsonl',extra=extra)
        if b'1 passed; 0 failed; 0 ignored' not in helper:raise ValueError('Need real Rust helper encode-error test')
        for data,prefix in [(raw,'protocol'),(helper,'validation-helper')]:
            for line in data.splitlines():
                if not line.startswith(b'{'):continue
                row=json.loads(line)
                if row.get('reason')=='compiler-artifact' and row.get('executable') and row.get('profile',{}).get('test'):
                    path=Path(row['executable']);snapshot='artifacts/'+label+'/'+prefix+'/'+row['target']['name']+'.exe'
                    put(out/snapshot,path.read_bytes());artifacts.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    rust_tests('cargo',{},'rust-1-99')
    msrv=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
    command([msrv/'rustc.exe','--version'],'msrv-version.log')
    rust_tests(msrv/'cargo.exe',dict(RUSTC=str(msrv/'rustc.exe'),RUSTDOC=str(msrv/'rustdoc.exe'),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production-msrv'),'rust-1-85')
    if len(artifacts)!=18:raise ValueError('Need16 protocol test binaries+two helper test binaries')
    command(['cargo','build','--offline','--locked','--package','seata-protocol','--example','peer_profile_probe'],'rust-probe-build.log')
    rust=Path(env['CARGO_TARGET_DIR'])/'debug/examples/peer_profile_probe.exe'
    command(['go','version'],'go-version.log',GO,goenv)
    deps_raw=command(['go','list','-mod=readonly','-deps','-json',HERE/'peer_probe.go'],'go-packages.json',GO,goenv)
    packages=[];tail=deps_raw.decode();decoder=json.JSONDecoder()
    while tail.strip():
        tail=tail.lstrip();row,length=decoder.raw_decode(tail);packages.append(row);tail=tail[length:]
    dependency_paths=set()
    for package in packages:
        for field in ['GoFiles','CgoFiles','CFiles','HFiles','SFiles','FFiles','MFiles','SwigFiles','SwigCXXFiles','SysoFiles','EmbedFiles']:
            dependency_paths.update(Path(package['Dir'])/p for p in package.get(field,[]))
    dependencies=[dict(path=str(p),sha256=sha(p)) for p in sorted(dependency_paths)]
    save(out/'go-dependency-inputs.json',dependencies)
    go_binary=runtime/'go-peer.exe'
    command(['go','build','-mod=readonly','-o',go_binary,HERE/'peer_probe.go'],'go-build.log',GO,goenv)
    command(['go','version','-m',go_binary],'go-build-info.log',GO,goenv)
    for path,name in [(rust,'rust-peer.exe'),(go_binary,'go-peer.exe')]:
        snapshot='artifacts/'+name;put(out/snapshot,path.read_bytes());artifacts.append(dict(path=str(path),sha256=sha(path),snapshot=snapshot))
    rust_raw=command([rust,runtime/'input.tsv'],'rust-output.log')
    go_raw=command([go_binary,runtime/'input.tsv'],'go-output.log',GO,goenv)
    if go_raw.count(b'NATIVE_REGISTRY_UNCHANGED')!=1:raise ValueError('Native registry marker missing')
    go_rows=corpus.parse_output(go_raw);rust_rows=corpus.parse_output(rust_raw)
    comparison=corpus.compare(cases,go_rows,rust_rows)
    save(out/'comparison.json',comparison)
    # 对实际输出做破坏，验证即使双方同错也会被独立Java语义预期拦截；不是生产代码RED。
    mutations=[]
    def mutate(name,identity,change,both=False):
        go_copy=dict(go_rows);rust_copy=dict(rust_rows)
        row=copy.deepcopy(go_copy[identity]);change(row);go_copy[identity]=row
        if both:rust_copy[identity]=copy.deepcopy(row)
        save(out/('mutations/'+name+'.json'),dict(identity=identity,actual_original=go_rows[identity],mutated=row,both_peers=both))
        try:corpus.compare(cases,go_copy,rust_copy)
        except ValueError as error:mutations.append(dict(name=name,rejected=True,reason=str(error)))
        else:raise ValueError('Evidence mutant escaped: '+name)
    def field(row,indices,value):
        cursor=row['semantic']
        for index in indices[:-1]:cursor=cursor[index]
        cursor[indices[-1]]=value
    mutate('both_wrong_timeout','ordinary:GlobalBeginRequest',lambda r:field(r,['fields',0],2001),True)
    mutate('bool_is_not_one','ordinary:RegisterTMResponse',lambda r:field(r,['fields',0],1))
    mutate('exact_i64','ordinary:BranchRegisterResponse',lambda r:field(r,['fields',3],0x0102030405060709))
    mutate('raft_domain','exception:2_NotRaftLeader',lambda r:field(r,['fields',2],24))
    mutate('wire_version_null_empty','merge_profile:AbsentTMVersion',lambda r:field(r,['fields',0,0,'fields',1],''))
    mutate('report_absent_unknown','merge_profile:NullReportStatus',lambda r:field(r,['fields',0,0,'fields',2],0))
    mutate('header_null_empty','new-java-frame:HeaderNull',lambda r:field(r,['headers',0,1],''))
    mutate('signed_request_id','new-java-frame:SignedMinimum',lambda r:field(r,['id'],0))
    mutate('wire_bytes','ordinary:GlobalBeginRequest',lambda r:r.update(hex=r['hex'][:-2]+'00'))
    mutate('encoder_error_not_rejection','bad-type:105',lambda r:r.update(status='encode_error'))
    mutate('panic_not_rejection','bad-type:105',lambda r:r.update(status='panic'))
    # 集合破坏保留实际结果身份，防止只比较交集或遗漏例外。
    missing=dict(go_rows);missing.pop('ordinary:GlobalBeginRequest')
    try:corpus.compare(cases,missing,rust_rows)
    except ValueError as error:mutations.append(dict(name='missing_case',rejected=True,reason=str(error)))
    else:raise ValueError('Missing case escaped')
    save(out/'actual-output-mutation-controls.json',mutations)
    if len(mutations)!=12:raise ValueError('Need12 actual-output mutation controls')
    for row in frozen:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Source input drift: '+row['path'])
    for row in dependencies+artifacts:
        if sha(Path(row['path']))!=row['sha256']:raise ValueError('Dependency/artifact drift: '+row['path'])
    if candidate!=audit.run(GO) or original_go!=audit.sdk.c.audit_go() or sha(jar)!=jar_sha:raise ValueError('Pinned peer source drift')
    save(out/'go-candidate-after.json',audit.run(GO));save(out/'test-artifacts.json',artifacts)
    save(out/'result.json',dict(approved_profile='native V1 / Java authority / explicit Go0001-0006 adapter',
        formal_rust_tests=74,validation_helper_rust_tests=1,quality_scope='seata-protocol',rust_1_99=True,msrv_1_85=True,python_tests=7,
        positive_contexts=628,positive_payloads=576,positive_frames=52,negative_complete_input_cases=2050,
        independent_java_semantics=True,actual_output_mutation_controls=12,new_java_execution=False,new_go_probe_execution=True,
        go_original_members=714,go_candidate_files=24,go_auxiliary_files=13,native_registry_unchanged=True,
        original_go_default_compatibility_complete=False,WP00_R_complete=False,full_client_complete=False,
        external_services_started=0,network_fetches=0,production_codec_changed=False))
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(out.rglob('*')) if p.is_file()]
    save(out/'catalog.json',dict(members=members))
    print(json.dumps(dict(run=str(out),source_inputs=len(frozen),members=len(members),catalog_sha256=sha(out/'catalog.json'))),flush=True)


if __name__=='__main__':run()
