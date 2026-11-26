"""Fresh Java/Rust/payload regressions followed by actual opt-in Getty frame tests."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import uuid

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('getty_merge_support',ROOT/'tools/go-java-v1-merge-profile/run.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
spec=importlib.util.spec_from_file_location('getty_prepare_support',HERE/'prepare_candidate.py')
prep=importlib.util.module_from_spec(spec);spec.loader.exec_module(prep)
l=m.l;e=m.e;h=m.h;c=m.c
PATCH=m.PATCH.with_name('0004-java-v1-getty-frame-profile.patch')
MANIFEST=PATCH.with_name('getty_profile_manifest.json')
PACKAGES=['./pkg/protocol/codec','./pkg/util/bytes','./pkg/util/errors','./pkg/protocol/message','./pkg/remoting/getty']
NEW_TESTS={'TestJavaV1GettyCapturedAndComposedFrames','TestJavaV1GettyHeaderOptionalityAndOrder','TestJavaV1GettyRejectsMalformedFrames',
    'TestJavaV1GettyRejectsLossyWrites','TestJavaV1GettyPropagatesPayloadErrors','TestJavaV1GettyDefaultAndExplicitSelection','TestJavaV1GettyFrameSizeBoundary'}


def top(path):
    events=[json.loads(line) for line in path.read_bytes().splitlines() if line.startswith(b'{')]
    if any(row.get('Action')=='fail' for row in events):raise RuntimeError('Go test failure in '+path.name)
    return {(row['Package'],row['Test']) for row in events if row.get('Action')=='pass' and row.get('Test') and '/' not in row['Test']}


def compose(controls,merged,exceptions,ordinary):
    if [len(group) for group in [controls,merged,exceptions,ordinary]]!=[9,38,250,29]:raise RuntimeError('Frame composition source inventory differs')
    if {r['name'] for r in controls}!=set(c.collect.NAMES) or {r['name'] for r in merged}!=m.NAMES or {r['name'] for r in exceptions}!=e.EXPECTED:
        raise RuntimeError('Frame source identities differ')
    rows=[dict(name='Java_'+r['name'],frame_hex=r['frame_hex'],source='java-frame') for r in controls]
    results={2,4,6,8,10,12,14,16,18,22,102,104,60}
    for prefix,group in [('Ordinary',ordinary),('Merge',merged),('Exception',exceptions)]:
        for index,row in enumerate(group):
            body=bytes.fromhex(row['serialized_hex']);kind=1 if struct.unpack('>H',body[:2])[0] in results else 0
            frame=bytes.fromhex('dada01')+struct.pack('>IHBbbi',16+len(body),16,kind,1,0,-index-1)+body
            rows.append(dict(name=prefix+'_'+row['name'],frame_hex=frame.hex(),source='tool-frame/java-certified-payload',payload_sha256=hashlib.sha256(body).hexdigest()))
    if len(rows)!=326 or len({row['name'] for row in rows})!=326:raise RuntimeError('Frame identity/count differs')
    return rows


def inherited_copies():
    return {
      'pkg/protocol/codec/java_v1_length_compat_test.go':ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go',
      'pkg/protocol/codec/java_v1_response_profile_test.go':e.HERE/'java_v1_response_profile_test.go',
      'pkg/protocol/codec/java_v1_message_profile_test.go':m.HERE/'java_v1_message_profile_test.go',
      'tools/seata-rust-golden/main.go':ROOT/'tools/go-golden-vectors/main.go',
      'tools/seata-rust-merge-profile/exception_common.go':e.HERE/'common.go',
      'tools/seata-rust-merge-profile/main.go':m.HERE/'merge_main.go',
      'pkg/remoting/getty/java_v1_profile_readwriter_test.go':HERE/'java_v1_profile_readwriter_test.go',
    }


def audit_candidate(work):
    manifest=json.loads(MANIFEST.read_bytes())
    changes={r['path']:r['patched_sha256'] for r in json.loads((l.PATCH_DIR/'manifest.json').read_bytes())['files']}
    changes.update({r['path']:r['patched_sha256'] for r in manifest['files'] if r['original_sha256'] is not None})
    original=l.audit_cohort(work,changes,(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
    extra=json.loads(e.MANIFEST.read_bytes())['files']+json.loads(m.MANIFEST.read_bytes())['files']+manifest['files']
    for row in extra:
        if h.sha(work/row['path'])!=row['patched_sha256']:raise RuntimeError('Candidate source drifted: '+row['path'])
    # 可编辑模板也必须对应实际执行的补丁内容，防止新模板认证旧实现。
    for path,data in prep.files().items():
        if (work/path).read_bytes()!=data:raise RuntimeError('Editable/generated Getty source differs from executed patch: '+path)
    copies=inherited_copies()
    for path,source in copies.items():
        if (work/path).read_bytes()!=source.read_bytes():raise RuntimeError('Executed test/harness drifted: '+path)
    expected={row['path'] for row in original if row['path'].endswith('.go')}|{row['path'] for row in extra}|set(copies)
    actual={path.relative_to(work).as_posix() for path in work.rglob('*.go')}
    if expected!=actual:raise RuntimeError('Unexpected/missing candidate .go source: '+str(expected^actual))
    return dict(original_members=original,profile_files=extra,auxiliary=[dict(path=path,sha256=h.sha(work/path)) for path in sorted(copies)])


def live(out,work):
    directory=out/'live';directory.mkdir()
    runtime=Path('D:/codex-builds')/('seata-getty-live-'+out.name.removeprefix('run_'));runtime.mkdir(exist_ok=False)
    if h.port_open(28091):raise RuntimeError('Owned live-test port is already occupied')
    jar=c.ORACLE/'server/target/seata-server.jar'
    if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR drifted')
    binary=directory/'getty-live.exe'
    l.run_command(['go','build','-o',binary,HERE/'live_main.go'],work,c.GO_ENV,directory/'go-build.log')
    config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091')
    h.write(runtime/'application.yml',config);h.write(directory/'application.yml',config)
    env=h.ENV.copy();env.update(SEATA_TC_WORK_DIR=runtime.as_posix())
    tc=None;success=False;proof=None
    try:
        tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),'-jar',jar,'-h','127.0.0.1'],directory,'tc',runtime,env)
        h.wait_until(lambda:h.port_open(28091),90,'Owned Getty live TC startup',tc)
        result=subprocess.run([str(binary),'127.0.0.1:28091'],cwd=runtime,env=env,capture_output=True,creationflags=h.FLAGS,timeout=60)
        h.write(directory/'client.log',result.stdout+result.stderr)
        lines=[line.split(b'GETTY_LIVE_PASS ',1)[1] for line in result.stdout.splitlines() if line.startswith(b'GETTY_LIVE_PASS ')]
        if result.returncode or len(lines)!=1:raise RuntimeError('Actual Java TC roundtrip failed')
        proof=json.loads(lines[0])
        expected=dict(identified=True,java_version='2.8.0-SNAPSHOT',client_wire_version='1.1.0',outer_id=2,merge_children=3,mixed_result_order=True,begin_queries=2,rollbacks=2,finished_queries=2,sent=8,received=8,transaction_resends=0)
        if any(proof.get(k)!=v for k,v in expected.items()):raise RuntimeError('Live actual transaction inventory differs')
        xids=proof['xids']
        if len(xids)!=2 or len(set(xids))!=2 or not all(re.fullmatch(r'(?:\d{1,3}\.){3}\d{1,3}:28091:\d+',xid) for xid in xids):raise RuntimeError('Original live XID identity differs')
        if not all(xid.encode() in (directory/'tc-stdout.log').read_bytes() for xid in xids):raise RuntimeError('Live original XIDs missing from actual TC logs')
        success=True
    finally:
        h.stop(tc)
        h.wait_until(lambda:not h.port_open(28091),10,'Owned Getty TC port cleanup')
        state=dict(succeeded=success,runtime_directory=str(runtime),tc_pid=tc.pid if tc else None,port_released=not h.port_open(28091),jar_sha256=h.sha(jar),binary_sha256=h.sha(binary),source_sha256=h.sha(HERE/'live_main.go'),
          proof=proof,scope='Actual unchanged Java TC, FILE, auth disabled, no RM/Undo; actual handler over TCP, no full SDK client-selection claim.')
        h.write(directory/'result.json',json.dumps(state,indent=2)+'\n')
    return state


def run():
    manifest=json.loads(MANIFEST.read_bytes())
    if h.sha(PATCH)!=manifest['patch_sha256']:raise RuntimeError('Getty patch hash differs')
    for name,digest in manifest['requires'].items():
        if h.sha(PATCH.with_name(name))!=digest:raise RuntimeError('Getty prerequisite patch differs')
    out=ROOT/'docs/phase-0/evidence/local-go-java-getty-profile'/('run_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True,exist_ok=False)
    sources=list(HERE.glob('*.go'))+list(HERE.glob('*.py'))+[PATCH,MANIFEST,ROOT/'docs/phase-0/go-java-v1-getty-profile-plan.md',ROOT/'tools/java-tc-integration/config/application.yml']
    frozen={path:path.read_bytes() for path in sources}
    parent,base=m.run(extra_sources=sources)
    if (base['java_selected_tests'],base['rust_tests'],base['go_top_level_tests'])!=(475,117,110):raise RuntimeError('Inherited inventory differs')
    # 保留载荷阶段已经验证的副本；帧阶段应用补丁时使用新的独立目录。
    work=Path('D:/codex-builds')/('seata-go-getty-profile-'+out.name.removeprefix('run_'))
    shutil.copytree(Path(base['cohort']),work)
    ex=ROOT/base['parent_evidence'];child=ROOT/base['java_rust_evidence']
    controls=json.loads((parent/'controls/vectors.json').read_bytes());merged=json.loads((parent/'capture/vectors.json').read_bytes());exceptions=json.loads((ex/'capture/vectors.json').read_bytes())
    ordinary=json.loads((ROOT/'tools/go-golden-vectors/expected.json').read_bytes())
    h.write(out/'vectors.json',json.dumps(compose(controls,merged,exceptions,ordinary),indent=2)+'\n')
    code=l.run_command(['go','run',HERE/'native_red_main.go',parent/'controls/vectors.json'],c.GO_ROOT,c.GO_ENV,out/'go-original-red.log',expected=None)
    red=(out/'go-original-red.log').read_bytes()
    if code!=1 or {line.split(b': ',1)[1].decode() for line in red.splitlines() if line.startswith(b'GETTY RED missing typed body: ')}!=m.ORIGINAL or b'GETTY RED empty input returns error rather than waiting' not in red:
        raise RuntimeError('Expected actual Getty baseline behavior failures missing')
    env=c.GO_ENV.copy();env.update(SEATA_GETTY_VECTORS=str(out/'vectors.json'),SEATA_MERGE_VECTORS=str(parent/'capture/vectors.json'),SEATA_EXCEPTION_VECTORS=str(ex/'capture/vectors.json'),SEATA_JAVA_LENGTH_VECTORS=str(child/'capture/vectors.json'))
    l.run_command(['go','test',*PACKAGES,'-count=1','-json'],work,env,out/'go-before-getty-tests.jsonl')
    old=top(out/'go-before-getty-tests.jsonl')
    h.write(out/'candidate.patch',PATCH.read_bytes());h.write(out/'patch-manifest.json',MANIFEST.read_bytes())
    git=['git','-c','core.autocrlf=false','-c','core.safecrlf=false']
    l.run_command(git+['apply','--check',out/'candidate.patch'],work,h.ENV,out/'patch-check.log')
    l.run_command(git+['apply',out/'candidate.patch'],work,h.ENV,out/'patch-apply.log')
    test=work/'pkg/remoting/getty/java_v1_profile_readwriter_test.go';test.write_bytes(frozen[HERE/test.name])
    h.write(out/'go-getty-test-executed.go',test.read_bytes())
    l.run_command(['go','test',*PACKAGES,'-count=1','-json'],work,env,out/'go-after-getty-tests.jsonl',timeout=240)
    current=top(out/'go-after-getty-tests.jsonl')
    if current!=old|{('seata.apache.org/seata-go/v2/pkg/remoting/getty',name) for name in NEW_TESTS}:raise RuntimeError('Getty regression test identity set differs')
    live_result=live(out,work)
    audit=audit_candidate(work);h.write(out/'candidate-source-audit.json',json.dumps(audit,indent=2)+'\n')
    for path,data in frozen.items():
        if path.read_bytes()!=data:raise RuntimeError('Getty ROOT input changed after capture')
    for row in json.loads((child/'source-input-audit.json').read_bytes()):
        if h.sha(ROOT/row['path'])!=row['sha256']:raise RuntimeError('Frozen inherited source drifted')
    c.audit_go()
    summary=dict(succeeded=True,executed_at=datetime.datetime.now().astimezone().isoformat(),java_selected_tests=475,rust_tests=117,go_top_level_tests=len(current),new_go_tests=len(NEW_TESTS),
      java_frame_cases=9,tool_frame_java_payload_cases=317,all_frame_cases=326,java_pin=h.PIN,go_pin=manifest['go_pin'],cohort=str(work),parent_evidence=parent.relative_to(ROOT).as_posix(),java_rust_evidence=child.relative_to(ROOT).as_posix(),
      parent_summary_sha256=h.sha(parent/'summary.json'),patch_sha256=h.sha(PATCH),original_jar_sha256=h.sha(c.ORACLE/'server/target/seata-server.jar'),live_tc=live_result,
      scope='Actual RpcPackageHandler explicit injection; fragmentation/coalescing/nullable ordered headers/errors verified. Default client selection unchanged; full SDK/production Rust/WP00 remain open.')
    h.write(out/'summary.json',json.dumps(summary,indent=2)+'\n')
    (out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8')
    print(json.dumps(summary));return out,summary


if __name__=='__main__':run()
