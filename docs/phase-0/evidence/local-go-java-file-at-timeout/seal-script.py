from pathlib import Path
import hashlib,json,re,runpy
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-file-at-timeout';PRIMARY=BASE/'run_20261113_032718_6712fd5e'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
assert json.loads((BASE/'review.json').read_bytes())['succeeded']
runpy.run_path('D:/codex-builds/archive_file_at_timeout_20261113.py')
put(BASE/'.gitattributes','* -text\n');put(BASE/'seal-script.py',Path(__file__).read_bytes())
result=json.loads((PRIMARY/'result.json').read_bytes())
save(BASE/'latest_run.json',dict(succeeded=True,evidence_directory=str(PRIMARY),runtime_directory=result['runtime']))
docs=[ROOT/name for name in ['docs/delivery-progress.md','docs/phase-0/README.md','docs/phase-0/validation-ledger.md','docs/phase-0/java-tc-retry-timeout-recovery.md','docs/phase-0/go-java-v1-file-at-timeout-plan.md']]
links=0
for path in docs+[BASE/'README.md']:
 for link in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
  link=link.strip('<>').split('#')[0]
  if not link or re.match(r'^[a-z]+:',link,re.I):continue
  assert (path.parent/link).exists(),(path,link);links+=1
summary=json.loads((PRIMARY/'summary.json').read_bytes())
manifest=dict(succeeded=True,primary=PRIMARY.name,frozen_inputs=803,python_tests_newly_executed=44,inherited_go_tests=254,inherited_rust_tests=117,inherited_java_tests=617,new_go_unit_execution=False,new_rust_execution=False,new_java_unit_execution=False,actual_tc_processes=3,actual_sdk_processes=3,business_database_restarts=2,actual_class_origins=24,java_candidate_classes=19,original_tm_begins=1,original_tm_rollbacks=1,actual_rollback_requests_withheld=8,actual_success_acknowledgements_lost=1,native_queries=5,native_frames=30,journal_restores=2,journal_unchanged_during_lost_ack=True,original_java_jar_sha256=summary['java_jar_sha256'],sdk_binary_sha256=summary['binary_sha256'],documentation=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in docs],links_checked=links,files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json'],scope=summary['scope'])
save(BASE/'manifest.json',manifest)
print(json.dumps(dict(succeeded=True,primary=PRIMARY.name,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']),links_checked=links)))
