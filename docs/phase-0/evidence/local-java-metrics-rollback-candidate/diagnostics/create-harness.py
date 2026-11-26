from pathlib import Path
root=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
old=root/'tools/java-metrics-lifecycle-candidate';new=root/'tools/java-metrics-rollback-candidate'
def change(text,before,after):
 assert before in text,before
 return text.replace(before,after)
checker=(old/'metrics_verify.py').read_text(encoding='utf-8')
for before,after in [('after_committed','after_rollbacked'),('AfterCommitted','AfterRollbacked'),('CommitRetryTimeout','RollbackRetryTimeout'),("'Committed'","'Rollbacked'"),('real-mixed-commit-timeout','real-at-natural-timeout'),('Partial commit','Partial rollback'),('after-commit','after-rollback')]:checker=change(checker,before,after)
(new/'metrics_verify.py').write_text(checker,encoding='utf-8')
control=(old/'MetricsLifecycleControl.java').read_text(encoding='utf-8')
for before,after in [('MetricsLifecycleControl','MetricsRollbackControl'),('after_committed','after_rollbacked'),('AFTER_COMMITTED','AFTER_ROLLBACKED')]:control=change(control,before,after)
(new/'MetricsRollbackControl.java').write_text(control,encoding='utf-8')
test=(old/'test_metrics_verify.py').read_text(encoding='utf-8')
test=change(test,"official=os.environ.get('SEATA_METRICS_EVIDENCE');source=Path(official or 'E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation/docs/phase-0/evidence/local-java-metrics-lifecycle-candidate/run_20261113_042734_82868d53')","official=os.environ['SEATA_METRICS_ROLLBACK_EVIDENCE'];source=Path(official)")
start=test.index('  if not official:');end=test.index(' def read(self):',start);test=test[:start]+test[end:]
for before,after in [('after_committed','after_rollbacked'),('CommitRetryTimeout','RollbackRetryTimeout')]:
 if before in test:test=test.replace(before,after)
(new/'test_real_metrics.py').write_text(test,encoding='utf-8')
run=(old/'run.py').read_text(encoding='utf-8')
run=change(run,'Owned real FILE16 lifecycle metrics','Owned real FILE17 rollback lifecycle metrics')
run=change(run,"('mct_'+stamp)","('mrt_'+stamp)")
run=change(run,"parent=ROOT/'docs/phase-0/evidence/local-go-java-file-commit-timeout/run_20261113_035053_4ea827e1'","parent=ROOT/'docs/phase-0/evidence/local-go-java-file-at-timeout/run_20261113_032718_6712fd5e'")
run=change(run,'aec4261bc4ed8e5036d89d57a5bd2d3c61289c3257a0318d439027372dcd5f48','4a8bb3abfcbfde159ecff41e3354b0495d12152761e1880bf7b78cb1f465fc1b')
run=change(run,'Certified FILE commit','Certified FILE rollback')
run=change(run,"previous=json.loads((parent/'summary.json').read_bytes());real_summary=previous",'''previous=json.loads((parent/'summary.json').read_bytes())
 metrics_parent=ROOT/'docs/phase-0/evidence/local-java-metrics-lifecycle-candidate/run_20261113_043739_9fbdad8c';metrics_catalog=metrics_parent.parent/'manifest.json'
 if sha(metrics_catalog.read_bytes())!='ddb4ded1ccd8740d2bc603722f1808caa6d61feed9b652470663bbd885cbe666':raise RuntimeError('Certified metrics parent catalog changed')
 metrics_catalog_rows=json.loads(metrics_catalog.read_bytes())['files']
 def authenticate_metrics(path):
  expected=next(row['sha256'] for row in metrics_catalog_rows if row['path']==path.relative_to(metrics_catalog.parent).as_posix())
  if sha(path.read_bytes())!=expected:raise RuntimeError('Certified metrics parent file changed: '+str(path))
 for path in [metrics_parent/'summary.json',metrics_parent/'source-input-audit.json',metrics_parent/'external-input-audit.json',metrics_parent/'metrics-overlay-manifest.json']:authenticate_metrics(path)
 real_summary=json.loads((metrics_parent/'summary.json').read_bytes());metrics_here=ROOT/'tools/java-metrics-lifecycle-candidate' ''')
run=change(run,"ROOT/'docs/phase-0/java-metrics-lifecycle-candidate-plan.md'","ROOT/'docs/phase-0/java-metrics-rollback-candidate-plan.md'")
run=change(run,"rows=[]\n for path,data in sorted(source.items()):",'''for path in [metrics_catalog,metrics_parent/'summary.json',metrics_parent/'source-input-audit.json',metrics_parent/'external-input-audit.json',metrics_parent/'metrics-overlay-manifest.json']+list((metrics_parent/'metrics-overlay').rglob('*.class')):
  authenticate_metrics(path) if path!=metrics_catalog else None
  source[path]=path.read_bytes()
 for row in json.loads((metrics_parent/'source-input-audit.json').read_bytes()):
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256']:raise RuntimeError('Certified metrics source changed: '+row['path'])
  source[path]=data
 for path in (ROOT/'tools/go-java-v1-file-at-timeout').glob('*.py'):source[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(source.items()):''')
