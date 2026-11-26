"""Fresh complete merged-payload candidate validation; Getty integration is a separate required stage."""
import datetime
import importlib.util
import json
from pathlib import Path
import struct
import uuid

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('merge_exception_support',ROOT/'tools/go-java-v1-exception-profile/run.py')
e=importlib.util.module_from_spec(spec);spec.loader.exec_module(e)
l=e.l;h=e.h;c=e.c
HERE=Path(__file__).resolve().parent
PATCH=e.PATCH.with_name('0003-java-v1-merged-payload-profile.patch')
MANIFEST=PATCH.with_name('merge_profile_manifest.json')
KINDS=[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,21,22,101,102,103,104,111]
NAMES={f'Single_{kind}' for kind in KINDS}|{'All25WithIds','All25WithoutIds','All12Results','All250ExceptionResults','NestedAll',
    'SignedTimeout_-1','SignedTimeout_-2147483648','SignedTimeout_0','SignedTimeout_2147483647',
    'AbsentTMVersion','AbsentRMVersion','NullReportStatus','UnknownReportStatus'}
ORIGINAL={'MergeRequestWithIds','MergeRequestWithoutIds','MergeRequestEmpty','MergeRequestNested','MergeResult','MergeResultEmpty'}


def read_rows(log):
    rows=[json.loads(line.split(b'SEATA_MERGE_PROFILE ',1)[1].decode('ascii')) for line in log.read_bytes().splitlines() if line.startswith(b'SEATA_MERGE_PROFILE ')]
    if len(rows)!=38 or len({r['name'] for r in rows})!=38 or {r['name'] for r in rows}!=NAMES:
        raise RuntimeError('Need all38 unique merge samples')
    for row in rows:
        data=bytes.fromhex(row['serialized_hex'])
        if len(data)<8 or struct.unpack('>H',data[:2])[0] not in [59,60] or struct.unpack('>i',data[2:6])[0]!=len(data)-6:
            raise RuntimeError('Merge sealed length/type differs')
        if row['name'] in ['AbsentTMVersion','AbsentRMVersion']:
            if row.get('java_effective_version')!='2.8.0-SNAPSHOT' or not row.get('java_reencoded_hex') or row['java_reencoded_hex']==row['serialized_hex']:
                raise RuntimeError('Absent version must retain explicit Java getter/re-encode distinction')
    return rows


def export(rows,path):
    path.mkdir(parents=True,exist_ok=False)
    h.write(path/'vectors.json',json.dumps(rows,indent=2)+'\n')
    h.write(path/'vectors.tsv',''.join(r['name']+'\t'+r['serialized_hex']+'\n' for r in rows))
    for row in rows:h.write(path/(row['name']+'.payload.bin'),bytes.fromhex(row['serialized_hex']))
    h.write(path/'manifest.json',json.dumps(dict(java_pin=h.PIN,go_pin='82570d905919c1c66bb025a3a5979f25390de498',cases=38,
        scope='Java-generated merge payloads; absent versions preserve raw wire plus separate Java getter semantics',
        files=[dict(path=p.name,sha256=h.sha(p)) for p in sorted(path.iterdir())]),indent=2)+'\n')


def results(log):
    rows=[json.loads(line.split(b'SEATA_MERGE_RESULT ',1)[1]) for line in log.read_bytes().splitlines() if line.startswith(b'SEATA_MERGE_RESULT ')]
    if len(rows)!=44 or len({r['name'] for r in rows})!=44 or {r['name'] for r in rows}!=NAMES|ORIGINAL:
        raise RuntimeError('Need exact38+six Go merge result identities')
    if not all(all(row.get(key) is True for key in ['fields_match','bytes_match','pointer_match','caller_unchanged','roundtrip_match','java_semantics_match']) for row in rows):
        raise RuntimeError('Go merge field/byte/pointer/semantic check failed')
    return rows


