"""Fresh pinned source, exact native RED, isolated Java candidate and full affected regressions."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import uuid
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
sdk=module('preface_sdk_support',ROOT/'tools/go-java-v1-sdk-profile/run.py');prepare=module('preface_prepare_support',HERE/'prepare_candidate.py');runtime=module('preface_runtime_support',HERE/'runtime.py')
h=sdk.h;c=sdk.c;l=sdk.l
PATCH=prepare.DEST/'0001-incremental-protocol-preface.patch';MANIFEST=prepare.DEST/'manifest.json'
TEST='ProtocolPrefaceFragmentationTest'
CORE=['ProtocolConstantsTest','ProtocolDecoderV1Test','ProtocolRpcMessageV0Test','ProtocolEncoderV0Test','ProtocolDecoderV0Test','ProtocolConstantsV0Test','MultiProtocolDecoderTest','HttpDetectorTest','Http2DetectorTest']
SELECTED=set(c.TESTS)|set(CORE)|{TEST}

def norm(data):return data.replace(b'\r\n',b'\n')
def digest(data):return hashlib.sha256(data).hexdigest()
def snapshot(out,paths):
    rows=[];directory=out/'source-inputs';directory.mkdir()
    for path in sorted(set(paths)):
        data=path.read_bytes();name=hashlib.sha256(path.relative_to(ROOT).as_posix().encode()).hexdigest()[:20]+'.bin';h.write(directory/name,data)
        rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=digest(data)))
    h.write(out/'source-input-audit.json',json.dumps(rows,indent=2)+'\n');return {path:path.read_bytes() for path in set(paths)}

def export_original(build,out):
    archive=out/'java-pinned-source.tar'
    with archive.open('xb') as stream:
        result=subprocess.run(['git','-C',str(c.ORACLE),'archive','--format=tar',h.PIN],env=h.ENV,stdout=stream,stderr=subprocess.PIPE,creationflags=h.FLAGS,timeout=60)
    if result.returncode:raise RuntimeError('Pinned Java archive failed')
    build.mkdir(exist_ok=False)
    with tarfile.open(archive) as source:
        for entry in source:
            if not entry.isfile():continue
            target=build/entry.name
            if not target.resolve().is_relative_to(build.resolve()):raise RuntimeError('Archive escape')
            target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(source.extractfile(entry).read())
    original=h.audit_build(c.ORACLE,build)
    if len(original)!=2122:raise RuntimeError('Original Java production source inventory differs')
    return original

def audit_candidate(build,original,manifest):
    wanted={row['path']:row['patched_lf_sha256'] for row in manifest['files']};rows=[]
    for row in original:
        path=build/row['path'];actual=digest(norm(path.read_bytes()));expected=wanted.get(row['path'],row['lf_sha256'])
        if actual!=expected:raise RuntimeError('Java candidate unexpected main/POM/resource drift: '+row['path'])
        rows.append(dict(path=row['path'],original_lf_sha256=row['lf_sha256'],actual_lf_sha256=actual,patched=row['path'] in wanted))
    expected_paths={r['path'] for r in original}|set(wanted)
    actual_paths={p.relative_to(build).as_posix() for p in build.rglob('*') if p.is_file() and 'target' not in p.relative_to(build).parts and ('/src/main/' in '/'+p.relative_to(build).as_posix() or p.name=='pom.xml')}
    if actual_paths!=expected_paths:raise RuntimeError('Java main/resource/POM path closure differs')
    for path,data in prepare.files().items():
        if norm((build/path).read_bytes())!=data:raise RuntimeError('Editable Java candidate differs from executed source: '+path)
    # 原公共接口必须保持相同，不能偷偷通过修改既有接口完成分片修复。
    unchanged=manifest['existing_interface_unchanged'];before=next(row['lf_sha256'] for row in original if row['path']==unchanged)
    if digest(norm((build/unchanged).read_bytes()))!=before:raise RuntimeError('Original ProtocolDetector interface changed')
    return rows

def copy_overlays(build,out,frozen):
    rows=[]
    for source in sorted((ROOT/'tools/java-v1-golden').glob('*.java')):
        matches=list((c.BUILD/'test-suite/test-new-version/src/test/java').rglob(source.name))
        if len(matches)!=1:raise RuntimeError('Established Java overlay destination ambiguous: '+source.name)
        relative=matches[0].relative_to(c.BUILD);target=build/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(frozen[source]);rows.append((source,target,relative.as_posix()))
    target=build/'core/src/test/java/org/apache/seata/core/rpc/netty'/ (TEST+'.java');target.parent.mkdir(parents=True,exist_ok=True);source=HERE/(TEST+'.java');target.write_bytes(frozen[source]);rows.append((source,target,target.relative_to(build).as_posix()))
    resources=[]
    for source_name,target_name in [('tools/go-golden-vectors/expected.json','seata-rust-payload-vectors.json'),('tools/go-failure-variants/expected.json','seata-rust-failure-variants.json')]:
        source=ROOT/source_name;target=build/'test-suite/test-new-version/src/test/resources'/target_name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(frozen[source]);resources.append(dict(source=source_name,target=target.relative_to(build).as_posix(),sha256=digest(frozen[source])))
    h.write(out/'resource-audit.json',json.dumps(resources,indent=2)+'\n');return rows

def maven(build,out,label,tests=None):
    # 只删除本次独立构建中的选定报告，防止旧XML充作新执行证据。
    for path in build.rglob('TEST-*.xml'):
        if path.parent.name=='surefire-reports' and any(path.name.endswith('.'+name+'.xml') for name in SELECTED):
            if not path.resolve().is_relative_to(build.resolve()):raise RuntimeError('Report cleanup escape')
            path.unlink()
    return l.run_command([h.MAVEN,'-B','-ntp','-s',h.SETTINGS,'-f',build/'pom.xml','-pl','test-suite/test-new-version','-am','-Dtest='+','.join(sorted(SELECTED if tests is None else tests)),'-Dsurefire.failIfNoSpecifiedTests=false','-Djava.io.tmpdir=D:/Temp','test'],build,h.ENV,out/(label+'.log'),timeout=420,expected=None)

def reports(build,out,label,authority=True,include_new=True):
    directory=out/label;directory.mkdir();rows=[]
    for path in sorted(build.rglob('TEST-*.xml')):
        if path.parent.name!='surefire-reports':continue
        suite=ET.parse(path).getroot();simple=suite.get('name','').rsplit('.',1)[-1]
        if simple not in SELECTED:continue
        relative=path.relative_to(build).as_posix();name=hashlib.sha256(relative.encode()).hexdigest()[:20]+'.xml';h.write(directory/name,path.read_bytes())
        cases=[dict(name=node.get('name'),classname=node.get('classname'),status='error' if node.find('error') is not None else 'failure' if node.find('failure') is not None else 'skipped' if node.find('skipped') is not None else 'pass') for node in suite.findall('testcase')]
        rows.append(dict(module=path.parent.parent.parent.relative_to(build).as_posix(),suite=suite.get('name'),simple=simple,tests=int(suite.get('tests','0')),failures=int(suite.get('failures','0')),errors=int(suite.get('errors','0')),skipped=int(suite.get('skipped','0')),snapshot=label+'/'+name,cases=cases))
    for name,count in (c.TESTS.items() if authority else []):
        matches=[r for r in rows if r['simple']==name]
        if len(matches)!=1 or matches[0]['tests']!=count:raise RuntimeError('Java authority report identity/count differs: '+name)
    if {r['simple'] for r in rows if r['module']=='core'}!=set(CORE)|({TEST} if include_new else set()):raise RuntimeError('Core affected protocol suite set differs')
    h.write(out/(label+'.json'),json.dumps(rows,indent=2)+'\n');return rows

def run():
    if h.command(['git','-C',c.ORACLE,'rev-parse','HEAD']).stdout.decode().strip()!=h.PIN or h.sha(c.ORACLE/'server/target/seata-server.jar')!=h.JAR_HASH:raise RuntimeError('Original Java reference identity differs')
    manifest=json.loads(MANIFEST.read_bytes())
    if h.sha(PATCH)!=manifest['patch_sha256']:raise RuntimeError('Java patch differs')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-0/evidence/local-java-preface-candidate'/('run_'+stamp);out.mkdir(parents=True,exist_ok=False)
    sources=list(HERE.glob('*.java'))+list(HERE.glob('*.py'))+[HERE/'core_pom.xml',PATCH,MANIFEST,ROOT/'docs/phase-0/java-preface-candidate-plan.md',ROOT/'tools/java-tc-integration/config/application.yml']
    for directory in ['go-java-v1-sdk-profile','go-java-v1-getty-profile','go-java-v1-merge-profile','go-java-v1-exception-profile','go-java-v1-length-patch','java-control-envelopes','java-tc-integration/backend_recovery','go-golden-vectors','go-failure-variants']:
        for pattern in ['*.go','*.py','*.json']:sources+=list((ROOT/'tools'/directory).glob(pattern))
    sources+=list((ROOT/'tools/java-v1-golden').glob('*.java'))+list((ROOT/'tools/java-tc-integration/rust-client/src').rglob('*.rs'))
    sources+=[ROOT/'tools/java-tc-integration/rust-client'/name for name in ['Cargo.toml','Cargo.lock']]
    sources+=[path for path in (ROOT/'tests/fixtures/protocol/v1').rglob('*') if path.is_file()]
    sources+=list((ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498').glob('*.patch'))+list((ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498').glob('*.json'))
    frozen=snapshot(out,sources);build=Path('D:/codex-builds')/('sjpref_'+stamp);original=export_original(build,out);h.write(out/'original-source-audit.json',json.dumps(original,indent=2)+'\n')
    overlays=copy_overlays(build,out,frozen)
    if maven(build,out,'java-original-baseline',SELECTED-{TEST}):raise RuntimeError('Original existing regression baseline failed')
    baseline=reports(build,out,'baseline-reports',include_new=False)
    if any(r['failures'] or r['errors'] or r['skipped'] for r in baseline):raise RuntimeError('Original baseline contains unsuccessful cases')
    code=maven(build,out,'java-original-red');red=reports(build,out,'red-reports',authority=False);special=next(r for r in red if r['simple']==TEST)
    if code!=1 or (special['tests'],special['failures'],special['errors'],special['skipped'])!=(48,29,0,0):raise RuntimeError('Original48 behavioral RED differs')
    if any(r['failures'] or r['errors'] or r['skipped'] for r in red if r['simple']!=TEST):raise RuntimeError('Existing Java baseline regressions fail; investigate original evidence')
    if len(h.audit_build(c.ORACLE,build))!=2122:raise RuntimeError('Original production source drifted before patch')
    h.write(out/'candidate.patch',frozen[PATCH]);h.write(out/'patch-manifest.json',frozen[MANIFEST])
    for row in manifest['files']:
        if row['original_lf_sha256'] is None:continue
        target=build/row['path'];data=norm(target.read_bytes())
        if digest(data)!=row['original_lf_sha256']:raise RuntimeError('LF-normalized Java patch base differs')
        target.write_bytes(data)
    git=['git','-c','core.autocrlf=false','-c','core.safecrlf=false'];l.run_command(git+['apply','--check',out/'candidate.patch'],build,h.ENV,out/'patch-check.log');l.run_command(git+['apply',out/'candidate.patch'],build,h.ENV,out/'patch-apply.log')
    if maven(build,out,'java-candidate-green'):raise RuntimeError('Fresh Java candidate regression failed')
    green=reports(build,out,'green-reports')
    if any(r['failures'] or r['errors'] or r['skipped'] for r in green):raise RuntimeError('Java green reports contain unsuccessful cases')
    identities=lambda rows:{(r['module'],r['suite'],case['classname'],case['name']) for r in rows for case in r['cases']}
    if identities(red)|identities(baseline)!=identities(green):raise RuntimeError('Java baseline/RED/GREEN testcase identities differ')
    audit=audit_candidate(build,original,manifest);h.write(out/'candidate-source-audit.json',json.dumps(audit,indent=2)+'\n')
    executed=out/'overlay-executed';executed.mkdir();overlay_rows=[]
    for source,target,relative in overlays:
        h.write(executed/source.name,target.read_bytes());same=target.read_bytes()==frozen[source]
        if source.parent==HERE and not same:raise RuntimeError('Reconcile new-test formatter before certification')
        overlay_rows.append(dict(source=source.relative_to(ROOT).as_posix(),target=relative,input_sha256=digest(frozen[source]),executed_sha256=h.sha(target),byte_identical=same))
    h.write(out/'overlay-audit.json',json.dumps(overlay_rows,indent=2)+'\n')
    for row in json.loads((out/'resource-audit.json').read_bytes()):
        if h.sha(build/row['target'])!=row['sha256']:raise RuntimeError('Java test resource drifted')
    log=out/'java-candidate-green.log';capture=out/'capture';capture.mkdir()
    c.collect.export(c.collect.read_rows(log),capture/'control');sdk.m.export(sdk.m.read_rows(log),capture/'merge');sdk.e.export(sdk.e.read_rows(log),capture/'exception');sdk.l.export(sdk.l.read_rows(log),capture/'length')
    env={key:value for key,value in h.ENV.items() if not key.startswith('SEATA_IT_')};env.update(SEATA_IT_CONTROL_WIRE_DIR=str(capture/'control'),SEATA_IT_MERGE_PROFILE_DIR=str(capture/'merge'),SEATA_IT_EXCEPTION_PROFILE_DIR=str(capture/'exception'),SEATA_IT_ERROR_LENGTH_DIR=str(capture/'length'),SEATA_IT_HANDLER_WIRE_DIR=str(ROOT/'tests/fixtures/protocol/v1/handler_matrix'))
    h.write(out/'rust-fixture-environment.json',json.dumps({key:value for key,value in env.items() if key.startswith('SEATA_IT_')},indent=2)+'\n')
    cargo=ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'
    for label,args in [('rust-tests',['cargo','test','--manifest-path',cargo]),('rust-clippy',['cargo','clippy','--manifest-path',cargo,'--all-targets','--','-D','warnings']),('rust-fmt',['cargo','fmt','--manifest-path',cargo,'--','--check'])]:l.run_command(args,ROOT,env,out/(label+'.log'))
    if b'117 passed; 0 failed;' not in (out/'rust-tests.log').read_bytes():raise RuntimeError('Fresh Rust inventory differs')
    latest=json.loads((ROOT/'docs/phase-0/evidence/local-go-java-sdk-profile/latest_run.json').read_bytes());previous=json.loads((Path(latest['evidence_directory'])/'summary.json').read_bytes());go_cohort=Path(previous['cohort']);sdk.audit_candidate(go_cohort)
    frames=sdk.g.compose(json.loads((capture/'control/vectors.json').read_bytes()),json.loads((capture/'merge/vectors.json').read_bytes()),json.loads((capture/'exception/vectors.json').read_bytes()),json.loads(frozen[ROOT/'tools/go-golden-vectors/expected.json']))
    h.write(out/'vectors.json',json.dumps(frames,indent=2)+'\n');go_env=c.GO_ENV.copy();go_env.update(SEATA_GETTY_VECTORS=str(out/'vectors.json'),SEATA_MERGE_VECTORS=str(capture/'merge/vectors.json'),SEATA_EXCEPTION_VECTORS=str(capture/'exception/vectors.json'),SEATA_JAVA_LENGTH_VECTORS=str(capture/'length/vectors.json'))
    l.run_command(['go','test',*sdk.PACKAGES,'-count=1','-json'],go_cohort,go_env,out/'go-tests.jsonl',timeout=240)
    actual=sdk.g.top(out/'go-tests.jsonl');expected=sdk.g.top(Path(latest['evidence_directory'])/'go-after-sdk-tests.jsonl')
    if actual!=expected or len(actual)!=235:raise RuntimeError('Fresh Go235 identity regression differs')
    h.write(out/'go-source-audit.json',json.dumps(sdk.audit_candidate(go_cohort),indent=2)+'\n')
    live=runtime.run(out,build,go_cohort,sdk,prepare)
    l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,h.ENV,out/'python-tests.log')
    audit_candidate(build,original,manifest);c.audit_go()
    for path,data in frozen.items():
        if path.read_bytes()!=data:raise RuntimeError('Frozen ROOT input drifted: '+str(path))
    summary=dict(succeeded=True,executed_at=datetime.datetime.now().astimezone().isoformat(),java_selected_tests=sum(r['tests'] for r in green),java_authority_tests=475,new_java_tests=48,affected_original_tests=sum(r['tests'] for r in green)-523,java_suites=len(green),original_new_failures=29,original_new_controls=19,rust_tests=117,go_tests=235,java_pin=h.PIN,go_pin='82570d905919c1c66bb025a3a5979f25390de498',patch_sha256=h.sha(PATCH),original_jar_sha256=h.sha(c.ORACLE/'server/target/seata-server.jar'),java_build=str(build),go_cohort=str(go_cohort),runtime_profiles=live,scope='Independent V1/HTTP preface Java candidate. Original artifact remains faulty; V0 fragmentation, dedicated V2 proof, full clients/WP00/production Rust remain open.')
    h.write(out/'summary.json',json.dumps(summary,indent=2)+'\n');(out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8');print(json.dumps(summary),flush=True);return out,summary

if __name__=='__main__':run()
