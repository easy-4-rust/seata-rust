"""Freeze the original Java JUnit baseline; all Rust source parity remains missing."""
import copy,datetime,hashlib,importlib.util,json,os,re,subprocess,uuid,urllib.parse,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
PIN='cba73e180fd8896a99df6415b859d275122feaa7'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(v,why):
    if not v:raise ValueError(why)
def module(name,p):
    s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def run(manifest):
    values=read(manifest);build=ROOT/values['build'];runtime=ROOT/values['runtime'];authroot=ROOT/values['authentication'];failed=ROOT/values['failed_guard_diagnostic'];previous=ROOT/values['prior_Rust']
    need(sha(previous/'catalog.json')==values['prior_Rust_catalog_sha256'],'Prior Rust authority differs')
    members={r['path']:r['sha256'] for r in read(previous/'catalog.json')['members']}
    for leaf in ['source-inputs.json','result.json']:need(members[leaf]==sha(previous/leaf),'Consumed prior member differs')
    old_sources=read(previous/'source-inputs.json');rust_paths={Path(r['path']) for r in old_sources if Path(r['path']).is_relative_to(ROOT/'crates') or Path(r['path']) in [ROOT/'Cargo.toml',ROOT/'Cargo.lock']}
    need(rust_paths=={p for p in (ROOT/'crates').rglob('*') if p.is_file()}|{ROOT/'Cargo.toml',ROOT/'Cargo.lock'},'Rust source set changed')
    for row in old_sources:
        if Path(row['path']) in rust_paths:need(sha(Path(row['path']))==row['sha256']==members[row['snapshot']],'Inherited Rust source changed')
    state=read(build/'compile-result.json');need(state['status']=='SOURCE_TEST_COMPILE_PASSED' and state['exit_code']==0 and state['java_pin']==PIN,'Original reactor failed')
    stage=Path(state['work']);source=stage/'source';paths=set(rust_paths)|{Path(__file__),manifest,previous/'catalog.json',previous/'source-inputs.json',previous/'result.json',ROOT/'docs/phase-2/source-junit-contract.md'}
    paths.update(p for p in HERE.rglob('*') if p.is_file())
    for folder in [build,runtime,failed,authroot]:paths.update(p for p in folder.rglob('*') if p.is_file())
    original=read(build/'original-source-inputs.json');need(len(original)==4269,'Original Git source set differs')
    for row in original:
        p=source/row['path'];need(sha(p)==row['sha256'],'Original source changed');paths.add(p)
    command=read(build/'build-command.json');need(command['source_pin']==PIN and sha(stage/'source.zip')==command['archive_sha256'],'Original archive authority differs');paths.update([stage/'source.zip',stage/'settings.xml',stage/'global-settings.xml'])
    need(command.get('literal_Git_blobs_verified') is True and command.get('formatter_apply_disabled_to_preserve_source_bytes') is True,'Literal Git byte export not proved')
    literal=read(build/'literal-git-inputs.json');need(len(literal)==4269 and len({r['path'] for r in literal})==4269,'Literal source set differs')
    oracle=ROOT.parent/'seata-java-phase09-oracle';tree=subprocess.check_output(['git','-C',str(oracle),'ls-tree','-rz',PIN],creationflags=subprocess.CREATE_NO_WINDOW);git={}
    for item in tree.split(b'\0'):
        if not item:continue
        header,name=item.split(b'\t',1);mode,kind,digest=header.decode().split();need(kind=='blob','Unexpected original Git object');git[name.decode('utf-8')]=(mode,digest)
    need(set(git)=={r['path'] for r in literal},'Literal export omitted original Git files')
    with zipfile.ZipFile(stage/'source.zip') as archive:
        need(set(archive.namelist())==set(git),'Literal archive file set differs')
        for row in literal:
            data=(source/row['path']).read_bytes();need((row['git_mode'],row['git_blob'])==git[row['path']] and hashlib.sha1(('blob '+str(len(data))+'\0').encode()+data).hexdigest()==row['git_blob'] and archive.read(row['path'])==data,'Literal Git object bytes or archive differ')
    paths.update(p for p in (stage/'canonical_inventory').rglob('*') if p.is_file())
    paths.update([Path('D:/codex-evidence/seata-rust/source-junit-storage-audit-20261118.json'),Path('D:/codex-evidence/seata-rust/source-junit-storage-before-20261118.json')])
    test_files=[p for folder in [source/'rm-datasource/src/test',source/'sqlparser/seata-sqlparser-core/src/test'] for p in folder.rglob('*.java')];need(len(test_files)==189,'Original test-file denominator differs')
    assets=[]
    for folder in [source/'rm-datasource/src/test',source/'sqlparser/seata-sqlparser-core/src/test']:
        for p in folder.rglob('*'):
            if p.is_file() and p.suffix!='.java':
                relative=p.relative_to(source).as_posix();target=ROOT/'tests/fixtures/mysql/java_git_assets'/relative;need(p.read_bytes()==target.read_bytes(),'Pinned Git source asset changed');paths.add(target);assets.append(dict(source=relative,sha256=sha(p),canonical_copy=str(target),Rust_disposition='SOURCE_ASSET_COPIED_ONLY_NOT_CASE_PARITY'))
    need(len(assets)==7,'Original source asset denominator differs')
    auth=read(authroot/'dependency-authentication.json');need(auth['all_authenticated'] and auth['counts']=={'MATCH':1279},'Dependency input authentication incomplete');repo=Path(auth['repository']);authenticated_jars={}
    need({Path(r['path']) for r in auth['files']}==set(repo.rglob('*.pom'))|set(repo.rglob('*.jar')),'Authenticated Maven file set changed')
    for row in auth['files']:
        p=Path(row['path']);data=p.read_bytes();sidecar=Path(row['checksum_response']);need(sha(p)==row['sha256'] and hashlib.sha1(data).hexdigest()==row['sha1']==row['published_sha1'] and sha(sidecar)==row['checksum_response_sha256'] and re.search(r'(?i)\b[0-9a-f]{40}\b',sidecar.read_text('ascii')).group().lower()==row['published_sha1'],'Official dependency checksum mismatch')
        need(row['checksum_url']=='https://repo.maven.apache.org/maven2/'+p.relative_to(repo).as_posix()+'.sha1','Checksum source URL differs');paths.update([p,sidecar])
        if p.suffix=='.jar':authenticated_jars[row['sha256']]=dict(path=str(p),checksum_url=row['checksum_url'])
    v=module('source_junit_final_events',HERE/'validate.py');process=module('source_junit_final_processes',HERE/'process_proof.py');groups=[];negative=[];proofs=[];pids=[];aliases=[];all_loaded={}
    actual=read(runtime/'result.json');need(actual['mode']=='execute' and actual['source_pin']==PIN and all(r['exit_code']==0 for r in actual['modules']),'Actual original engine failed')
    work=Path(actual['work']);paths.add(work/'hosts')
    for row in read(runtime/'preexisting-runtime-inputs.json')+read(runtime/'helper-artifacts.json'):
        p=Path(row['path']);need(sha(p)==row['sha256'],'Executed runtime input changed');paths.add(p)
        if p.suffix=='.jar' and not p.is_relative_to(repo):need(row['sha256'] in authenticated_jars,'Unverified generated runtime Jar');aliases.append(dict(path=str(p),sha256=row['sha256'],source=authenticated_jars[row['sha256']]))
    agent=read(runtime/'agent-binding.json');need(sha(Path(agent['path']))==agent['sha256']==sha(runtime/'environment-probe.jar') and agent['class_transformation'] is False,'Observation agent differs');paths.add(Path(agent['path']))
    for name in v.MODULES:
        rows=v.load(runtime/(name+'-events.jsonl'));group=v.validate_events(rows,name);groups.append(group);negative.append(dict(module=name,**v.controls(rows,name)))
        props=[process.read(p) for p in (runtime/name/'environments').glob('*.xml')];proof=process.validate(props,name,rows[0],work);proofs.append(dict(module=name,**proof));pids.extend(props)
        if name=='rm-datasource':negative.append(dict(module=name,kind='process',**process.controls(props,rows[0],work)))
        loaded={m[1]:m[2].replace('\\','/') for m in re.finditer(r'\[class,load\] (\S+) source: ([^\r\n]+)',(runtime/(name+'-class-load.log')).read_text('utf-8'))}
        need(loaded.get('org.junit.jupiter.engine.JupiterTestEngine')==rows[0]['jupiter_origin'] and loaded.get('org.junit.platform.launcher.core.LauncherFactory')==rows[0]['launcher_origin'],'Independent real engine class origins differ')
        fullmodule='sqlparser/seata-sqlparser-core' if name=='seata-sqlparser-core' else name
        declared=source/fullmodule/'target/declared-test-classpath.txt';paths.add(declared)
        cp=[work/'classes',source/fullmodule/'target/test-classes',source/fullmodule/'target/classes']+[Path(p) for p in declared.read_text('utf-8').strip().split(os.pathsep)]
        need(rows[0]['classpath']==os.pathsep.join(map(str,cp)),'Actual module classpath differs from original Maven result')
        expected_files={f for p in cp for f in (p.rglob('*') if p.is_dir() else [p]) if f.is_file()}
        recorded={Path(row['path']) for row in read(runtime/'preexisting-runtime-inputs.json')+read(runtime/'helper-artifacts.json')}
        need(expected_files.issubset(recorded),'Actual classpath file omitted from footprint')
        allowed_dirs={p.resolve() for p in cp if p.is_dir()};allowed_jars={p.resolve() for p in cp if p.is_file() and p.suffix=='.jar'}
        def origin_path(value):
            parsed=urllib.parse.unquote(urllib.parse.urlparse(value).path);parsed=parsed[1:] if re.match(r'^/[A-Za-z]:',parsed) else parsed;return Path(parsed).resolve()
        file_owners={m[1] for m in re.finditer(r'\[class,load\] (\S+) source: ([^\r\n]+)',(runtime/(name+'-class-load.log')).read_text('utf-8')) if m[1].startswith('org.apache.seata.') and m[2].startswith('file:') and origin_path(m[2]) in allowed_dirs}
        generated=[]
        seata={k:value for k,value in loaded.items() if k.startswith('org.apache.seata.')};need(seata,'No original Seata classes loaded')
        for classname,value in seata.items():
            if value.startswith('file:'):
                location=origin_path(value)
                if location not in allowed_dirs:
                    owner=None
                    if classname.endswith('ConstructorAccess') and location.name=='reflectasm-1.11.9.jar':owner=classname[:-len('ConstructorAccess')]
                    match=re.fullmatch(r'(.+?)ForyRefCodec(?:_\d+|MetaShared\d+_\d+)',classname)
                    if match and location.name=='fory-core-0.12.3.jar':owner=match.group(1)
                    need(owner in file_owners and location in allowed_jars and sha(location) in authenticated_jars,'Unbound generated class or original Seata fallback outside reactor')
                    generated.append(dict(classname=classname,source_owner=owner,generator=str(location),generator_sha256=sha(location)))
            else:need(value.startswith('__JVM') or value.startswith('__VM') or classname.startswith(value+'$$Lambda$') and value in seata,'Unknown non-file original class source')

        all_loaded[name]=dict(launcher=loaded['org.junit.platform.launcher.core.LauncherFactory'],jupiter=loaded['org.junit.jupiter.engine.JupiterTestEngine'],original_Seata_file_or_generated_classes=seata,verified_generator_derivatives=generated)
    declarations=v.load(build/'canonical-methods.jsonl');methods=v.bind_methods(declarations,groups);method_controls=[]
    for label,changed in [('missing-declaration',declarations[:-1]),('duplicate-declaration',declarations+[declarations[0]])]:
        try:v.bind_methods(changed,groups)
        except ValueError:method_controls.append(label);continue
        raise ValueError('Weakened original declaration floor accepted')
    wrong=copy.deepcopy(groups);wrong[0]['cases'][0]['declaring_class']='unknown.Source'
    try:v.bind_methods(declarations,wrong)
    except ValueError:method_controls.append('foreign-original-method')
    else:raise ValueError('Unbound source method accepted')
    need(not(source/'rm-datasource/db_store').exists(),'Original H2 teardown did not clean owned relative DB directory')
    # Reused PIDs are harmless only when the process command no longer identifies this owned Java job.
    ids=sorted({int(p['pid']) for p in pids});query='$wanted=@('+','.join(map(str,ids))+'); @((Get-CimInstance Win32_Process | Where-Object { $wanted -contains $_.ProcessId }) | ForEach-Object { [pscustomobject]@{pid=$_.ProcessId;command=$_.CommandLine} }) | ConvertTo-Json -Compress'
    live=subprocess.run(['powershell','-NoProfile','-Command',query],capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30);need(live.returncode==0,'Owned Java process audit failed');raw=live.stdout.decode('utf-8-sig').strip();active=json.loads(raw) if raw else [];active=active if isinstance(active,list) else [active]
    need(not any(str(work).replace('\\','/') in (p.get('command') or '').replace('\\','/') for p in active),'Owned JUnit process remains')
    out=ROOT/'docs/phase-2/evidence/source-junit-gate'/('run_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True);print('SOURCE_JUNIT_GATE '+str(out),flush=True);cache={}
    def put(leaf,data):
        target=out/leaf;target.parent.mkdir(parents=True,exist_ok=True);digest=hashlib.sha256(data).hexdigest();existing=cache.get(digest)
        if existing and existing.stat().st_nlink<900:os.link(existing,target)
        else:
            target.write_bytes(data);cache[digest]=target
            if len(data)>=1048576:
                r=subprocess.run(['C:/Windows/System32/compact.exe','/C','/I','/Q',str(target)],capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);need(r.returncode==0 and sha(target)==digest,'Evidence compression changed bytes')
    def save(leaf,value):put(leaf,(json.dumps(value,ensure_ascii=False,indent=2)+'\n').encode('utf-8'))
    frozen=[]
    for p in sorted(paths):
        leaf='sources/'+hashlib.sha256(str(p).encode()).hexdigest()[:24]+'.bin';digest=sha(p);put(leaf,p.read_bytes());frozen.append(dict(path=str(p),snapshot=leaf,sha256=digest))
    snapshot={r['path']:r['snapshot'] for r in frozen}
    for row in methods:row['source_snapshot']=snapshot[row['source']];row['source_git_relative_path']=Path(row['source']).relative_to(source).as_posix()
    save('source-inputs.json',frozen);save('explicit-inputs.json',values);save('source-method-bindings.json',methods);save('concrete-source-cases.json',[c for g in groups for c in g['cases']]);save('source-assets.json',assets);save('negative-controls.json',dict(event_and_process=negative,method_floor=method_controls));save('actual-jvm-processes.json',dict(proofs=proofs,properties=pids,verified_owned_processes_stopped=True,actual_live_pid_snapshot=active,original_H2_directory_cleaned=True));save('class-origin-validation.json',all_loaded);save('generated-runtime-jar-aliases.json',aliases)
    prior=read(previous/'result.json');need(prior['ordinary']['1.99']['passed']==prior['ordinary']['1.85']['passed']==339,'Prior Rust baseline differs')
    save('result.json',dict(status='SELECTED_ORIGINAL_JAVA_JUNIT_BASELINE_VERIFIED_RUST_MIGRATION_OPEN',java_pin=PIN,original_source_files=4269,literal_Git_blob_bytes_verified=True,formatter_apply_skipped_without_test_skips=True,original_test_java_files=189,original_assets=7,declared_test_methods=1135,Java_methods_executed=1096,Java_methods_disabled_JDK17=39,concrete_case_identities=1327,Java_concrete_cases_passed=1288,Java_concrete_cases_failed=0,Java_concrete_cases_disabled=39,actual_Java_JVM_processes=len(pids),actual_pressure_child_JVMs=3,original_pressure_rounds=3,original_pressure_threads_per_child=200,original_pressure_timeout_seconds=30,authenticated_Maven_POMs=876,authenticated_Maven_JARs=403,authenticated_Maven_files=1279,generated_runtime_Jar_aliases=len(aliases),source_inputs=len(frozen),negative_controls=sum(v['count'] for v in negative)+len(method_controls),original_source_and_assertions_changed=False,original_reactor_test_compile_executed=True,original_JUnit_engine_executed=True,original_Surefire_test_goal_executed=False,all_JDK_profiles_executed=False,all_disabled_source_bodies_executed=False,Java_source_baseline_only=True,Rust_source_methods_MISSING=1135,Rust_concrete_cases_MISSING=1327,Rust_differential_cases_MATCH=0,Rust_source_parity_complete=False,Rust_regression='inherited exact unchanged source from prior qualified gate',inherited_Rust_ordinary=prior['ordinary'],prior_Rust_catalog_sha256=values['prior_Rust_catalog_sha256'],TC_safety_gate_passed=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',candidate_derivatives='historical only; authoritative ledgers recomputed from raw final-plan events',initial_guard_diagnostic='224 leaf and 6 container failures were harness JMX context permission setup; original Java source never changed'))
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Final baseline input drift')
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));print(json.dumps(dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run(HERE/'qualification-inputs-20261118.json')
