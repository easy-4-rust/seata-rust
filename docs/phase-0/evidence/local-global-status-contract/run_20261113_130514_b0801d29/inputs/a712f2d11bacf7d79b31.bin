"""Fresh Java matrix plus explicit Go exception profile; original references stay intact."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import uuid

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('exception_length_support',ROOT/'tools/go-java-v1-length-patch/run.py')
l=importlib.util.module_from_spec(spec);spec.loader.exec_module(l)
h=l.h;c=l.c
HERE=Path(__file__).resolve().parent
PATCH=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498/0002-java-v1-response-exception-profile.patch'
MANIFEST=PATCH.with_name('exception_profile_manifest.json')
KINDS=[2,4,6,8,10,12,14,16,18,22]
NAMES=['Unknown','BeginFailed','LockKeyConflict','IO','BranchRollbackFailed_Retriable','BranchRollbackFailed_Unretriable',
       'BranchRegisterFailed','BranchReportFailed','LockableCheckFailed','BranchTransactionNotExist','GlobalTransactionNotExist',
       'GlobalTransactionNotActive','GlobalTransactionStatusInvalid','FailedToSendBranchCommitRequest','FailedToSendBranchRollbackRequest',
       'FailedToAddBranch','FailedLockGlobalTransaction','FailedWriteSession','FailedStore','NotRaftLeader','LockKeyConflictFailFast',
       'TransactionTimeout','CommitHeuristic','Broken','BranchPrepareFailed']
EXPECTED={f'{kind}_{name}' for kind in KINDS for name in NAMES}


def read_rows(log):
    rows=[json.loads(line.split(b'SEATA_EXCEPTION_PROFILE ',1)[1].decode('ascii')) for line in log.read_bytes().splitlines() if line.startswith(b'SEATA_EXCEPTION_PROFILE ')]
    if len(rows)!=250 or len({r['name'] for r in rows})!=250 or {r['name'] for r in rows}!=EXPECTED:
        raise RuntimeError('Need all250 unique exception cases')
    for row in rows:
        code=row['error_code'];kind=row['type_id']
        if kind not in KINDS or not isinstance(code,int) or code<0 or code>=25 or row['error_name']!=NAMES[code] or row['name']!=f'{kind}_{NAMES[code]}':
            raise RuntimeError('Exception case identity/meaning differs')
        data=bytes.fromhex(row['serialized_hex'])
        if len(data)<6 or struct.unpack('>H',data[:2])[0]!=kind or data[2]!=0:
            raise RuntimeError('Wrong exception result envelope')
        count=struct.unpack('>h',data[3:5])[0]
        if count<0 or 5+count>=len(data) or data[5:5+count].decode('utf-8')!='wire:'+NAMES[code] or data[5+count]!=code:
            raise RuntimeError('Actual Java error/text bytes differ')
    return rows


def export(rows,path):
    path.mkdir(parents=True,exist_ok=False)
    h.write(path/'vectors.json',json.dumps(rows,indent=2)+'\n')
    h.write(path/'vectors.tsv',''.join(row['name']+'\t'+row['serialized_hex']+'\n' for row in rows))
    for row in rows:h.write(path/(row['name']+'.payload.bin'),bytes.fromhex(row['serialized_hex']))
    h.write(path/'manifest.json',json.dumps(dict(java_pin=h.PIN,go_pin='82570d905919c1c66bb025a3a5979f25390de498',
        profile='Java authority; Go requires explicit0001+0002 candidate and opt-in response API',cases=250,
        files=[dict(path=p.name,sha256=h.sha(p)) for p in sorted(path.iterdir())]),indent=2)+'\n')


def results(log,expected_bad):
    rows=[json.loads(line.split(b'SEATA_EXCEPTION_RESULT ',1)[1]) for line in log.read_bytes().splitlines() if line.startswith(b'SEATA_EXCEPTION_RESULT ')]
    if len(rows)!=250 or len({r['name'] for r in rows})!=250 or {r['name'] for r in rows}!=EXPECTED:
        raise RuntimeError('Go exception result inventory differs')
    bad={r['name'] for r in rows if not all(r[k] for k in ['encode_match','decode_match','reencode_match'])}
    if bad!=expected_bad:raise RuntimeError('Go exception mismatches differ from exact expected identities')
    if not expected_bad and not all(r.get('pointer_checked') is True and r.get('pointer_match') is True for r in rows):
        raise RuntimeError('Profile pointer/caller-preservation checks missing')
    return rows


def run(extra_capture=None, rust_extra_env=None, extra_sources=()):
    if ROOT.drive.upper() not in ['D:','E:']:raise RuntimeError('Development restricted to D:/E:')
    manifest=json.loads(MANIFEST.read_bytes());patch_bytes=PATCH.read_bytes()
    if h.sha(PATCH)!=manifest['patch_sha256'] or h.sha(l.PATCH)!=manifest['requires_length_patch_sha256']:
        raise RuntimeError('Profile/length patch identity differs')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-0/evidence/local-go-java-exception-profile'/('run_'+stamp);out.mkdir(parents=True,exist_ok=False)
    capture=out/'capture'
    extra=list(HERE.glob('*.go'))+[Path(__file__),PATCH,MANIFEST,ROOT/'tools/go-failure-variants/expected.json']
    extra+=list(extra_sources)
    frozen={source:source.read_bytes() for source in extra}
    original_before=c.audit_go()
    def capture_matrix(log,parent):
        rows=read_rows(log);export(rows,capture)
        fixtures=ROOT/'tests/fixtures/protocol/v1/exception_profile'
        if not fixtures.exists():export(rows,fixtures)
        else:
            for path in capture.iterdir():
                if path.read_bytes()!=(fixtures/path.name).read_bytes():raise RuntimeError('Refusing exception fixture overwrite: '+path.name)
        h.write(out/'java-capture-source.json',json.dumps(dict(parent_evidence=parent.relative_to(ROOT).as_posix(),java_log_sha256=h.sha(log),cases=250),indent=2)+'\n')
        if extra_capture is not None:extra_capture(log,parent)
    parent,base=l.run(extra_capture=capture_matrix,rust_extra_env={'SEATA_IT_EXCEPTION_PROFILE_DIR':str(capture),**(rust_extra_env or {})},extra_sources=extra)
    if base['java_selected_tests']!=sum(l.SUITES.values()) or base['rust_tests']!=117:raise RuntimeError('Combined Java/Rust inventory differs')
    work=Path(base['cohort'])
    native_files=[HERE/'common.go',HERE/'native_main.go',capture/'vectors.json']
    raw_exit=l.run_command(['go','run',*native_files],c.GO_ROOT,c.GO_ENV,out/'go-original-red.log',expected=None)
    raw_bad={f'{kind}_{NAMES[code]}' for kind in KINDS for code in range(25) if kind in [14,22] or code>=19}
    if raw_exit!=1:raise RuntimeError('Original Go RED did not fail')
    results(out/'go-original-red.log',raw_bad)
    length_exit=l.run_command(['go','run',*native_files],work,c.GO_ENV,out/'go-length-only-red.log',expected=None)
    length_bad={f'{kind}_{NAMES[code]}' for kind in KINDS for code in range(19,25)}
    if length_exit!=1:raise RuntimeError('Length-only Go RED did not fail')
    results(out/'go-length-only-red.log',length_bad)
    h.write(out/'candidate.patch',patch_bytes);h.write(out/'patch-manifest.json',MANIFEST.read_bytes())
    for row in manifest['files']:
        if (work/row['path']).exists():raise RuntimeError('Profile candidate path already exists in fresh cohort')
    git=['git','-c','core.autocrlf=false','-c','core.safecrlf=false']
    l.run_command(git+['apply','--check',out/'candidate.patch'],work,h.ENV,out/'patch-check.log')
    l.run_command(git+['apply',out/'candidate.patch'],work,h.ENV,out/'patch-apply.log')
    profile_sources=[]
    for row in manifest['files']:
        if h.sha(work/row['path'])!=row['patched_sha256']:raise RuntimeError('Profile applied source hash differs')
        profile_sources.append(dict(path=row['path'],sha256=h.sha(work/row['path'])))
    l.run_command(['go','run',HERE/'common.go',HERE/'profile_main.go',capture/'vectors.json'],work,c.GO_ENV,out/'go-profile-green.log')
    results(out/'go-profile-green.log',set())
    test_source=HERE/'java_v1_response_profile_test.go';test_bytes=frozen[test_source]
    cohort_test=work/'pkg/protocol/codec/java_v1_response_profile_test.go';cohort_test.write_bytes(test_bytes)
    h.write(out/'go-profile-test-executed.go',test_bytes)
    env=c.GO_ENV.copy();env['SEATA_EXCEPTION_VECTORS']=str(capture/'vectors.json');env['SEATA_JAVA_LENGTH_VECTORS']=str(parent/'capture/vectors.json')
    l.run_command(['go','test','./pkg/protocol/codec','./pkg/util/bytes','./pkg/util/errors','-count=1','-json'],work,env,out/'go-profile-tests.jsonl')
    unit=[json.loads(line) for line in (out/'go-profile-tests.jsonl').read_bytes().splitlines() if line.startswith(b'{')]
    parent_unit=[json.loads(line) for line in (parent/'go-patched-tests.jsonl').read_bytes().splitlines() if line.startswith(b'{')]
    old_names={(r['Package'],r['Test']) for r in parent_unit if r.get('Action')=='pass' and r.get('Test') and '/' not in r['Test']}
    actual_names={(r['Package'],r['Test']) for r in unit if r.get('Action')=='pass' and r.get('Test') and '/' not in r['Test']}
    new_names={'TestJavaV1ExceptionProfileKeepsOriginalDomainAndRegistry','TestJavaV1ExceptionProfileRejectsGoOnlyErrors',
        'TestJavaV1ExceptionProfileRejectsMalformed250CaseMatrix','TestJavaV1ExceptionProfileValidatesInputsAndDoesNotTruncate',
        'TestJavaV1ExceptionProfileSuccessAndOptionalText'}
    package='seata.apache.org/seata-go/v2/pkg/protocol/codec'
    if actual_names!=old_names|{(package,name) for name in new_names} or any(r.get('Action')=='fail' for r in unit):
        raise RuntimeError('Combined Go unit identity set differs')
    l.run_command(['go','run',HERE/'corpus_main.go',ROOT/'tools/go-golden-vectors/expected.json',ROOT/'tools/go-failure-variants/expected.json',parent/'capture/vectors.json'],work,c.GO_ENV,out/'go-profile-corpus.log')
    if b'PROFILE CORPUS PASS: original14 response bytes,15 non-response rejections,all7 Java failure semantics,24 length wire roundtrips' not in (out/'go-profile-corpus.log').read_bytes():
        raise RuntimeError('Profile original/failure/length regressions missing')
    l.run_command(['go','run','.'],work/'tools/seata-rust-golden',c.GO_ENV,out/'go-default-original29-sixframes.log')
    for row in profile_sources:
        if h.sha(work/row['path'])!=row['sha256']:raise RuntimeError('Executed profile source changed')
    if cohort_test.read_bytes()!=test_bytes:raise RuntimeError('Executed profile test changed')
    length_manifest=json.loads((l.PATCH_DIR/'manifest.json').read_bytes())
    changes={row['path']:row['patched_sha256'] for row in length_manifest['files']}
    cohort_audit=l.audit_cohort(work,changes,(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
    original_after=c.audit_go()
    if original_before!=original_after:raise RuntimeError('Original Go source drifted')
    for source,data in frozen.items():
        if source.read_bytes()!=data:raise RuntimeError('Profile ROOT source changed during execution')
    for row in json.loads((parent/'source-input-audit.json').read_bytes()):
        if h.sha(ROOT/row['path'])!=row['sha256']:raise RuntimeError('Combined ROOT source changed after base validation')
    h.write(out/'go-source-audit.json',json.dumps(dict(original_before=original_before,original_after=original_after,
        cohort_original_files=cohort_audit,profile_files=profile_sources,test_sha256=hashlib.sha256(test_bytes).hexdigest()),indent=2)+'\n')
    summary=dict(succeeded=True,executed_at=datetime.datetime.now().astimezone().isoformat(),java_selected_tests=base['java_selected_tests'],java_exception_cases=250,
        rust_tests=base['rust_tests'],go_top_level_tests=len(actual_names),original_go_divergences=len(raw_bad),length_only_divergences=len(length_bad),
        profile_compatible_cases=250,profile_original_response_regressions=14,profile_non_response_rejections=15,profile_failure_regressions=7,
        profile_length_wire_regressions=24,parent_evidence=parent.relative_to(ROOT).as_posix(),parent_summary_sha256=h.sha(parent/'summary.json'),
        java_pin=h.PIN,go_pin='82570d905919c1c66bb025a3a5979f25390de498',length_patch_sha256=h.sha(l.PATCH),profile_patch_sha256=h.sha(PATCH),
        original_jar_sha256=h.sha(c.ORACLE/'server/target/seata-server.jar'),cohort=str(work),
        scope='Explicit opt-in ten-response profile; original Go enums/default API unchanged and incompatible. Not transport integration, all-peer conformance, WP00 exit or production Rust.')
    h.write(out/'summary.json',json.dumps(summary,indent=2)+'\n')
    (out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8')
    print(json.dumps(summary))
    return out,summary


if __name__=='__main__':run()
