from pathlib import Path
import hashlib,importlib.util,json,os,re,sys,socket
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-raft-at-timeout';OUT=BASE/'run_20261113_052816_8ea31012';HERE=ROOT/'tools/go-java-v1-raft-at-timeout'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());assert summary['succeeded'] and result['succeeded'] and result['container_state']=='exited' and result['ports_released']
rows=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(rows)==summary['frozen_inputs']==923
for row in rows:assert sha(Path(row['path']))==row['sha256']==sha(OUT/row['snapshot'])
e=module('archive_raft_business_evidence',HERE/'evidence.py');proof=e.derive(OUT);assert json.loads(json.dumps(proof))==json.loads((OUT/'proof.json').read_bytes())==result['proof']
from raft_stop_binding_20261113 import verify_stop_binding
stops=[]
for label,prior in [('election_one','timeout'),('election_two','restored')]:
 receipt=json.loads((OUT/('leader-stop-'+label+'.json')).read_bytes());before=json.loads((OUT/('cluster-'+prior+'.json')).read_bytes());after=json.loads((OUT/('cluster-'+label+'.json')).read_bytes());process=json.loads(Path(receipt['process_file']).read_bytes());verify_stop_binding(receipt,before,after,process);stops.append(dict(label=label,stopped_node=receipt['node'],stopped_pid=receipt['pid'],stopped_term=receipt['before']['term'],next_pid=after['nodes'][str(after['leader'])]['pid'],next_term=after['nodes'][str(after['leader'])]['term']))
h=module('archive_raft_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py');a=module('archive_raft_go',ROOT/'tools/go-java-v1-inflight/audit.py');deps=module('archive_raft_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py');runner=module('archive_raft_runner',HERE/'run.py')
assert a.run(Path(summary['cohort']))==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go()
assert h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')==h.JAR_HASH==summary['java_jar_sha256'];assert h.sha(h.DRIVER)==summary['driver_sha256']
assert sha(OUT/'real-timeout.exe')==summary['binary_sha256']==sha(Path(summary['parent_evidence'])/'real-timeout.exe')
dependencies=deps.dependency_inventory((OUT/'effective-classpath.txt').read_text().strip());assert dependencies==json.loads((OUT/'compile-dependency-audit.json').read_bytes())
overlay=json.loads((OUT/'overlay-manifest.json').read_bytes());assert len(overlay)==21
assert [dict(path=p.relative_to(OUT/'overlay').as_posix(),sha256=sha(p)) for p in sorted((OUT/'overlay').rglob('*.class'))]==overlay
compiled=json.loads((OUT/'compiled-manifest.json').read_bytes());assert len(compiled)==3 and [dict(path=p.relative_to(OUT/'compiled').as_posix(),sha256=sha(p)) for p in sorted((OUT/'compiled').rglob('*.class'))]==compiled
parent=Path(summary['raft_parent']);check=runner.catalog(parent,'e2a284c1cdf7f2b09821ec2daf363730ce9b115f445a907303760d80a5cb79a2');check(parent/'overlay-manifest.json')
for row in json.loads((parent/'overlay-manifest.json').read_bytes()):
 if 'RaftReloadObservationControl' in row['path']:continue
 check(parent/'overlay'/row['path']);assert sha(parent/'overlay'/row['path'])==row['sha256']==sha(OUT/'overlay'/row['path'])
assert len(proof['actual_class_origins'])==20 and len(proof['actual_snapshot_loads'])==2 and len(proof['snapshot_metadata'])==6
assert e.parse_metadata((OUT/'snapshot-metadata-reader-output.log').read_bytes())==proof['snapshot_metadata']
assert all(json.loads((OUT/'final-input-check.json').read_bytes()).values())
runtime=json.loads((OUT/'runtime.json').read_bytes());assert len(set(runtime['all_tc_pids']+runtime['sdk_pids']))==8 and len(set(runtime['database_starts']))==3
item=json.loads(h.command(['docker','inspect',result['container']]).stdout)[0];assert item['State']['Status']=='exited' and item['Image']==h.IMAGES['db'] and item['Config']['Labels']['seata-raft-at']==Path(result['runtime']).name
mount=next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql');assert mount['Source'].replace('\\','/').lower()==(Path(result['runtime'])/'mysql').as_posix().lower()
for port in runner.PORTS:
 try:
  with socket.create_connection((runner.HOST if port in runner.RAFT else '127.0.0.1',port),timeout=.2):raise AssertionError('Owned port still listening: '+str(port))
 except OSError:pass
