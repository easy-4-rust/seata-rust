from pathlib import Path
import sys,json,subprocess,re,zipfile,shutil
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=ROOT/'docs/phase-2/evidence/mysql-timestamp/validation_20261128_final'
sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
from inventory_rebuilt import save,sha
from validate_structured_logs import catalog
runs=[('timestamp','docs/phase-2/evidence/mysql-timestamp/java_20261128_094447_15187547','61c26d40263f8428cba84f5b002567f61276952469524ef833240edbac8160d9'),('datetime','docs/phase-2/evidence/mysql-datetime/java_20261128_094754_3a05c258','a757bae0121a87ee0a272e0af340eb45a0cfb84f452fd1d0602a91d7f9e3bf6b'),('json','docs/phase-2/evidence/mysql-json/java_20261128_095121_47583850','3c98fa239b01b6e7ffd7c4b14af93d3495a27d49b3837a0ae63fcabfc6d9567b'),('at','docs/phase-2/evidence/at-concurrent-load/java_20261128_095319_affcbf57',None)]
assert all((ROOT/p/'catalog.json').is_file() for _,p,_ in runs),'All owned runtime suites must finish before validation'
OUT.mkdir(exist_ok=True);commands=[];authorities=[]
for i,(kind,path,digest) in enumerate(runs):
 digest=digest or sha(ROOT/path/'catalog.json')
 script={'timestamp':'validate_timestamp_column.py','datetime':'validate_datetime_column.py','json':'validate_json_column.py','at':'validate_at_concurrent.py'}[kind]
 argv=[sys.executable,'-B',str(ROOT/'tools/formal-source-junit'/script),str(ROOT/path),digest,'--weakenings']
 r=subprocess.run(argv,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
 (OUT/(str(i)+'.stdout')).write_bytes(r.stdout);(OUT/(str(i)+'.stderr')).write_bytes(r.stderr);commands.append(dict(argv=argv,exit_code=r.returncode));save(OUT/'commands.json',commands)
 assert r.returncode==0,r.stderr.decode(errors='replace');authorities.append(dict(kind=kind,path=path,catalog_sha256=digest));print(kind,'VERIFIED',flush=True)
for v,name in [('1.99','regression_1.99_20261128_094505_3790548b'),('1.85','regression_1.85_20261128_094505_aa970e29')]:
 p=ROOT/'docs/phase-2/evidence/mysql-timestamp'/name;digest=sha(p/'catalog.json');catalog(p,digest);r=json.loads((p/'result.json').read_bytes());assert r['Rust']==v and r['passed']==812 and r['ignored']==400 and r['successful_test_harness_summaries']==186 and r['all_features'] and r['strict_mysql_all_targets_clippy'];cmds=json.loads((p/'commands.json').read_bytes());assert all(x['exit_code']==0 for x in cmds)
 args=next(x['argv'] for x in cmds if x['label']=='clippy');assert all(x in args for x in ['seata-db-mysql','seata-at','seata-datasource','--all-targets','--all-features'])
 summaries=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (p/'workspace.stdout').read_bytes());assert len(summaries)==186 and sum(int(x[0]) for x in summaries)==812 and sum(int(x[2]) for x in summaries)==400 and not any(int(x[1]) for x in summaries)
 for source in r['production_sources']:assert sha(ROOT/source['path'])==source['sha256']
 authorities.append(dict(kind='workspace',path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
baseline=ROOT/'docs/phase-2/evidence/mysql-timestamp/tdd_20261128'
assert json.loads((baseline/'red-command.json').read_bytes())['exit_code']==101
assert b'TIMESTAMP must have a native identity' in (baseline/'red.stdout').read_bytes() and b'test result: FAILED. 0 passed; 1 failed;' in (baseline/'red.stdout').read_bytes()
old=subprocess.check_output(['git','show','958935aecbbaebb92b86dd84f3154aeb881403c1:crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs'],cwd=ROOT,creationflags=subprocess.CREATE_NO_WINDOW).decode().replace('\r\n','\n').rstrip()
pre=(baseline/'before-mysql_table_meta_reader.rs').read_bytes().decode().replace('\r\n','\n');assert pre.split('#[cfg(test)]\nmod timestamp_tests')[0].rstrip()==old
assert b'timestamp_is_a_distinct_native_type ... ok' in (ROOT/'docs/phase-2/evidence/mysql-timestamp/regression_1.99_20261128_094505_3790548b/workspace.stdout').read_bytes()
save(OUT/'baseline.json',dict(path=baseline.relative_to(ROOT).as_posix(),exit_code=101,target='missing native TIMESTAMP identity',before_source_sha256=sha(baseline/'before-mysql_table_meta_reader.rs'),source_before_test_matches_git='958935aecbbaebb92b86dd84f3154aeb881403c1',is_database_runtime_counterexample=False))
save(baseline/'catalog.json',[dict(path=p.relative_to(baseline).as_posix(),sha256=sha(p)) for p in sorted(baseline.rglob('*')) if p.is_file() and p.name!='catalog.json']);authorities.append(dict(kind='tdd',path=baseline.relative_to(ROOT).as_posix(),catalog_sha256=sha(baseline/'catalog.json')))
compression=ROOT/'docs/phase-2/evidence/mysql-timestamp/retained-data-compression_20261128.json';c=json.loads(compression.read_bytes());assert c['status']=='STOPPED_OWNED_DATA_LOGICAL_BYTES_PRESERVED'
for row in c['files']:assert row['path'].startswith('E:\\codex-builds\\seata-datetime-20261128_091126_') and sha(Path(row['path']))==row['sha256'] and row['compact_exit']==0
shutil.copyfile(compression,OUT/compression.name);save(OUT/'authority.json',authorities)
inputs=[]
with zipfile.ZipFile(OUT/'validator-inputs.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted((ROOT/'tools').rglob('*.py')):name=p.relative_to(ROOT).as_posix();z.writestr(name,p.read_bytes());inputs.append(dict(path=name,sha256=sha(p)))
save(OUT/'validator-inputs.json',inputs);shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
save(OUT/'result.json',dict(status='MYSQL_TIMESTAMP_NATIVE_AND_ORIGINAL_TC_UTC_VERIFIED',Rust_versions=['1.99','1.85'],original_java_pin='cba73e180fd8896a99df6415b859d275122feaa7',timestamp_global_transactions=8,datetime_regression_transactions=8,json_regression_transactions=4,ordinary_at_concurrent_transactions=64,timestamp_business_statements=48,actual_timestamp_session_cases=24,non_utc_session_refusals=16,original_jdbc=93,original_value_class='java.sql.Timestamp',explicit_utc_profile_codes=[3,4],legacy_profile_codes_preserved=[1,2],evidence_weakenings_rejected=dict(timestamp=160,datetime=160,json=62,ordinary_at=40),workspace_per_rust=dict(passed=812,ignored=400,harness_summaries=186,strict_clippy=True),SQL_grammar_unchanged=True,production_default_changed_commit_enabled=False,timestamp_process_restart_recovery_verified=False,TIMESTAMP_full_gate_complete=False,DATE_TIME_TIMESTAMP_complete=False,WP05_full_gate_complete=False,whole_plan_complete=False))
save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print('FINAL_CATALOG',sha(OUT/'catalog.json'),flush=True)
