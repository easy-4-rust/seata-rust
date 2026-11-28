from pathlib import Path
import sys,json,subprocess,re,hashlib,zipfile,shutil
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');OUT=ROOT/'docs/phase-2/evidence/mysql-datetime/validation_20261128_final';OUT.mkdir(exist_ok=True);sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
from inventory_rebuilt import save,sha
from validate_structured_logs import catalog,sources
runs=[('datetime','docs/phase-2/evidence/mysql-datetime/java_20261128_091126_3388134d','593f9e66417a66525a69233adc98d86a54a6fe856cf31598cc64c82d4ad9ff31'),('datetime','docs/phase-2/evidence/mysql-datetime/java_20261128_091126_f83829a0','7477bfd42270d544fdce7f41de41d47bc380f10b25c5f58a1d45289ac9da9b69'),('json','docs/phase-2/evidence/mysql-json/java_20261128_091452_4eb0d6d8','ca519d136c869f1a9ccc3ec90bd2c6da50265875174bb081631d9d37d4f73801'),('at','docs/phase-2/evidence/at-concurrent-load/java_20261128_091452_95d90e2c','39929299acd82eef68d3b12bd1c092a429f421536bd2c1205b42dea0bdc7492e')]
commands=[];authorities=[]
for i,(kind,path,digest) in enumerate(runs):
 script={'datetime':'validate_datetime_column.py','json':'validate_json_column.py','at':'validate_at_concurrent.py'}[kind];argv=[sys.executable,'-B',str(ROOT/'tools/formal-source-junit'/script),str(ROOT/path),digest,'--weakenings'];r=subprocess.run(argv,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);(OUT/(str(i)+'.stdout')).write_bytes(r.stdout);(OUT/(str(i)+'.stderr')).write_bytes(r.stderr);commands.append(dict(argv=argv,exit_code=r.returncode));save(OUT/'commands.json',commands);assert r.returncode==0,r.stderr.decode(errors='replace');authorities.append(dict(kind=kind,path=path,catalog_sha256=digest));print(kind,i,'VERIFIED',flush=True)
mutation=ROOT/'docs/phase-2/evidence/mysql-datetime/mutations_20261128_090939_e3cab0d0';digest='1d953c21f5ee167cb6c620b00f79d082549bb5ce219840080d7c31cc9ed56a69';catalog(mutation,digest);sources(mutation);rows=json.loads((mutation/'commands.json').read_bytes());assert len(rows)==8 and all(r['exit_code']==101 for r in rows)
for row in rows:
 label=row['label'];assert b'test result: FAILED. 0 passed; 1 failed;' in (mutation/(label+'.stdout')).read_bytes();assert sha(ROOT/row['source'])==row['original_sha256'] and sha(mutation/(label+'.rs'))==row['mutated_sha256'];assert row['original_sha256']!=row['mutated_sha256']
authorities.append(dict(kind='source-counterexamples',path=mutation.relative_to(ROOT).as_posix(),catalog_sha256=digest))
for v,name in [('1.99','regression_1.99_20261128_091156_e90f6a83'),('1.85','regression_1.85_20261128_091156_138d7698')]:
 p=ROOT/'docs/phase-2/evidence/mysql-datetime'/name;digest=sha(p/'catalog.json');catalog(p,digest);r=json.loads((p/'result.json').read_bytes());assert r['Rust']==v and r['passed']==808 and r['ignored']==398 and r['successful_test_harness_summaries']==184 and r['all_features'] and r['strict_mysql_all_targets_clippy'];cmds=json.loads((p/'commands.json').read_bytes());assert all(x['exit_code']==0 for x in cmds)
 args=next(x['argv'] for x in cmds if x['label']=='clippy');assert all(x in args for x in ['seata-db-mysql','seata-at','seata-datasource','--all-targets','--all-features'])
 summaries=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (p/'workspace.stdout').read_bytes());assert len(summaries)==184 and sum(int(x[0]) for x in summaries)==808 and sum(int(x[2]) for x in summaries)==398 and not any(int(x[1]) for x in summaries)
 for source in r['production_sources']:assert sha(ROOT/source['path'])==source['sha256']
 authorities.append(dict(kind='workspace',path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
baseline=ROOT/'docs/phase-2/evidence/mysql-datetime/java_20261128_085403_e8b42dba';stderr=(baseline/'1.99-commit/client.stderr').read_bytes();assert b'native DATETIME metadata and image must be supported: Unsupported { feature: "column type" }' in stderr
old=subprocess.check_output(['git','show','bf27d819883e63c56be172b68f732b0d8b2373d5:crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs'],cwd=ROOT,creationflags=subprocess.CREATE_NO_WINDOW)
with zipfile.ZipFile(baseline/'source-inputs.zip') as z:assert z.read('crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs')==old
save(OUT/'baseline.json',dict(path=baseline.relative_to(ROOT).as_posix(),exit_code=json.loads((baseline/'1.99-commit/client-command.json').read_bytes())['exit_code'],source_archive_sha256=sha(baseline/'source-inputs.zip'),stderr_sha256=sha(baseline/'1.99-commit/client.stderr'),target_error='Unsupported column type',outer_checkpoint_wait_also_timed_out=False))
save(OUT/'authority.json',authorities);inputs=[]
with zipfile.ZipFile(OUT/'validator-inputs.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted((ROOT/'tools').rglob('*.py')):name=p.relative_to(ROOT).as_posix();z.writestr(name,p.read_bytes());inputs.append(dict(path=name,sha256=sha(p)))
save(OUT/'validator-inputs.json',inputs);shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
relocation=ROOT/'docs/phase-2/evidence/mysql-datetime/data-retention-relocation_20261128.json'
for row in json.loads(relocation.read_bytes()):
 assert row['container_absence_confirmed'] and row['regular_files_moved_and_hash_verified']
 for item in row['files']:assert sha(Path(row['destination'])/item['path'])==item['sha256']
shutil.copyfile(relocation,OUT/relocation.name)
save(OUT/'result.json',dict(status='MYSQL_DATETIME_COLUMN_DUAL_RUST_UTC_VERIFIED',Rust_versions=['1.99','1.85'],original_java_pin='cba73e180fd8896a99df6415b859d275122feaa7',original_connector_j='8.0.27',original_jackson='2.18.3',datetime_global_transactions=8,json_regression_transactions=4,ordinary_at_concurrent_transactions=64,datetime_business_statements=48,gregorian_days_checked_per_toolchain=3287182,original_jdbc=93,original_value_class='java.time.LocalDateTime',explicit_utc_profile_codes=[3,4],legacy_profile_codes_preserved=[1,2],evidence_weakenings_rejected=dict(datetime=160,json_regression=62,ordinary_at_regression=40),source_assertion_counterexamples_rejected=8,workspace_per_rust=dict(passed=808,ignored=398,harness_summaries=184,strict_clippy=True),SQL_grammar_unchanged=True,production_default_changed_commit_enabled=False,DATE_TIME_TIMESTAMP_complete=False,WP05_full_gate_complete=False,whole_plan_complete=False))
save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print('FINAL_CATALOG',sha(OUT/'catalog.json'),flush=True)
