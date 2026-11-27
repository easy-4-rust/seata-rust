import sys,subprocess,os,json,hashlib,shutil,re
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=ROOT/'docs/phase-4/evidence/tcc-managed/validation_dynamic_process_20261128_final';OUT.mkdir(exist_ok=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,x):p.write_bytes((json.dumps(x,ensure_ascii=False,indent=2)+'\n').encode())
def pair(p):return [str(p),sha(p/'catalog.json')]
managed=ROOT/'docs/phase-4/evidence/tcc-managed';core=ROOT/'docs/phase-4/evidence/tcc-core'
paths=dict(dynamic=managed/'dynamic-process_20261128_064454_747d5fe1',static=managed/'process_20261128_064454_ab967a2f',rust199=core/'regression_1.99_20261128_064454_5fa6fa77',rust185=core/'regression_1.85_20261128_064454_2a0bc501',mutations=core/'mutations_20261128_051031_07579471',java=core/'java_20261128_002854_2a650ebd')
commands=[]
def run(label,argv):
 value=subprocess.run([sys.executable,'-B',*map(str,argv)],cwd=ROOT,env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1'),capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
 (OUT/(label+'.stdout')).write_bytes(value.stdout);(OUT/(label+'.stderr')).write_bytes(value.stderr)
 commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=value.returncode));save(OUT/'commands.json',commands)
 print(label,value.returncode,value.stdout.decode(errors='replace'),value.stderr.decode(errors='replace')[-1500:],flush=True);assert value.returncode==0
run('dynamic', [ROOT/'tools/formal-source-junit/validate_tcc_dynamic_process.py',*pair(paths['dynamic']),'--weakenings'])
flags=[]
for key in ['java','mutations','rust199','rust185']:
 p,digest=pair(paths[key]);flags+=['--'+key,p,'--'+key+'-sha',digest]
run('core',[ROOT/'tools/formal-source-junit/validate_tcc_core.py',*flags])
code="import sys,json;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from validate_tcc_process import check_case;from validate_structured_logs import catalog,sources;p=Path(sys.argv[2]);catalog(p,sys.argv[3]);sources(p);r=json.loads((p/'result.json').read_bytes());assert r['Rust_versions']==['1.99','1.85'] and r['database_fault_modes']==['none'] and r['selected_case']=='none';assert json.loads((p/'cleanup.json').read_bytes())['owned_container_removed'];assert all(x['exit_code']==(1 if x['label']=='confirm-removed' else 0) for x in json.loads((p/'commands.json').read_bytes()));[check_case(p,v,0,i) for v in ['1.99','1.85'] for i in range(9)];print('Verified eighteen unchanged static TCC writer-death regressions')"
run('static',['-c',code,ROOT/'tools/formal-source-junit',*pair(paths['static'])])
value=subprocess.run(['git','diff','e243a2cf7d9630a5d11b2fd68fefc89754803d84','--','crates/*/src','Cargo.toml','Cargo.lock','crates/*/migrations'],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);assert value.returncode==0 and not value.stdout
(OUT/'production-unchanged.stdout').write_bytes(value.stdout)
for name in ['validate_tcc_dynamic_process.py','validate_tcc_core.py','validate_tcc_process.py','validate_tcc_dynamic.py','validate_structured_logs.py']:
 src=ROOT/'tools/formal-source-junit'/name;shutil.copyfile(src,OUT/name)
shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
save(OUT/'authority.json',{k:dict(path=v.relative_to(ROOT).as_posix(),catalog_sha256=sha(v/'catalog.json')) for k,v in paths.items()})
save(OUT/'result.json',dict(status='SELECTED_DYNAMIC_PROCESS_DATABASE_RECOVERY_VERIFIED',actual_dynamic_cases=60,evidence_weakenings_rejected=1312,static_cases=18,production_unchanged_from='e243a2cf7d9630a5d11b2fd68fefc89754803d84',TCC_full_gate_complete=False,whole_plan_complete=False))
save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json'])
print('VALIDATION_CATALOG',sha(OUT/'catalog.json'))
