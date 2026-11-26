from pathlib import Path
import hashlib,importlib.util,json,os,socket,subprocess,zipfile
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-java-raft-context-candidate';OUT=BASE/'run_20261113_064040_f5c35163';HERE=ROOT/'tools/go-java-v1-raft-context-update';UNIT=Path('D:/codex-builds/raft-context-unit-v4-20261113')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def put(p,v):
 p.parent.mkdir(parents=True,exist_ok=True);v=v.encode() if isinstance(v,str) else v
 if p.exists():assert p.read_bytes()==v,('Refusing overwrite',p)
 else:
  with p.open('xb') as stream:stream.write(v)
def save(p,v):put(p,json.dumps(v,indent=2)+'\n')
def module(n,p):
 spec=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
s=json.loads((OUT/'summary.json').read_bytes());r=json.loads((OUT/'result.json').read_bytes());assert s['succeeded'] and r['succeeded'] and r['container_state']=='exited' and r['ports_released']
rows=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(rows)==s['frozen_inputs']==1039
for row in rows:assert sha(Path(row['path']))==row['sha256']==sha(OUT/row['snapshot'])
e=module('context_candidate_archive_evidence',HERE/'evidence.py');proof=e.derive(OUT);assert json.loads(json.dumps(proof))==json.loads((OUT/'proof.json').read_bytes())==r['proof']
runner=module('context_candidate_archive_runner',HERE/'run_candidate.py');a=module('context_candidate_archive_go',ROOT/'tools/go-java-v1-inflight/audit.py');h=a.sdk.h;deps=module('context_candidate_archive_deps',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
assert a.run(Path(s['cohort']))==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go();jar=a.sdk.c.ORACLE/'server/target/seata-server.jar';assert h.sha(jar)==s['java_jar_sha256']==h.JAR_HASH;assert h.sha(h.DRIVER)==s['driver_sha256']
reference=Path(s['reference_parent']);check_ref=runner.catalog(reference,'09170f1593070d895cca9122397252cc80319a07d920a03285313847e6ef7296')
for path in [reference/'summary.json',reference/'proof.json',reference/'real-commit.exe',reference/'independent-revalidation.json']:check_ref(path)
assert json.loads((reference/'proof.json').read_bytes())['expected_defect_reproduced'] and json.loads((reference/'summary.json').read_bytes())['recovery_certified'] is False
assert sha(OUT/'real-commit.exe')==s['binary_sha256']==sha(reference/'real-commit.exe')
inventory=deps.dependency_inventory((OUT/'effective-classpath.txt').read_text().strip());assert inventory==json.loads((OUT/'compile-dependency-audit.json').read_bytes())
for folder,count in [('overlay',24),('compiled',3)]:
 actual=[dict(path=p.relative_to(OUT/folder).as_posix(),sha256=sha(p)) for p in sorted((OUT/folder).rglob('*.class'))];assert len(actual)==count and actual==json.loads((OUT/(folder+'-manifest.json')).read_bytes())
parent=Path(s['raft_parent']);check=runner.catalog(parent,'e2a284c1cdf7f2b09821ec2daf363730ce9b115f445a907303760d80a5cb79a2')
for row in json.loads((parent/'overlay-manifest.json').read_bytes()):
 if 'RaftReloadObservationControl' in row['path']:continue
 check(parent/'overlay'/row['path']);assert sha(OUT/'overlay'/row['path'])==row['sha256']
for row in json.loads((UNIT/'candidate-class-audit.json').read_bytes()):assert sha(UNIT/'candidate'/row['path'])==row['sha256']==sha(OUT/'overlay'/row['path'])
unit_deps=deps.dependency_inventory((UNIT/'effective-classpath.txt').read_text().strip())
for filename in ['dependency-before.json','red-dependency-after.json','green-dependency-after.json']:assert unit_deps==json.loads((UNIT/filename).read_bytes())
assert len(unit_deps)==278 and sum(1 if row['kind']=='file' else len(row['files']) for row in unit_deps)==1950
for row in json.loads((UNIT/'candidate-source-audit.json').read_bytes()):assert sha(Path(row['path']))==row['sha256']==sha(Path(row['snapshot']))
for row in json.loads((UNIT/'test-binary-audit.json').read_bytes()):assert sha(UNIT/'test-classes'/row['path'])==row['sha256']
with zipfile.ZipFile(jar) as archive:
 for row in json.loads((UNIT/'original-class-audit.json').read_bytes()):assert hashlib.sha256(archive.read('BOOT-INF/classes/'+row['path'])).hexdigest()==row['sha256']==sha(UNIT/'original-jar'/row['path'])
assert 'OBSERVATION_JUNIT tests=25 succeeded=14 failed=11 skipped=0' in (UNIT/'red.log').read_text();assert 'OBSERVATION_JUNIT tests=25 succeeded=25 failed=0 skipped=0' in (UNIT/'green.log').read_text()
unit_archive=[]
for path in sorted(UNIT.rglob('*')):
 if path.is_file():
  leaf='java-unit-evidence/'+hashlib.sha256(str(path).encode()).hexdigest()[:20]+'.bin';put(OUT/leaf,path.read_bytes());unit_archive.append(dict(path=str(path),snapshot=leaf,sha256=sha(path)))
save(OUT/'java-unit-evidence-inputs.json',unit_archive)
assert (UNIT/'legacy-status-only.bin').stat().st_size==705
assert len(proof['actual_class_origins'])==20 and len(proof['updated_context']['actual_context_class_origins'])==15 and len(proof['actual_snapshot_loads'])==2 and len(proof['snapshot_metadata'])==6
assert e.parse_metadata((OUT/'snapshot-metadata-reader-output.log').read_bytes())==proof['snapshot_metadata'];assert all(json.loads((OUT/'final-input-check.json').read_bytes()).values())
runtime=json.loads((OUT/'runtime.json').read_bytes());assert len(set(runtime['all_tc_pids']+runtime['sdk_pids']))==8 and len(set(runtime['database_starts']))==3
item=json.loads(h.command(['docker','inspect',r['container']]).stdout)[0];assert item['State']['Status']=='exited' and item['Image']==h.IMAGES['db'] and item['Config']['Labels']['seata-raft-context']==Path(r['runtime']).name
assert next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')['Source'].replace('\\','/').lower()==(Path(r['runtime'])/'mysql').as_posix().lower()
for port in runner.PORTS:
 try:
  with socket.create_connection((runner.HOST if port in runner.RAFT else '127.0.0.1',port),timeout=.2):raise AssertionError('Owned port listening')
 except OSError:pass
ids=','.join(map(str,runtime['all_tc_pids']+runtime['sdk_pids']));command='$ownedIds=@('+ids+'); @(Get-CimInstance Win32_Process | Where-Object { $ownedIds -contains $_.ProcessId } | Select-Object ProcessId,CommandLine) | ConvertTo-Json -Compress';raw=h.command(['powershell','-NoProfile','-Command',command]).stdout.strip();present=json.loads(raw) if raw else [];present=[present] if isinstance(present,dict) else present;assert not any(Path(r['runtime']).name in str(row.get('CommandLine')) for row in present)
save(OUT/'cleanup-verification.json',dict(container=item['Id'],state='exited',image=item['Image'],ports_released=runner.PORTS,owned_tc_pids=runtime['all_tc_pids'],owned_sdk_pids=runtime['sdk_pids'],owned_processes_stopped=True,reused_pid_observations=present))
post=[Path(__file__),Path('D:/codex-builds/test_raft_updated_context_actual_20261113.py'),Path('D:/codex-builds/replay_raft_context_guard_pair_20261113.py')]+[p for p in Path('D:/codex-builds/raft-context-python-guard-pair-20261113').iterdir() if p.is_file()];audits=[]
for path in post:
 leaf='post-run-validation/'+path.name;put(OUT/leaf,path.read_bytes());audits.append(dict(path=str(path),snapshot=leaf,sha256=sha(path)))
save(OUT/'post-run-validation-inputs.json',audits);env=h.ENV.copy();env['SEATA_UPDATED_CONTEXT_ACTUAL']=str(OUT);test=subprocess.run(['python','D:/codex-builds/test_raft_updated_context_actual_20261113.py','-v'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=30,creationflags=h.FLAGS);assert test.returncode==0 and b'Ran 10 tests' in test.stdout
if not (OUT/'context-actual-tests.log').exists():put(OUT/'context-actual-tests.log',test.stdout)
assert b'Ran 102 tests' in (OUT/'python-tests.log').read_bytes() and (OUT/'python-tests.log').read_bytes().rstrip().endswith(b'OK')
pair=json.loads(Path('D:/codex-builds/raft-context-python-guard-pair-20261113/result.json').read_bytes());assert pair['same_test_bytes'] and pair['results']['red']['failures']==14 and pair['results']['green']['failures']==0
assert pair['results']['green']['test_source_sha256']==sha(HERE/'test_updated_context.py')==pair['results']['red']['test_source_sha256']
save(OUT/'additional-context-certification.json',dict(succeeded=True,additional_to_frozen_runtime=True,validator='post-run-validation/test_raft_updated_context_actual_20261113.py',actual_evidence_mutation_tests=10,paired_normalized_tests=15,paired_normalized_red_failures=14,paired_normalized_red_controls=1,counts_already_include_15_in_102_runtime_tests=True,requires_runtime_and_this_additional_gate=True))
save(OUT/'java-unit-certification.json',dict(succeeded=True,actual_new_java_tests=25,target_tests=21,original_regression_tests=4,red_failures=11,red_controls=14,green_passes=25,same_test_binary_bytes=True,actual_legacy_blob_bytes=705,legacy_blob_produced_by_original_classes=True,runtime_three_candidate_classes_equal_tested_bytes=True,compile_classpath_entries=278,compile_dependency_files=1950))
save(OUT/'independent-revalidation.json',dict(succeeded=True,source_check_epoch='Before final progress documentation update; runtime source snapshots remain authoritative',frozen_runtime_inputs=1039,post_run_validation_inputs=len(audits),runtime_python_tests=102,additional_actual_context_python_tests=10,total_new_python_tests=112,actual_new_java_tests=25,java_red_failures=11,java_red_controls=14,production_overlay_classes=23,readonly_control_classes=1,diagnostic_classes=3,compile_classpath_entries=len(inventory),compile_dependency_files=sum(1 if row['kind']=='file' else len(row['files']) for row in inventory),actual_class_origins=35,actual_tc_jvms=5,actual_sdk_processes=3,actual_elections=2,snapshot_requests=6,actual_snapshot_loads=2,jfr_recordings_reparsed=2,native_queries=5,native_frames=30,reference_defect_reproduced=True,changed_prepare_context_recovery_certified=True,uniform_patch_nodes_only=True,mixed_version_upgrade_certified=False,historic_lost_data_reconstruction_certified=False,go_unit_tests_inherited=254,rust_tests_inherited=117,new_go_unit_execution=False,new_rust_unit_execution=False,original_reference_defects='OPEN',election_1000ms_issue='OPEN',resources_stopped=True,ports_released=11))
print(json.dumps(dict(succeeded=True,primary=OUT.name,frozen_inputs=1039,new_python_tests=112,new_java_tests=25,post_run_validation_inputs=len(audits))))
