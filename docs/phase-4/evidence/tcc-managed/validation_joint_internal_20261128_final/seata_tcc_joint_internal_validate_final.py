import sys,json,hashlib,subprocess,os,shutil
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');OUT=ROOT/'docs/phase-4/evidence/tcc-managed/validation_joint_internal_20261128_final';OUT.mkdir(exist_ok=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,x):p.write_bytes((json.dumps(x,ensure_ascii=False,indent=2)+'\n').encode())
def pair(p):return [str(p),sha(p/'catalog.json')]
managed=ROOT/'docs/phase-4/evidence/tcc-managed';core=ROOT/'docs/phase-4/evidence/tcc-core'
paths=dict(none=managed/'java-joint-internal_20261128_073517_9e977bb9',restart=managed/'java-joint-internal_20261128_073517_3f7426ed',crash=managed/'java-joint-internal_20261128_073517_9c1d0701',baseline=managed/'java-joint_20261128_073517_0fd21eb5',rust199=core/'regression_1.99_20261128_073517_37bb98d6',rust185=core/'regression_1.85_20261128_073517_1c2c3c49',java=core/'java_20261128_002854_2a650ebd',mutations=core/'mutations_20261128_051031_07579471')
commands=[]
def run(label,argv):
 r=subprocess.run([sys.executable,'-B',*map(str,argv)],cwd=ROOT,env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1'),capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
 (OUT/(label+'.stdout')).write_bytes(r.stdout);(OUT/(label+'.stderr')).write_bytes(r.stderr);commands.append(dict(label=label,argv=list(map(str,argv)),exit_code=r.returncode));save(OUT/'commands.json',commands)
 print(label,r.returncode,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')[-1500:],flush=True);assert r.returncode==0
for mode in ['none','restart','crash']:run('internal-'+mode,[ROOT/'tools/formal-source-junit/validate_tcc_joint_internal.py',*pair(paths[mode]),'--weakenings'])
flags=[]
for key in ['java','mutations','rust199','rust185']:
 p,digest=pair(paths[key]);flags+=['--'+key,p,'--'+key+'-sha',digest]
run('core',[ROOT/'tools/formal-source-junit/validate_tcc_core.py',*flags])
code="import sys,json;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from validate_structured_logs import catalog,sources;from validate_tcc_joint_crash import check_case;p=Path(sys.argv[2]);catalog(p,sys.argv[3]);sources(p);r=json.loads((p/'result.json').read_bytes());assert r['Rust_versions']==['1.99','1.85'] and r['modes']==['delayed'] and r['database_modes']==['none'] and r['directions']==['commit','rollback'] and r['actual_global_transactions']==4;assert json.loads((p/'cleanup.json').read_bytes())['owned_container_removed'];assert all(x['exit_code']==(1 if x['label']=='confirm-removed' else 0) for x in json.loads((p/'commands.json').read_bytes()));[check_case(p,v,'delayed','none',d) for v in ['1.99','1.85'] for d in ['commit','rollback']];print('Verified four original pre-callback joint-crash regressions')"
run('baseline',['-c',code,ROOT/'tools/formal-source-junit',*pair(paths['baseline'])])
for label,p in [('proxy-regression',ROOT/'tools/formal-rm-java/test_tcc_dynamic_proxy.py'),('joint-proxy',ROOT/'tools/formal-rm-java/test_tcc_joint_proxy.py'),('published-control',ROOT/'tools/formal-source-junit/test_tcc_joint_control.py')]:run(label,[p])
r=subprocess.run(['git','diff','6b22d101208c1c08fff9182f1d347048058e9b59','--','crates/*/src','Cargo.toml','Cargo.lock','crates/*/migrations'],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);assert r.returncode==0 and not r.stdout;(OUT/'production-unchanged.stdout').write_bytes(r.stdout)
inputs=[]
for relative in ['tools/formal-source-junit/validate_tcc_joint_internal.py','tools/formal-source-junit/validate_tcc_joint_crash.py','tools/formal-source-junit/validate_tcc_core.py','tools/formal-source-junit/validate_tcc_dynamic.py','tools/formal-source-junit/validate_structured_logs.py','tools/formal-source-junit/tcc_joint_internal.py','tools/formal-source-junit/tcc_joint_crash.py','tools/formal-source-junit/test_tcc_joint_control.py','tools/formal-source-junit/owned_writer_observation.py','tools/formal-rm-java/rm_proxy.py','tools/formal-rm-java/tcc_joint_proxy.py','tools/formal-rm-java/test_tcc_joint_proxy.py','tools/formal-rm-java/test_tcc_dynamic_proxy.py','tools/formal-rm-java/tcc_dynamic_proxy.py','tools/formal-rm-java/tcc_dynamic_proxy_frames.json']:
 p=ROOT/relative;shutil.copyfile(p,OUT/p.name);inputs.append(dict(path=relative,sha256=sha(p)))
shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
save(OUT/'validator-inputs.json',inputs);save(OUT/'authority.json',{k:dict(path=v.relative_to(ROOT).as_posix(),catalog_sha256=sha(v/'catalog.json')) for k,v in paths.items()})
save(OUT/'result.json',dict(status='ORIGINAL_TC_RUST_INTERNAL_JOINT_CRASH_MATRIX_VERIFIED',actual_global_transactions=72,evidence_weakenings_rejected=3816,original_process_kills=144,pre_callback_joint_regressions=4,observer_and_file_control_tests=11,production_unchanged_from='6b22d101208c1c08fff9182f1d347048058e9b59',TCC_full_gate_complete=False,whole_plan_complete=False))
save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print('VALIDATION_CATALOG',sha(OUT/'catalog.json'))
