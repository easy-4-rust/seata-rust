"""Fresh Java/Rust authority, inherited strict frames, real SDK loop and source closure."""
import datetime
import importlib.util
import json
from pathlib import Path
import re
import shutil
import uuid

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('sdk_getty_support',ROOT/'tools/go-java-v1-getty-profile/run.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
spec=importlib.util.spec_from_file_location('sdk_prepare',HERE/'prepare_candidate.py');prep=importlib.util.module_from_spec(spec);spec.loader.exec_module(prep)
spec=importlib.util.spec_from_file_location('sdk_live_runner',HERE/'live.py');live=importlib.util.module_from_spec(spec);spec.loader.exec_module(live)
m=g.m;l=g.l;e=g.e;h=g.h;c=g.c
PATCH=g.PATCH.with_name('0005-java-v1-sdk-session-profile.patch');MANIFEST=PATCH.with_name('sdk_profile_manifest.json')
PACKAGES=g.PACKAGES+['./pkg/remoting/config','./pkg/remoting/processor/client']


def copies():
    out=g.inherited_copies()
    for name in ['sdk_export_test.go','java_v1_sdk_dispatch_test.go','java_v1_sdk_config_test.go','java_v1_sdk_state_test.go']:out['pkg/remoting/getty/'+name]=HERE/name
    out['pkg/protocol/message/java_v1_message_future_test.go']=HERE/'java_v1_message_future_test.go'
    return out


def audit_candidate(work):
    manifests=[json.loads(path.read_bytes()) for path in [l.PATCH_DIR/'manifest.json',e.MANIFEST,m.MANIFEST,g.MANIFEST,MANIFEST]]
    expected={row['path']:row for manifest in manifests for row in manifest['files']}
    original=l.audit_cohort(work,{path:row['patched_sha256'] for path,row in expected.items()},(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
    for path,row in expected.items():
        if h.sha(work/path)!=row['patched_sha256']:raise RuntimeError('Final candidate file drifted: '+path)
    generated={**g.prep.files(),**prep.files()}
    for path,data in generated.items():
        if (work/path).read_bytes()!=data:raise RuntimeError('Editable/generated SDK implementation differs from executed patch: '+path)
    auxiliary=copies()
    for path,source in auxiliary.items():
        if (work/path).read_bytes()!=source.read_bytes():raise RuntimeError('Executed test/harness differs: '+path)
    paths={row['path'] for row in original if row['path'].endswith('.go')}|set(expected)|set(auxiliary)
    actual={path.relative_to(work).as_posix() for path in work.rglob('*.go')}
    if paths!=actual:raise RuntimeError('Unexpected/missing final SDK .go source: '+str(paths^actual))
    return dict(original_members=original,candidate_files=list(expected.values()),auxiliary=[dict(path=path,sha256=h.sha(work/path)) for path in sorted(auxiliary)])


def apply(patch,work,out,name):
    git=['git','-c','core.autocrlf=false','-c','core.safecrlf=false'];l.run_command(git+['apply','--check',patch],work,h.ENV,out/(name+'-check.log'));l.run_command(git+['apply',patch],work,h.ENV,out/(name+'-apply.log'))


def run():
    manifest=json.loads(MANIFEST.read_bytes())
    if h.sha(PATCH)!=manifest['patch_sha256']:raise RuntimeError('SDK patch identity differs')
    for name,digest in manifest['requires'].items():
        if h.sha(PATCH.with_name(name))!=digest:raise RuntimeError('SDK prerequisite identity differs')
    out=ROOT/'docs/phase-0/evidence/local-go-java-sdk-profile'/('run_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True,exist_ok=False)
    sources=list(HERE.glob('*.go'))+list(HERE.glob('*.py'))+list(g.HERE.glob('*.go'))+list(g.HERE.glob('*.py'))+[PATCH,MANIFEST,g.PATCH,g.MANIFEST,ROOT/'docs/phase-0/go-java-v1-sdk-profile-plan.md',ROOT/'docs/phase-0/java-v1-preface-fragmentation.md',ROOT/'tools/java-tc-integration/config/application.yml']
    frozen={path:path.read_bytes() for path in sources}
    parent,base=m.run(extra_sources=sources)
    if (base['java_selected_tests'],base['rust_tests'],base['go_top_level_tests'])!=(475,117,110):raise RuntimeError('Inherited source authority inventory differs')
    work=Path('D:/codex-builds')/('seata-go-sdk-profile-'+out.name.removeprefix('run_'));shutil.copytree(base['cohort'],work)
    child=ROOT/base['java_rust_evidence'];exception=ROOT/base['parent_evidence']
    apply(g.PATCH,work,out,'getty-frame-patch')
    frame_test=work/'pkg/remoting/getty/java_v1_profile_readwriter_test.go';frame_test.write_bytes(frozen[g.HERE/frame_test.name])
    vectors=g.compose(json.loads((parent/'controls/vectors.json').read_bytes()),json.loads((parent/'capture/vectors.json').read_bytes()),json.loads((exception/'capture/vectors.json').read_bytes()),json.loads((ROOT/'tools/go-golden-vectors/expected.json').read_bytes()))
    h.write(out/'vectors.json',json.dumps(vectors,indent=2)+'\n')
    env=c.GO_ENV.copy();env.update(SEATA_GETTY_VECTORS=str(out/'vectors.json'),SEATA_MERGE_VECTORS=str(parent/'capture/vectors.json'),SEATA_EXCEPTION_VECTORS=str(exception/'capture/vectors.json'),SEATA_JAVA_LENGTH_VECTORS=str(child/'capture/vectors.json'))
    l.run_command(['go','test',*PACKAGES,'-count=1','-json'],work,env,out/'go-before-sdk-tests.jsonl',timeout=240)
    old=g.top(out/'go-before-sdk-tests.jsonl')
    # 新API不存在不能作为RED。仅复制不依赖新API的基线行为测试与测试状态入口。
    native={name:work/'pkg/remoting/getty'/name for name in ['native_export_test.go','native_dispatch_test.go']}
    for name,path in native.items():path.write_bytes(frozen[HERE/name]);h.write(out/(name+'.executed.txt'),path.read_bytes())
    code=l.run_command(['go','test','./pkg/remoting/getty','-run','^TestJavaV1SDKBaseline','-count=1','-json'],work,env,out/'go-sdk-behavior-red.jsonl',expected=None)
    rows=[json.loads(line) for line in (out/'go-sdk-behavior-red.jsonl').read_bytes().splitlines() if line.startswith(b'{')]
    failed={r['Test'] for r in rows if r.get('Action')=='fail' and r.get('Test')};passed={r['Test'] for r in rows if r.get('Action')=='pass' and r.get('Test')}
    if code!=1 or failed!={'TestJavaV1SDKBaselineWholeReply','TestJavaV1SDKBaselineMalformedLegacyReply'} or passed!={'TestJavaV1SDKBaselineLegacyControl'}:raise RuntimeError('Actual SDK behavior RED identities differ')
    for name,path in native.items():
        if path.read_bytes()!=frozen[HERE/name]:raise RuntimeError('Owned baseline test drifted')
        path.unlink()
    config_test=work/'pkg/remoting/getty/java_v1_sdk_config_test.go';config_test.write_bytes(frozen[HERE/config_test.name])
    code=l.run_command(['go','test','./pkg/remoting/getty','-run','^TestJavaV1SDKProfileFlag','-count=1','-json'],work,env,out/'go-sdk-config-red.jsonl',expected=None)
    if code!=1 or b'flag provided but not defined' not in (out/'go-sdk-config-red.jsonl').read_bytes():raise RuntimeError('Runtime flag-selection RED missing')
    h.write(out/'candidate.patch',PATCH.read_bytes());h.write(out/'patch-manifest.json',MANIFEST.read_bytes());apply(out/'candidate.patch',work,out,'sdk-patch')
    for relative,source in copies().items():
        if source.parent==HERE:(work/relative).write_bytes(frozen[source])
    l.run_command(['go','test',*PACKAGES,'-count=1','-json'],work,env,out/'go-after-sdk-tests.jsonl',timeout=240)
    current=g.top(out/'go-after-sdk-tests.jsonl');new=set()
    for relative,source in copies().items():
        if source.parent!=HERE:continue
        package='seata.apache.org/seata-go/v2/'+str(Path(relative).parent).replace('\\','/')
        new|={(package,name) for name in re.findall(r'func (Test\w+)\(',source.read_text(encoding='utf-8'))}
    if len(new)!=13 or current!=old|new:raise RuntimeError('Original plus thirteen SDK test identity set differs')
    l.run_command(['go','test','./pkg/remoting/getty','-run','^TestJavaV1SDKConcurrentMerge','-count=20','-json'],work,env,out/'go-concurrent-repeat.jsonl')
    actual=live.run(out,work,g)
    audit=audit_candidate(work);h.write(out/'candidate-source-audit.json',json.dumps(audit,indent=2)+'\n')
    m.audit_candidate(Path(base['cohort']))
    for path,data in frozen.items():
        if path.read_bytes()!=data:raise RuntimeError('SDK ROOT input drifted after capture')
    for row in json.loads((child/'source-input-audit.json').read_bytes()):
        if h.sha(ROOT/row['path'])!=row['sha256']:raise RuntimeError('Frozen inherited source drifted')
    c.audit_go()
    summary=dict(succeeded=True,executed_at=datetime.datetime.now().astimezone().isoformat(),java_selected_tests=475,rust_tests=117,go_top_level_tests=len(current),inherited_go_tests=len(old),new_sdk_go_tests=len(new),frame_cases=326,java_frame_cases=9,tool_frame_cases=317,java_pin=h.PIN,go_pin=manifest['go_pin'],cohort=str(work),parent_evidence=parent.relative_to(ROOT).as_posix(),java_rust_evidence=child.relative_to(ROOT).as_posix(),parent_summary_sha256=h.sha(parent/'summary.json'),patch_sha256=h.sha(PATCH),original_jar_sha256=h.sha(c.ORACLE/'server/target/seata-server.jar'),live=actual,java_preface_issue='JAVA-V1-PREFACE-001 OPEN',scope='Explicit real SDK remoting loop/correlation candidate; TC-bound arbitrary first-byte fragmentation known failing. Native defaults/full client/real RM Undo/production Rust/WP00 remain open.')
    h.write(out/'summary.json',json.dumps(summary,indent=2)+'\n');(out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8');print(json.dumps(summary),flush=True);return out,summary

if __name__=='__main__':run()
