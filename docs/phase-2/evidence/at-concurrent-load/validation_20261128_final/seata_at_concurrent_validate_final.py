import json,hashlib,subprocess,sys,zipfile,shutil,re
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=ROOT/'docs/phase-2/evidence/at-concurrent-load/validation_20261128_final';OUT.mkdir(exist_ok=True)
sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
from validate_structured_logs import catalog
from inventory_rebuilt import sha,save
runs=[('java_20261128_080815_95498765','d8275176c46d66853d34d6bb3dff0fe2a52c58f66fb64597276a949f4b0ef615'),('java_20261128_080815_469d9856','fd000a890cc4f737676d5ce22abc5aa638869fde0be48e52ce357d7d1439b4b7')]
commands=[];measurements=[];rejected=0;authorities=[]
for name,digest in runs:
 p=OUT.parent/name
 args=[sys.executable,'-B',str(ROOT/'tools/formal-source-junit/validate_at_concurrent.py'),str(p),digest,'--weakenings','--report',str(OUT/(name+'.json'))]
 r=subprocess.run(args,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);(OUT/(name+'.stdout')).write_bytes(r.stdout);(OUT/(name+'.stderr')).write_bytes(r.stderr);commands.append(dict(argv=args,exit_code=r.returncode));save(OUT/'commands.json',commands);assert r.returncode==0,r.stderr.decode(errors='replace')
 result=json.loads((OUT/(name+'.json')).read_bytes());assert result['status']=='AT_CONCURRENT_EVIDENCE_VERIFIED' and result['evidence_weakenings_rejected']==60;rejected+=result['evidence_weakenings_rejected'];measurements+=result['measurements'];authorities.append(dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
for v,name in [('1.99','regression_1.99_20261128_080838_5c555118'),('1.85','regression_1.85_20261128_080838_d327e9a6')]:
 p=ROOT/'docs/phase-2/evidence/mysql-phase-one-metrics'/name;digest=sha(p/'catalog.json');catalog(p,digest);r=json.loads((p/'result.json').read_bytes());assert r['Rust']==v and r['passed']==801 and r['ignored']==396 and r['successful_test_harness_summaries']==180 and r['all_features'] and r['strict_mysql_all_targets_clippy'];assert all(x['exit_code']==0 for x in json.loads((p/'commands.json').read_bytes()))
 summary=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (p/'workspace.stdout').read_bytes());assert len(summary)==180 and sum(int(x[0]) for x in summary)==801 and sum(int(x[2]) for x in summary)==396 and not any(int(x[1]) for x in summary)
 for source in r['production_sources']:assert sha(ROOT/source['path'])==source['sha256']
 authorities.append(dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=digest))
assert len(measurements)==6 and sum(x['transactions'] for x in measurements)==208 and rejected==120
r=subprocess.run(['git','diff','a11cac1a74f83fe8e02fa31ff7449ef7892c6852','--','crates/*/src','Cargo.toml','Cargo.lock','crates/*/migrations'],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);assert r.returncode==0 and not r.stdout;(OUT/'production-unchanged.stdout').write_bytes(r.stdout)
inputs=[]
with zipfile.ZipFile(OUT/'validator-inputs.zip','w',zipfile.ZIP_DEFLATED) as archive:
 for p in sorted((ROOT/'tools').rglob('*.py')):
  name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());inputs.append(dict(path=name,sha256=sha(p)))
save(OUT/'validator-inputs.json',inputs);save(OUT/'authority.json',authorities);save(OUT/'result.json',dict(status='AT_CONCURRENT_DUAL_RUST_MATRIX_VERIFIED',actual_global_transactions=208,business_statements=624,overlap_rounds=48,evidence_weakenings_rejected=120,initial_rollback_retrying=sum(x['rollback_retrying'] for x in measurements),measurements=measurements,workspace_per_rust=dict(passed=801,ignored=396,harness_summaries=180,strict_clippy=True),production_unchanged_from='a11cac1a74f83fe8e02fa31ff7449ef7892c6852',production_capacity_certified=False,whole_plan_complete=False))
shutil.copyfile(Path(__file__),OUT/Path(__file__).name);save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print(json.dumps(dict(evidence=str(OUT),catalog_sha256=sha(OUT/'catalog.json'))))
