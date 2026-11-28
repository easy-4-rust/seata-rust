from pathlib import Path
import sys,json,subprocess,re,zipfile,shutil
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
from inventory_rebuilt import save,sha
from validate_structured_logs import catalog
OUT=ROOT/'docs/phase-2/evidence/mysql-timestamp-restart/validation_20261128_final'
NATIVE=ROOT/'docs/phase-2/evidence/mysql-timestamp-restart/java_20261128_101908_c8c94331'
assert (NATIVE/'catalog.json').is_file(),'Final owned native process matrix is not terminal'
OUT.mkdir(exist_ok=True);authorities=[]
digest=sha(NATIVE/'catalog.json');argv=[sys.executable,'-B',str(ROOT/'tools/formal-source-junit/validate_timestamp_restart.py'),str(NATIVE),digest,'--weakenings']
r=subprocess.run(argv,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);(OUT/'validator.stdout').write_bytes(r.stdout);(OUT/'validator.stderr').write_bytes(r.stderr);save(OUT/'command.json',dict(argv=argv,exit_code=r.returncode));assert r.returncode==0 and b'weakenings rejected: 264' in r.stdout,r.stderr.decode(errors='replace')
authorities.append(dict(kind='original-tc-process-recovery',path=NATIVE.relative_to(ROOT).as_posix(),catalog_sha256=digest))
for v,name in [('1.99','regression_1.99_20261128_101819_0d7fb5d8'),('1.85','regression_1.85_20261128_101819_8b43d61d')]:
 p=ROOT/'docs/phase-2/evidence/mysql-timestamp-restart'/name;digest=sha(p/'catalog.json');catalog(p,digest);r=json.loads((p/'result.json').read_bytes());assert r['Rust']==v and r['passed']==812 and r['ignored']==403 and r['successful_test_harness_summaries']==187 and r['all_features'] and r['strict_mysql_all_targets_clippy'];commands=json.loads((p/'commands.json').read_bytes());assert all(c['exit_code']==0 for c in commands)
 summaries=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (p/'workspace.stdout').read_bytes());assert len(summaries)==187 and sum(int(x[0]) for x in summaries)==812 and sum(int(x[2]) for x in summaries)==403 and not any(int(x[1]) for x in summaries)
 args=next(c['argv'] for c in commands if c['label']=='clippy');assert all(s in args for s in ['seata-db-mysql','seata-at','seata-datasource','seata-observability','--all-targets','--all-features','-D','warnings'])
 for source in r['production_sources']:assert sha(ROOT/source['path'])==source['sha256']
 authorities.append(dict(kind='workspace',path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
p=ROOT/'docs/phase-2/evidence/mysql-timestamp-restart/reset_controls_20261128_101417_f3d0179e';digest='4accd5ae272189563c895907c5921b208c0862c4994d74a1210dccde431ee096';catalog(p,digest);assert sha(p/'timestamp_restart_proxy.py')==sha(ROOT/'tools/formal-source-junit/timestamp_restart_proxy.py');r=json.loads((p/'result.json').read_bytes());assert r['positive_cases']==2 and r['rejected']==11 and len(r['negative_cases'])==11 and all(c['accepted'] is False for c in r['negative_cases']);authorities.append(dict(kind='reset-authorization',path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
changed=subprocess.check_output(['git','diff','--name-only','d30dae995eaf699eeb2ad8ba5dca0f1a5dbf41a9','--','crates','Cargo.toml','Cargo.lock'],cwd=ROOT,creationflags=subprocess.CREATE_NO_WINDOW).decode().splitlines()
untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','crates'],cwd=ROOT,creationflags=subprocess.CREATE_NO_WINDOW).decode().splitlines();assert set(changed)|set(untracked)=={'crates/seata_db_mysql/tests/live_timestamp_restart_java.rs','crates/seata_db_mysql/tests/support/timestamp_owned_process.rs'},(changed,untracked)
for name in ['retained-data-compression_20261128.json','diagnostic-data-compression_20261128.json']:
 p=ROOT/'docs/phase-2/evidence/mysql-timestamp-restart'/name;r=json.loads(p.read_bytes());assert r['status']=='STOPPED_OWNED_DATA_LOGICAL_BYTES_PRESERVED'
 for item in r['files']:assert item['compact_exit']==0 and sha(Path(item['path']))==item['sha256']
 shutil.copyfile(p,OUT/name)
inputs=[]
with zipfile.ZipFile(OUT/'validator-inputs.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted((ROOT/'tools').rglob('*.py')):name=p.relative_to(ROOT).as_posix();z.writestr(name,p.read_bytes());inputs.append(dict(path=name,sha256=sha(p)))
save(OUT/'validator-inputs.json',inputs);save(OUT/'authority.json',authorities);shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
save(OUT/'result.json',dict(status='MYSQL_TIMESTAMP_FORCED_PROCESS_RESTART_ORIGINAL_TC_VERIFIED',Rust_versions=['1.99','1.85'],actual_global_transactions=8,actual_rust_process_instances=24,forced_business_process_deaths=8,replacement_rm_processes=8,process_OS_observation_verified=True,persisted_original_branch_reconstructed=True,callbacks_on_new_connection_only=True,timestamp_process_restart_recovery_verified=True,TC_crashed=False,production_Rust_changed=False,production_baseline_git='d30dae995eaf699eeb2ad8ba5dca0f1a5dbf41a9',explicit_utc_profile_codes=[3,4],legacy_profile_codes_preserved=[1,2],rejected_evidence_weakenings=264,rejected_reset_authorizations=11,workspace_per_rust=dict(passed=812,ignored=403,harness_summaries=187,strict_clippy=True),production_default_changed_commit_enabled=False,DATE_TIME_TIMESTAMP_complete=False,WP05_full_gate_complete=False,whole_plan_complete=False))
save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print('FINAL_CATALOG',sha(OUT/'catalog.json'),flush=True)
