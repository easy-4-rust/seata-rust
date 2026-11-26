from pathlib import Path
import hashlib,json,re,runpy
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-java-metrics-rollback-candidate';PRIMARY=BASE/'run_20261113_045658_cc75268a';UNIT=Path('D:/codex-builds/metrics-lifecycle-20261113')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
assert json.loads((BASE/'review.json').read_bytes())['succeeded']
runpy.run_path('D:/codex-builds/archive_metrics_rollback_20261113.py')
put(BASE/'.gitattributes','* -text\n');put(BASE/'seal-script.py',Path(__file__).read_bytes())
for name in ['frozen-red.log','frozen-green.log','frozen-source-audit.json','frozen-binary-audit.json','frozen-candidate-compile.args','frozen-test-compile.args']:
 put(BASE/'java-unit'/name,(UNIT/name).read_bytes())
result=json.loads((PRIMARY/'result.json').read_bytes());summary=json.loads((PRIMARY/'summary.json').read_bytes())
save(BASE/'latest_run.json',dict(succeeded=True,evidence_directory=str(PRIMARY),runtime_directory=result['runtime']))
docs=[ROOT/name for name in ['docs/delivery-progress.md','docs/phase-0/README.md','docs/phase-0/validation-ledger.md','docs/phase-0/java-tc-retry-timeout-recovery.md','docs/phase-0/java-metrics-rollback-candidate-plan.md']]
links=0
for path in docs+[BASE/'README.md']:
 for link in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
  link=link.strip('<>').split('#')[0]
  if not link or re.match(r'^[a-z]+:',link,re.I):continue
  assert (path.parent/link).exists(),(path,link);links+=1
patch=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7/0004-honor-metrics-event-lifecycle.patch'
assert sha(patch)=='54d03915495491292fefce99700d68da65895172b5c667cd37089a2ca405f1bf'
manifest=dict(succeeded=True,primary=PRIMARY.name,patch_sha256=sha(patch),source_inputs=854,external_inputs=40,compile_classpath_entries=278,compile_dependency_files=1950,inherited_candidate_java_tests=77,new_java_unit_execution=False,python_tests=71,business_python_tests=44,new_metrics_python_tests=27,inherited_go_tests=254,inherited_rust_tests=117,inherited_java_full_suite=617,new_go_unit_execution=False,new_rust_execution=False,registry_snapshots=8,observation_receipts=8,positive_nullable_event_barriers=2,actual_class_origins=30,production_overlay_classes=20,readonly_control_classes=1,actual_tc_processes=3,actual_sdk_processes=3,business_database_restarts=2,journal_restores=2,native_queries=5,raw_native_frames=30,metrics_original_issue_status='OPEN',candidate_file17_lifecycle_verified=True,original_java_jar_sha256=summary['java_jar_sha256'],sdk_binary_sha256=summary['binary_sha256'],documentation=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in docs],links_checked=links,files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json'],scope=summary['scope'])
save(BASE/'manifest.json',manifest)
print(json.dumps(dict(succeeded=True,primary=PRIMARY.name,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']),links_checked=links)))
