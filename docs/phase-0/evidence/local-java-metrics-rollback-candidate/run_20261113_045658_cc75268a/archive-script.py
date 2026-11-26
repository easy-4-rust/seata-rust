from pathlib import Path
import hashlib,importlib.util,json,sys
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-java-metrics-rollback-candidate';OUT=BASE/'run_20261113_045658_cc75268a';HERE=ROOT/'tools/java-metrics-rollback-candidate'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,data):put(path,json.dumps(data,indent=2)+'\n')
summary=json.loads((OUT/'summary.json').read_bytes());result=json.loads((OUT/'result.json').read_bytes());assert summary['succeeded'] and result['succeeded'] and result['container_state']=='exited' and result['ports_released']
rows=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(rows)==summary['frozen_inputs']==854
for row in rows:assert sha(ROOT/row['path'])==row['sha256']==sha(OUT/row['snapshot'])
external=json.loads((OUT/'external-input-audit.json').read_bytes());assert len(external)==40
for row in external:assert sha(Path(row['path']))==row['sha256']==sha(OUT/row['snapshot'])
sys.path.insert(0,str(HERE));import metrics_verify
business=module('metrics_business_evidence',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py');validator=module('metrics_business_validator',ROOT/'tools/go-java-v1-file-at-timeout/verify.py');parent_audit=module('metrics_parent_class_audit',ROOT/'tools/go-java-v1-file-at-timeout/candidate.py')
proof=validator.validate(business.derive(OUT));assert json.loads(json.dumps(proof))==json.loads((OUT/'proof.json').read_bytes())==result['proof']
metrics=metrics_verify.derive(OUT);assert metrics==json.loads((OUT/'metrics-proof.json').read_bytes()) and len(metrics['snapshots'])==8
h=module('metrics_archive_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py');a=module('metrics_archive_go_audit',ROOT/'tools/go-java-v1-inflight/audit.py');deps=module('metrics_archive_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
assert a.run(Path(summary['cohort']))==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go()
classpath=(OUT/'effective-classpath.txt').read_text().strip();dependencies=deps.dependency_inventory(classpath);assert dependencies==json.loads((OUT/'compile-dependency-audit.json').read_bytes())
assert sha(OUT/'real-timeout.exe')==sha(Path(summary['parent_evidence'])/'real-timeout.exe')
assert h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')==h.JAR_HASH==summary['java_jar_sha256']
assert sha(OUT/'real-timeout.exe')==summary['binary_sha256'] and h.sha(h.DRIVER)==summary['driver_sha256']
parent=Path(summary['java_candidate_parent']);parent_classes=parent_audit.verify(parent);unit=Path('D:/codex-builds/metrics-lifecycle-20261113');binary=json.loads((unit/'frozen-binary-audit.json').read_bytes())
for row in json.loads((unit/'frozen-source-audit.json').read_bytes()):assert sha(Path(row['path']))==row['sha256']==sha(Path(row['snapshot']))
for row in binary['test_classes']:assert sha(unit/'frozen-test-classes'/row['path'])==row['sha256']==sha(unit/'test-classes'/row['path'])
leaf='org/apache/seata/server/metrics/MetricsSubscriber.class';assert sha(OUT/'metrics-overlay'/leaf)==binary['candidate_class_sha256']==sha(unit/'candidate'/leaf)==sha(unit/'frozen-candidate'/leaf)
overlay=json.loads((OUT/'metrics-overlay-manifest.json').read_bytes());assert len(overlay)==2
for row in overlay:assert sha(OUT/'metrics-overlay'/row['path'])==row['sha256']
assert b'OBSERVATION_JUNIT tests=77 succeeded=30 failed=47 skipped=0' in (unit/'frozen-red.log').read_bytes()
assert b'OBSERVATION_JUNIT tests=77 succeeded=77 failed=0 skipped=0' in (unit/'frozen-green.log').read_bytes()
java=module('metrics_archive_preface_sources',ROOT/'tools/java-preface-candidate/runtime.py');prepare=module('metrics_archive_preface_definitions',ROOT/'tools/java-preface-candidate/prepare_candidate.py');origins=[]
for name in ['initial','operator_loss','operator_replay']:
 directory=OUT/name;assert json.loads((directory/'overlay-before.json').read_bytes())==parent_classes==json.loads((directory/'overlay-after.json').read_bytes())
 assert (directory/'runtime-artifacts-before.json').read_bytes()==(directory/'runtime-artifacts-after.json').read_bytes()
 assert (directory/'jdbc-before.json').read_bytes()==(directory/'jdbc-after.json').read_bytes()
 assert json.loads((directory/'retention-class-origins.json').read_bytes())==parent_audit.class_sources(directory/'tc-stdout.log',parent/'overlay')
 assert json.loads((directory/'class-origins.json').read_bytes())==java.class_sources(directory/'tc-stdout.log',parent/'overlay',prepare)
 text=(directory/'tc-stdout.log').read_text(encoding='utf-8',errors='replace')
 for cls in ['org.apache.seata.server.metrics.MetricsSubscriber','org.apache.seata.server.metrics.MetricsRollbackControl']:
  matches=[line for line in text.splitlines() if ' '+cls+' source: ' in line];assert len(matches)==1 and 'file:/'+(OUT/'metrics-overlay').as_posix()+'/' in matches[0];origins.append(dict(stage=name,type=cls,line=matches[0]))
 assert 'Exception thrown by subscriber method recordGlobalTransactionEventForMetrics' not in text
 assert 'GlobalTransactionEvent.getEndTime()' not in text
save(OUT/'actual-metrics-class-origins.json',origins)
runtime=json.loads((OUT/'runtime.json').read_bytes());assert len(set(runtime['tc_pids']+runtime['sdk_pids']))==6
assert len(runtime['database_starts'])==len(set(runtime['database_starts']))==3
for index in range(2):assert proof['file_storage']['journals'][index]['files']==proof['file_storage']['restore_inputs'][index]['files']
for row in proof['file_storage']['journals'][-1]['files']:assert sha(Path(result['runtime'])/'store'/row['path'])==row['sha256']
assert all(json.loads((OUT/'final-input-check.json').read_bytes()).values())
item=json.loads(h.command(['docker','inspect',result['container']]).stdout)[0];assert item['State']['Status']=='exited' and item['Image']==h.IMAGES['db'] and item['Config']['Labels']['seata-metrics-rollback']==Path(result['runtime']).name
assert item['State']['StartedAt']==runtime['database_starts'][-1]
assert not any(h.port_open(port) for port in [28091,28092,28093,28094,28095])
save(OUT/'cleanup-verification.json',dict(container=item['Id'],state='exited',image=item['Image'],ports_released=[28091,28092,28093,28094,28095]))
put(BASE/'diagnostics/rollback-metrics-red.log',Path('D:/codex-builds/rollback-metrics-red.log').read_bytes())
put(BASE/'diagnostics/create-harness.py',Path('D:/codex-builds/create_rollback_metrics_20261113.py').read_bytes())
assert b'Ran 44 tests' in (OUT/'python-tests.log').read_bytes() and (OUT/'python-tests.log').read_bytes().rstrip().endswith(b'OK')
assert b'Ran 27 tests' in (OUT/'metrics-python-tests.log').read_bytes() and (OUT/'metrics-python-tests.log').read_bytes().rstrip().endswith(b'OK')
save(OUT/'independent-revalidation.json',dict(succeeded=True,source_inputs=854,external_inputs=40,compile_classpath_entries=len(dependencies),compile_dependency_files=sum(1 if row['kind']=='file' else len(row['files']) for row in dependencies),new_java_unit_execution=False,inherited_candidate_java_tests=77,business_python_tests=44,metrics_python_tests=27,actual_class_origins=30,parent_classes=19,metrics_candidate_classes=1,readonly_control_classes=1,native_queries_replayed=5,raw_native_frames=30,registry_snapshots=8,observation_receipts=8,positive_skip_barriers=2,owned_tc_processes=3,owned_sdk_processes=3,business_database_restarts=2,metrics_issue_original='OPEN',candidate_file17_lifecycle_verified=True,scope=summary['scope']))
put(OUT/'archive-script.py',Path(__file__).read_bytes())
print(json.dumps(dict(succeeded=True,primary=OUT.name,inherited_candidate_java_tests=77,python_tests=71,source_inputs=854,external_inputs=40)))