def audit_candidate(work):
    length_manifest=json.loads((l.PATCH_DIR/'manifest.json').read_bytes())
    changes={row['path']:row['patched_sha256'] for row in length_manifest['files']}
    original=l.audit_cohort(work,changes,(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
    extra=json.loads(e.MANIFEST.read_bytes())['files']+json.loads(MANIFEST.read_bytes())['files']
    for row in extra:
        if h.sha(work/row['path'])!=row['patched_sha256']:raise RuntimeError('Inherited/new profile implementation drifted: '+row['path'])
    copies={
        'pkg/protocol/codec/java_v1_length_compat_test.go':ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go',
        'pkg/protocol/codec/java_v1_response_profile_test.go':e.HERE/'java_v1_response_profile_test.go',
        'pkg/protocol/codec/java_v1_message_profile_test.go':HERE/'java_v1_message_profile_test.go',
        'tools/seata-rust-golden/main.go':ROOT/'tools/go-golden-vectors/main.go',
        'tools/seata-rust-merge-profile/exception_common.go':e.HERE/'common.go',
        'tools/seata-rust-merge-profile/main.go':HERE/'merge_main.go',
    }
    for path,source in copies.items():
        if (work/path).read_bytes()!=source.read_bytes():raise RuntimeError('Executed candidate test/harness drifted: '+path)
    expected={row['path'] for row in original if row['path'].endswith('.go')}|{row['path'] for row in extra}|set(copies)
    actual={path.relative_to(work).as_posix() for path in work.rglob('*.go')}
    if actual!=expected:raise RuntimeError('Unexpected/missing Go cohort source path')
    return dict(original_members=original,profile_files=extra,executed_auxiliary=[dict(path=path,sha256=h.sha(work/path)) for path in sorted(copies)])


def run(extra_sources=()):
    if ROOT.drive.upper() not in ['D:','E:']:raise RuntimeError('Development restricted to D:/E:')
    manifest=json.loads(MANIFEST.read_bytes())
    if h.sha(PATCH)!=manifest['patch_sha256'] or h.sha(l.PATCH)!=manifest['requires_length_patch_sha256'] or h.sha(e.PATCH)!=manifest['requires_exception_patch_sha256']:
        raise RuntimeError('Merge prerequisite/source patch identity differs')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-0/evidence/local-go-java-merge-profile'/('run_'+stamp);out.mkdir(parents=True,exist_ok=False)
    capture=out/'capture';controls=out/'controls'
    sources=list(HERE.glob('*.go'))+[Path(__file__),PATCH,MANIFEST]+list(extra_sources)
    frozen={source:source.read_bytes() for source in sources}
    def capture_merge(log,parent):
        rows=read_rows(log);export(rows,capture)
        fixtures=ROOT/'tests/fixtures/protocol/v1/merge_profile'
        if not fixtures.exists():export(rows,fixtures)
        else:
            for path in capture.iterdir():
                if path.read_bytes()!=(fixtures/path.name).read_bytes():raise RuntimeError('Refusing merge fixture overwrite: '+path.name)
        c.collect.export(c.collect.read_rows(log),controls)
        h.write(out/'java-capture-source.json',json.dumps(dict(parent_evidence=parent.relative_to(ROOT).as_posix(),java_log_sha256=h.sha(log),new_merge_cases=38,control_cases=9),indent=2)+'\n')
    parent,base=e.run(extra_capture=capture_merge,rust_extra_env={'SEATA_IT_MERGE_PROFILE_DIR':str(capture),'SEATA_IT_CONTROL_WIRE_DIR':str(controls)},extra_sources=sources)
    if base['java_selected_tests']!=475 or base['rust_tests']!=117 or base['go_top_level_tests']!=104:
        raise RuntimeError('Complete inherited validation inventory differs')
    work=Path(base['cohort'])
    code=l.run_command(['go','run',HERE/'native_red_main.go',controls/'vectors.json'],c.GO_ROOT,c.GO_ENV,out/'go-original-red.log',expected=None)
    raw=(out/'go-original-red.log').read_bytes()
    if code!=1 or {line.split(b': ',1)[1].decode() for line in raw.splitlines() if line.startswith(b'MERGE RED missing typed body: ')}!=ORIGINAL:
        raise RuntimeError('Original Go missing-codec RED identity differs')
    for row in manifest['files']:
        if (work/row['path']).exists():raise RuntimeError('New merge cohort source already exists')
    h.write(out/'candidate.patch',PATCH.read_bytes());h.write(out/'patch-manifest.json',MANIFEST.read_bytes())
    git=['git','-c','core.autocrlf=false','-c','core.safecrlf=false']
    l.run_command(git+['apply','--check',out/'candidate.patch'],work,h.ENV,out/'patch-check.log')
    l.run_command(git+['apply',out/'candidate.patch'],work,h.ENV,out/'patch-apply.log')
    for row in manifest['files']:
        if h.sha(work/row['path'])!=row['patched_sha256']:raise RuntimeError('Applied merge source differs')
    harness=work/'tools/seata-rust-merge-profile';harness.mkdir(parents=True,exist_ok=False)
    for source,name in [(e.HERE/'common.go','exception_common.go'),(HERE/'merge_main.go','main.go')]:
        (harness/name).write_bytes(source.read_bytes());h.write(out/(name+'.input.txt'),source.read_bytes())
    l.run_command(['go','run','.',ROOT/'tools/go-golden-vectors/expected.json',capture/'vectors.json',controls/'vectors.json'],harness,c.GO_ENV,out/'go-profile-green.log')
    results(out/'go-profile-green.log')
    test=HERE/'java_v1_message_profile_test.go';copied=frozen[test]
    cohort_test=work/'pkg/protocol/codec'/test.name;cohort_test.write_bytes(copied);h.write(out/'go-merge-test-executed.go',copied)
    env=c.GO_ENV.copy();env['SEATA_MERGE_VECTORS']=str(capture/'vectors.json')
    child=ROOT/base['parent_evidence'];env['SEATA_EXCEPTION_VECTORS']=str(parent/'capture/vectors.json');env['SEATA_JAVA_LENGTH_VECTORS']=str(child/'capture/vectors.json')
    l.run_command(['go','test','./pkg/protocol/codec','./pkg/util/bytes','./pkg/util/errors','-count=1','-json'],work,env,out/'go-profile-tests.jsonl')
    logs=lambda path:[json.loads(line) for line in path.read_bytes().splitlines() if line.startswith(b'{')]
    top=lambda rows:{(r['Package'],r['Test']) for r in rows if r.get('Action')=='pass' and r.get('Test') and '/' not in r['Test']}
    previous=logs(parent/'go-profile-tests.jsonl');current=logs(out/'go-profile-tests.jsonl')
    names={'TestJavaV1MessageProfilePreservesSignedTimeout','TestJavaV1MessageProfilePreservesOptionalMetadata','TestJavaV1MergeProfileRejectsEveryTruncationAndBadShape',
        'TestJavaV1MergeProfileSymmetricDepthAndNodeLimits','TestJavaV1MergeProfileRejectsLossyInputsAndUnknownObjects','TestJavaV1MergeProfileMutationsRejectOrReencodeExactly'}
    if top(current)!=top(previous)|{('seata.apache.org/seata-go/v2/pkg/protocol/codec',name) for name in names} or any(r.get('Action')=='fail' for r in current):
        raise RuntimeError('Merged-payload unit identities differ')
    if len(top(current))!=110:raise RuntimeError('Go110 inventory differs')
    for row in manifest['files']:
        if h.sha(work/row['path'])!=row['patched_sha256']:raise RuntimeError('Executed merge source drifted')
    if cohort_test.read_bytes()!=copied:raise RuntimeError('Executed merge test drifted')
    candidate_audit=audit_candidate(work)
    for source,data in frozen.items():
        if source.read_bytes()!=data:raise RuntimeError('Merge validation ROOT input drifted')
    for row in json.loads((child/'source-input-audit.json').read_bytes()):
        if h.sha(ROOT/row['path'])!=row['sha256']:raise RuntimeError('Inherited frozen input drifted')
    c.audit_go()
    h.write(out/'go-source-audit.json',json.dumps(dict(profile_files=manifest['files'],candidate=candidate_audit,test_sha256=h.sha(test),native_registry='Unchanged59/60 still absent; explicit profile only'),indent=2)+'\n')
    summary=dict(succeeded=True,executed_at=datetime.datetime.now().astimezone().isoformat(),java_selected_tests=475,rust_tests=117,go_top_level_tests=110,
        new_java_cases=38,original_merge_cases=6,profile_merge_cases=44,ordinary_payload_regressions=29,ordinary_type_count=25,merged_exception_cases=250,
        signed_timeout_cases=4,absent_registration_versions=2,nullable_report_cases=2,parent_evidence=parent.relative_to(ROOT).as_posix(),parent_summary_sha256=h.sha(parent/'summary.json'),
        java_rust_evidence=child.relative_to(ROOT).as_posix(),java_pin=h.PIN,go_pin='82570d905919c1c66bb025a3a5979f25390de498',length_patch_sha256=h.sha(l.PATCH),
        exception_patch_sha256=h.sha(e.PATCH),merge_patch_sha256=h.sha(PATCH),original_jar_sha256=h.sha(c.ORACLE/'server/target/seata-server.jar'),cohort=str(work),
        scope='Explicit merged-payload candidate incl signed/optional semantics. Native Go registry/default Getty unchanged; required transport stage, full clients, WP00 and production Rust remain open.')
    h.write(out/'summary.json',json.dumps(summary,indent=2)+'\n')
    (out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8')
    print(json.dumps(summary))
    return out,summary


if __name__=='__main__':run()
