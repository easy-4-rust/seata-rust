from pathlib import Path
import hashlib,importlib.util,json,os,subprocess,sys
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
BASE=ROOT/'docs/phase-0/evidence/local-go-java-file-at-timeout';OUT=BASE/'run_20261113_032718_6712fd5e';HERE=ROOT/'tools/go-java-v1-file-at-timeout'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
sys.path.insert(0,str(HERE));import evidence,verify,candidate
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());assert summary['succeeded'] and result['succeeded'] and result['container_state']=='exited' and result['ports_released']
proof=verify.validate(evidence.derive(OUT));assert proof==json.loads((OUT/'proof.json').read_bytes())==result['proof']
inputs=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(inputs)==summary['frozen_inputs']==803
for row in inputs:assert sha(ROOT/row['path'])==row['sha256']==sha(OUT/row['snapshot'])
parent=Path(summary['java_candidate_parent']);inventory=candidate.verify(parent)
h=module('file_archive_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
a=module('file_archive_go_audit',ROOT/'tools/go-java-v1-inflight/audit.py');real_parent=Path(summary['parent_evidence']);cohort=Path(json.loads((real_parent/'summary.json').read_bytes())['cohort'])
assert a.run(cohort)==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go()
assert h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')==h.JAR_HASH==summary['java_jar_sha256']
assert h.sha(OUT/'real-timeout.exe')==summary['binary_sha256']
assert all(json.loads((OUT/'final-input-check.json').read_bytes()).values())
for name in ['initial','operator_loss','operator_replay']:
 directory=OUT/name
 assert json.loads((directory/'overlay-before.json').read_bytes())==inventory==json.loads((directory/'overlay-after.json').read_bytes())
 assert (directory/'runtime-artifacts-before.json').read_bytes()==(directory/'runtime-artifacts-after.json').read_bytes()
 assert (directory/'jdbc-before.json').read_bytes()==(directory/'jdbc-after.json').read_bytes()
 assert len(json.loads((directory/'retention-class-origins.json').read_bytes()))==3
 assert len(json.loads((directory/'class-origins.json').read_bytes()))==5
rows=proof['file_storage']['journals'];restores=proof['file_storage']['restore_inputs']
assert rows[0]['files']==restores[0]['files'] and rows[1]['files']==restores[1]['files']
assert rows[0]['files']==rows[1]['files'],'Lost acknowledgement must retain the same original FILE journal in this scenario'
for row in rows[-1]['files']:
 path=Path(result['runtime'])/'store'/row['path'];assert sha(path)==row['sha256']
runtime=json.loads((OUT/'runtime.json').read_bytes());assert len(set(runtime['tc_pids']+runtime['sdk_pids']))==6
item=json.loads(h.command(['docker','inspect',result['container']]).stdout)[0]
assert item['Id']==result['container'] and item['State']['Status']=='exited' and item['Image']==h.IMAGES['db']
assert item['Config']['Labels']['seata-real-at-timeout']==Path(result['runtime']).name
assert item['State']['StartedAt']==runtime['database_starts'][-1]
assert next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')['Source'].replace('\\','/').lower()==(Path(result['runtime'])/'mysql').as_posix().lower()
assert not any(h.port_open(p) for p in [28091,28092,28093])
save(OUT/'cleanup-verification.json',dict(container=item['Id'],state=item['State']['Status'],image=item['Image'],label=Path(result['runtime']).name,ports_released=True))
for name in ['file-timeout-verifier-red-20261113.log','file-timeout-raw-red-20261113.log','file-timeout-raw-expanded-red-20261113.log','file-timeout-pid-red-20261113.log','file-timeout-controls-green-20261113.log','file-timeout-controls-final-green-20261113.log']:
 put(BASE/'diagnostics'/name,(Path('D:/codex-builds')/name).read_bytes())
log=(OUT/'python-tests.log').read_bytes();assert b'Ran 44 tests' in log and log.rstrip().endswith(b'OK')
save(OUT/'independent-revalidation.json',dict(succeeded=True,frozen_inputs_verified=803,go_cohort_verified=True,candidate_classes_verified=19,actual_class_origins=24,native_queries_replayed=5,raw_native_frames=30,journal_restores_verified=2,original_journal_unchanged_after_lost_ack=True,original_tm_begins=proof['tm_begins'],original_tm_rollbacks=proof['tm_rollbacks'],actual_request_withheld=proof['request_withheld'],actual_success_ack_lost=proof['successful_response_lost'],sdk_processes=3,tc_processes=3,business_database_restarts=2,python_tests=44,primary=str(OUT)))
put(OUT/'archive-script.py',Path(__file__).read_bytes())
print(json.dumps(dict(succeeded=True,primary=OUT.name,frozen_inputs=803,python_tests=44,journal_restores=2,native_queries=5)))
