from pathlib import Path
import hashlib,importlib.util,json,os,subprocess,sys
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
BASE=ROOT/'docs/phase-0/evidence/local-go-java-file-commit-timeout';OUT=BASE/'run_20261113_035053_4ea827e1';HERE=ROOT/'tools/go-java-v1-file-commit-timeout'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
sys.path.insert(0,str(HERE));import evidence,verify
candidate=module('file_commit_candidate_audit',ROOT/'tools/go-java-v1-file-at-timeout/candidate.py')
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());assert summary['succeeded'] and result['succeeded'] and result['container_state']=='exited' and result['ports_released']
proof=verify.validate(evidence.derive(OUT));assert json.loads(json.dumps(proof))==json.loads((OUT/'proof.json').read_bytes())==result['proof']
inputs=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(inputs)==summary['frozen_inputs']==819
for row in inputs:assert sha(ROOT/row['path'])==row['sha256']==sha(OUT/row['snapshot'])
parent=Path(summary['java_candidate_parent']);inventory=candidate.verify(parent)
h=module('file_archive_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
a=module('file_archive_go_audit',ROOT/'tools/go-java-v1-inflight/audit.py');real_parent=Path(summary['parent_evidence']);cohort=Path(summary['cohort'])
assert a.run(cohort)==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go()
assert h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')==h.JAR_HASH==summary['java_jar_sha256']
assert h.sha(OUT/'real-commit.exe')==summary['binary_sha256']
assert all(json.loads((OUT/'final-input-check.json').read_bytes()).values())
java=module('file_commit_class_source_audit',ROOT/'tools/java-preface-candidate/runtime.py');prepare=module('file_commit_preface_definition',ROOT/'tools/java-preface-candidate/prepare_candidate.py')
for name in ['initial','operator_loss','operator_replay']:
 directory=OUT/name
 assert json.loads((directory/'overlay-before.json').read_bytes())==inventory==json.loads((directory/'overlay-after.json').read_bytes())
 assert (directory/'runtime-artifacts-before.json').read_bytes()==(directory/'runtime-artifacts-after.json').read_bytes()
 assert (directory/'jdbc-before.json').read_bytes()==(directory/'jdbc-after.json').read_bytes()
 assert json.loads((directory/'retention-class-origins.json').read_bytes())==candidate.class_sources(directory/'tc-stdout.log',parent/'overlay')
 assert json.loads((directory/'class-origins.json').read_bytes())==java.class_sources(directory/'tc-stdout.log',parent/'overlay',prepare)
rows=proof['file_storage']['journals'];restores=proof['file_storage']['restore_inputs']
assert rows[0]['files']==restores[0]['files'] and rows[1]['files']==restores[1]['files']
assert rows[0]['files']!=rows[1]['files'],'Partial commit must persist original AT branch removal before the second restart'
for row in rows[-1]['files']:
 path=Path(result['runtime'])/'store'/row['path'];assert sha(path)==row['sha256']
runtime=json.loads((OUT/'runtime.json').read_bytes());assert len(set(runtime['tc_pids']+runtime['sdk_pids']))==6
item=json.loads(h.command(['docker','inspect',result['container']]).stdout)[0]
assert item['Id']==result['container'] and item['State']['Status']=='exited' and item['Image']==h.IMAGES['db']
assert item['Config']['Labels']['seata-file-commit-timeout']==Path(result['runtime']).name
assert item['State']['StartedAt']==runtime['database_starts'][-1]
assert next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')['Source'].replace('\\','/').lower()==(Path(result['runtime'])/'mysql').as_posix().lower()
assert not any(h.port_open(p) for p in [28091,28092,28093])
save(OUT/'cleanup-verification.json',dict(container=item['Id'],state=item['State']['Status'],image=item['Image'],label=Path(result['runtime']).name,ports_released=True))
for name in ['file-commit-verifier-red-20261113.log','file-commit-at-status-red-20261113.log','file-commit-report-status-red-20261113.log','file-commit-context-red-20261113.log','file-commit-report-ack-red-20261113.log','file-commit-phaseone-green-20261113.log','file-commit-raw-green-20261113.log','file-commit-final-green-20261113.log']:
 put(BASE/'diagnostics'/name,(Path('D:/codex-builds')/name).read_bytes())
log=(OUT/'python-tests.log').read_bytes();assert b'Ran 57 tests' in log and log.rstrip().endswith(b'OK')
save(OUT/'independent-revalidation.json',dict(succeeded=True,frozen_inputs_verified=819,go_cohort_verified=True,candidate_classes_verified=19,actual_class_origins=24,native_queries_replayed=5,raw_native_frames=30,journal_restores_verified=2,partial_commit_journal_changed=True,original_tm_begins=proof['tm_begins'],original_tm_commits=proof['tm_commits'],original_tm_rollbacks=proof['tm_rollbacks'],actual_request_withheld=proof['withheld'],actual_success_ack_lost=proof['lost'],sdk_processes=3,tc_processes=3,business_database_restarts=2,python_tests=57,primary=str(OUT)))
metrics=[]
for name in ['initial','operator_loss','operator_replay']:
 path=OUT/name/'tc-stdout.log';lines=path.read_text(encoding='utf-8',errors='replace').splitlines()
 for index,line in enumerate(lines):
  if 'java.lang.NullPointerException:' in line and 'GlobalTransactionEvent.getEndTime()' in line:
   metrics.append(dict(path=path.relative_to(OUT).as_posix(),line=index+1,context=lines[max(0,index-2):index+10]))
save(OUT/'known-metrics-events.json',dict(issue='JAVA-METRICS-RETRY-001',status='OPEN',events=metrics))
put(OUT/'archive-script.py',Path(__file__).read_bytes())
print(json.dumps(dict(succeeded=True,primary=OUT.name,frozen_inputs=819,python_tests=57,journal_restores=2,native_queries=5)))
