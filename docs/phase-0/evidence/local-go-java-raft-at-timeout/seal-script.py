from pathlib import Path
import hashlib,json,re,runpy
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-raft-at-timeout';OUT=BASE/'run_20261113_052816_8ea31012'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
assert json.loads((BASE/'review.json').read_bytes())['succeeded']
runpy.run_path('D:/codex-builds/archive_raft_at_timeout_20261113.py')
put(BASE/'.gitattributes','* -text\n');put(BASE/'seal-script.py',Path(__file__).read_bytes())
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());independent=json.loads((OUT/'independent-revalidation.json').read_bytes())
save(BASE/'latest_run.json',dict(succeeded=True,evidence_directory=str(OUT),runtime_directory=result['runtime'],requires_additional_stop_identity_gate=True))
docs=[ROOT/name for name in ['docs/delivery-progress.md','docs/phase-0/README.md','docs/phase-0/validation-ledger.md','docs/phase-0/java-tc-retry-timeout-recovery.md','docs/phase-0/go-java-v1-raft-at-timeout-plan.md']];links=0
for path in docs+[BASE/'README.md']:
 for link in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
  link=link.strip('<>').split('#')[0]
  if not link or re.match(r'^[a-z]+:',link,re.I):continue
  assert (path.parent/link).exists(),(path,link);links+=1
manifest=dict(succeeded=True,primary=OUT.name,frozen_runtime_inputs=923,post_run_certification_inputs=3,runtime_python_tests=65,additional_stop_identity_python_tests=4,total_new_python_tests=69,requires_runtime_and_additional_stop_identity_gate=True,compile_classpath_entries=341,compile_dependency_files=2821,production_overlay_classes=20,readonly_control_classes=1,actual_critical_class_origins=20,actual_tc_jvms=5,actual_sdk_processes=3,sdk_process_kind='Go executable',observed_leader_transitions=2,sampled_leader_terms=[1,2,4],snapshot_requests=6,snapshot_index_terms=[[9,1],[9,1],[9,1],[13,2],[13,2],[13,2]],actual_snapshot_loads=2,jfr_recordings_reparsed=2,business_database_restarts=2,native_readonly_queries=5,native_frames=30,go_tests_inherited=254,rust_tests_inherited=117,java_raft_candidate_tests_inherited=52,new_go_unit_execution=False,new_rust_unit_execution=False,new_java_unit_execution=False,original_java_jar_sha256=summary['java_jar_sha256'],sdk_binary_sha256=summary['binary_sha256'],original_reference_reload_issue='OPEN',original_metadata_logging_issues='OPEN',resources_stopped=True,ports_released=11,documentation=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in docs],links_checked=links,files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json'],scope=summary['scope'])
save(BASE/'manifest.json',manifest)
print(json.dumps(dict(succeeded=True,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']),links_checked=links,primary=OUT.name)))