for before,after in [('tools/go-java-v1-real-commit-timeout/proxy.py','tools/go-java-v1-real-at-timeout/proxy.py'),('tools/go-java-v1-file-commit-timeout/verify.py','tools/go-java-v1-file-at-timeout/verify.py'),('tools/go-java-v1-file-commit-timeout/evidence.py','tools/go-java-v1-file-at-timeout/evidence.py'),("binary=out/'real-commit.exe'","binary=out/'real-timeout.exe'"),("ROOT/'tools/go-java-v1-real-commit-timeout/main.go',ROOT/'tools/go-java-v1-real-commit-timeout/owned_commit_action.go'","ROOT/'tools/go-java-v1-real-at-timeout/main.go'"),("+(ROOT/'tools/go-java-v1-real-commit-timeout/schema.sql').read_bytes()",''),('seata_commit_fixture_only','seata_timeout_fixture_only'),('seata_commit_','seata_timeout_'),('seata-metrics-lifecycle','seata-metrics-rollback'),('max-commit-retry-timeout','max-rollback-retry-timeout'),('committing-retry-period: 200\n      async-committing-retry-period: 200','rollbacking-retry-period: 200'),("HERE/'MetricsSubscriber.java',HERE/'MetricsLifecycleControl.java'","metrics_here/'MetricsSubscriber.java',HERE/'MetricsRollbackControl.java'"),("(HERE/'MetricsSubscriber.java')","(metrics_here/'MetricsSubscriber.java')"),('MetricsLifecycleControl','MetricsRollbackControl'),('CommitRetryTimeout','RollbackRetryTimeout'),('p.CommitProxy','p.TimeoutProxy')]:run=change(run,before,after)
run=change(run,"candidate_class=compiled/'org/apache/seata/server/metrics/MetricsSubscriber.class'", "candidate_class=compiled/'org/apache/seata/server/metrics/MetricsSubscriber.class'\n  authenticate_metrics(metrics_parent/'metrics-overlay/org/apache/seata/server/metrics/MetricsSubscriber.class')\n  if h.sha(candidate_class)!=h.sha(metrics_parent/'metrics-overlay/org/apache/seata/server/metrics/MetricsSubscriber.class'):raise RuntimeError('Rollback production metrics class differs from certified FILE16 bytes')")
run=change(run,"save(out/'external-input-audit.json',external_rows)","save(out/'external-input-audit.json',external_rows)\n  certified_external=json.loads((metrics_parent/'external-input-audit.json').read_bytes())\n  if [(r['path'],r['sha256']) for r in external_rows]!=[(r['path'],r['sha256']) for r in certified_external]:raise RuntimeError('Frozen unit evidence no longer matches certified metrics parent')")
run=change(run,"; SELECT 'commit_account',id,reserved,confirmed FROM commit_account; SELECT 'reservation',xid,branch_id,phase,effect_count FROM commit_reservation",'')
run=change(run,"15 if name=='final' else 16,True,events","15 if name=='final' else 17,name=='final',events")
run=change(run,"'initial','deny-commit'","'initial','deny-rollback'")
run=change(run," if row['branch_type']==0",'')
run=change(run,"['status']==16","['status']==17")
run=change(run,'Timeout16','Timeout17')
run=change(run,'Original TM single commit returned','Original TM single rollback returned')
run=change(run,"'operator_loss','lose-tcc-ack'","'operator_loss','lose-ack'")
run=change(run,'successful-tcc-response-lost','successful-rollback-response-lost')
run=change(run,'Actual completed TCC confirmation response loss','Actual completed compensation response loss')
run=change(run,"h.wait_until(lambda:sql('SELECT COUNT(*) FROM undo_log').strip()==b'0',10,'Actual AT Undo cleanup',sdk);",'')
run=change(run,"tests_env['SEATA_FILE_COMMIT_EVIDENCE']","tests_env['SEATA_FILE_TIMEOUT_EVIDENCE']")
run=change(run,"ROOT/'tools/go-java-v1-file-commit-timeout'","ROOT/'tools/go-java-v1-file-at-timeout'")
run=change(run,"tests_env['SEATA_METRICS_EVIDENCE']","tests_env['SEATA_METRICS_ROLLBACK_EVIDENCE']")
run=change(run,'new_java_unit_execution=True,new_java_tests=77','new_java_unit_execution=False,inherited_candidate_java_tests=77')
run=change(run,'real FILE16 mixed recovery','real FILE17 rollback recovery')
run=change(run,'reconstruction,17,RAFT','reconstruction,16+17 concurrent,RAFT')
run=change(run,"local-java-metrics-lifecycle-candidate');run", "local-java-metrics-rollback-candidate');run")
(new/'run.py').write_text(run,encoding='utf-8')
print('Created independent rollback harness and read-only control')
