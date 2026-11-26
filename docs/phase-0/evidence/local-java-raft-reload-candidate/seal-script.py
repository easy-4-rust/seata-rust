from pathlib import Path
import hashlib,json,re,runpy
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
BASE=ROOT/'docs/phase-0/evidence/local-java-raft-reload-candidate';PRIMARY=BASE/'run_20261113_030623_c4a999a3'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,data):put(path,json.dumps(data,indent=2)+'\n')
assert json.loads((BASE/'review.json').read_bytes())['succeeded']
runpy.run_path('D:/codex-builds/archive_raft_observation_final_20261113.py')
put(BASE/'.gitattributes','* -text\n')
for name in ['red-run.log','green-run.log','fresh-regression.log','fresh-regression-compile.log','spring-regression.log']:
 put(BASE/'diagnostics'/name,(Path('D:/codex-builds/raft-observation-20261113')/name).read_bytes())
put(BASE/'seal-script.py',Path(__file__).read_bytes())
docs=[ROOT/name for name in ['docs/delivery-progress.md','docs/phase-0/README.md','docs/phase-0/validation-ledger.md','docs/phase-0/java-tc-retry-timeout-recovery.md','docs/phase-0/java-raft-reload-observation-candidate-plan.md','tools/java-raft-reload-candidate/README.md']]
links=0
for path in docs+[BASE/'README.md']:
 for link in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
  link=link.strip('<>').split('#')[0]
  if not link or re.match(r'^[a-z]+:',link,re.I):continue
  assert (path.parent/link).exists(),(path,link)
  links+=1
patch=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7/0003-observe-raft-session-reload.patch'
assert sha(patch)=='144e13f24cc660df9ca862bc4c79e2fe0bf83240b7bfd2683d6b8bbb833f9da5'
manifest=dict(succeeded=True,primary=PRIMARY.name,java_tests=52,new_java_tests=8,original_raft_regressions=44,primary_run_java_tests_newly_executed=0,primary_run_java_tests_reused_by_binary_equivalence=52,python_tests=18,actual_tc_jvms=12,leader_transitions=6,snapshot_reloads=3,native_queries=18,raw_frames=108,leader_confirmations=18,class_origin_observations=48,frozen_inputs=53,generated_class_inputs=3,compile_classpath_entries=340,compile_dependency_files=2800,overlay_classes=21,patch_sha256=sha(patch),documentation=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in docs],links_checked=links,files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json'],scope='Independent Java preface/retention/async-success-observation candidate; actual seeded RAFT elections and snapshot reload only; no failed-leader fencing, natural deadline business recovery or production Rust acceptance')
save(BASE/'manifest.json',manifest)
print(json.dumps(dict(succeeded=True,primary=PRIMARY.name,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']),links_checked=links)))