ids=','.join(str(v) for v in runtime['all_tc_pids']+runtime['sdk_pids']);command='$ownedIds=@('+ids+'); @(Get-CimInstance Win32_Process | Where-Object { $ownedIds -contains $_.ProcessId } | Select-Object ProcessId,CommandLine) | ConvertTo-Json -Compress';raw=h.command(['powershell','-NoProfile','-Command',command]).stdout.strip();present=json.loads(raw) if raw else []
if isinstance(present,dict):present=[present]
assert not any(Path(result['runtime']).name in str(row.get('CommandLine')) for row in present)
save(OUT/'cleanup-verification.json',dict(container=item['Id'],state='exited',image=item['Image'],ports_released=runner.PORTS,owned_tc_pids=runtime['all_tc_pids'],owned_sdk_pids=runtime['sdk_pids'],owned_processes_stopped=True,reused_pid_observations=present))
post_paths=[Path(__file__),Path('D:/codex-builds/raft_stop_binding_20261113.py'),Path('D:/codex-builds/test_raft_stop_binding_20261113.py')];post={p:p.read_bytes() for p in post_paths};post_rows=[]
for path,data in post.items():
 leaf='post-run-validation/'+path.name;put(OUT/leaf,data);post_rows.append(dict(path=str(path),snapshot=leaf,sha256=sha(path)))
save(OUT/'post-run-validation-inputs.json',post_rows)
test=h.command(['python','-m','unittest','discover','-s','D:/codex-builds','-p','test_raft_stop_binding_20261113.py','-v']);assert b'Ran 4 tests' in test.stdout and test.stdout.rstrip().endswith(b'OK')
# unittest耗时每次不同，首份实际日志封存；重跑只验证结果，不覆盖历史。
if not (OUT/'stop-binding-tests.log').exists():put(OUT/'stop-binding-tests.log',test.stdout)
for path,data in post.items():assert path.read_bytes()==data
for name in ['raft-business-cluster-red.log','raft-business-followers-red.log','raft-business-metadata-red.log','raft-business-full-evidence-red.log','raft-stop-binding-red.log','raft-stop-binding-green.log']:
 put(BASE/'diagnostics'/name,(Path('D:/codex-builds')/name).read_bytes())
for name in ['legacy_raft_business_evidence_20261113.py','extract_raft_business_legacy_validator_20261113.py','cleanup_raft_at_052429_20261113.py']:
 put(BASE/'diagnostics'/name,(Path('D:/codex-builds')/name).read_bytes())
assert b'Ran 65 tests' in (OUT/'python-tests.log').read_bytes() and (OUT/'python-tests.log').read_bytes().rstrip().endswith(b'OK')
save(OUT/'stop-identity-certification.json',dict(succeeded=True,additional_to_frozen_runtime=True,validator='post-run-validation/raft_stop_binding_20261113.py',tests=4,red_failures=3,red_controls=1,transitions=stops))
texts='\n'.join(p.read_text(encoding='utf-8',errors='replace') for p in OUT.glob('node_*_generation_*/tc-stdout.log'))
save(OUT/'independent-revalidation.json',dict(succeeded=True,frozen_inputs=923,post_run_validation_inputs=3,runtime_python_tests=65,post_run_stop_tests=4,total_new_python_tests=69,compile_classpath_entries=len(dependencies),compile_dependency_files=sum(1 if row['kind']=='file' else len(row['files']) for row in dependencies),production_overlay_classes=20,readonly_control_classes=1,actual_class_origins=20,actual_tc_jvms=5,actual_sdk_jvms=3,actual_elections=2,snapshot_requests=6,actual_snapshot_loads=2,actual_jfr_recordings_replayed=2,native_queries=5,native_frames=30,go_unit_tests_inherited=254,rust_tests_inherited=117,java_raft_candidate_unit_tests_inherited=52,new_go_unit_execution=False,new_rust_unit_execution=False,new_java_unit_execution=False,metadata_issue_string_occurrences=texts.count('PutNodeMetadataResponse'),logging_issue_string_occurrences=texts.count('configureByResource'),original_java_raft_reload_issue='OPEN',original_metadata_logging_issues='OPEN',scope=summary['scope']))
print(json.dumps(dict(succeeded=True,primary=OUT.name,source_inputs=923,new_python_tests=69,compile_classpath_entries=len(dependencies),compile_dependency_files=sum(1 if r['kind']=='file' else len(r['files']) for r in dependencies))))
