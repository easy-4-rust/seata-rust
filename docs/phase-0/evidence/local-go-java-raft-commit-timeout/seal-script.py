from pathlib import Path
import hashlib,json,re,runpy
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-raft-commit-timeout';OUT=BASE/'run_20261113_055235_cb3164e4'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
assert json.loads((BASE/'review.json').read_bytes())['succeeded']
runpy.run_path('D:/codex-builds/archive_raft_commit_timeout_20261113.py')
put(BASE/'.gitattributes','* -text\n');put(BASE/'seal-script.py',Path(__file__).read_bytes())
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());proof=json.loads((OUT/'proof.json').read_bytes())
save(BASE/'latest_run.json',dict(succeeded=True,evidence_directory=str(OUT),runtime_directory=result['runtime'],election_timeout_ms=3000,requires_additional_context_binding_gate=True,election_1000ms_issue='OPEN'))
docs=[ROOT/name for name in ['docs/delivery-progress.md','docs/phase-0/README.md','docs/phase-0/validation-ledger.md','docs/phase-0/java-tc-retry-timeout-recovery.md','docs/phase-0/go-java-v1-raft-commit-timeout-plan.md','docs/phase-0/raft-commit-runtime-findings.md']];links=0
for path in docs+[BASE/'README.md']:
 for link in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
  link=link.strip('<>').split('#')[0]
  if not link or re.match(r'^[a-z]+:',link,re.I):continue
  assert (path.parent/link).exists(),(path,link);links+=1
manifest=dict(succeeded=True,primary=OUT.name,election_timeout_ms=3000,election_1000ms_issue='OPEN',frozen_runtime_inputs=948,post_run_certification_inputs=3,runtime_python_tests=87,additional_context_binding_python_tests=4,total_new_python_tests=91,requires_runtime_and_additional_context_binding_gate=True,compile_classpath_entries=341,compile_dependency_files=2821,production_overlay_classes=20,readonly_control_classes=1,diagnostic_classes=3,helper_bytes_equal_to_raft17_parent=True,actual_critical_class_origins=20,actual_tc_jvms=5,actual_sdk_processes=3,sdk_process_kind='Go executable',observed_leader_transitions=2,sampled_leader_terms=[1,2,3],snapshot_requests=6,snapshot_index_terms=[[row['index'],row['term']] for row in proof['snapshot_metadata']],actual_snapshot_loads=2,jfr_recordings_reparsed=2,business_database_restarts=2,native_readonly_queries=5,native_frames=30,original_register_report_context_bytes_equal=True,replica_branch_report_bindings=17,changed_prepare_context_replication_certified=False,go_tests_inherited=254,rust_tests_inherited=117,java_raft_candidate_tests_inherited=52,new_go_unit_execution=False,new_rust_unit_execution=False,new_java_unit_execution=False,original_java_jar_sha256=summary['java_jar_sha256'],sdk_binary_sha256=summary['binary_sha256'],original_reference_reload_issue='OPEN',original_metadata_logging_issues='OPEN',resources_stopped=True,ports_released=11,documentation=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in docs],links_checked=links,files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json'],scope=summary['scope'])
save(BASE/'manifest.json',manifest)
print(json.dumps(dict(succeeded=True,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']),links_checked=links,primary=OUT.name)))
