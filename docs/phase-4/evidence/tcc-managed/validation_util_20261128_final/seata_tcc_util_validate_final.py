import sys, json, hashlib, subprocess, shutil, os
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=ROOT/'docs/phase-4/evidence/tcc-managed/validation_util_20261128_final'
OUT.mkdir(exist_ok=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def pair(p):return [str(p),sha(p/'catalog.json')]
core=ROOT/'docs/phase-4/evidence/tcc-core'
managed=ROOT/'docs/phase-4/evidence/tcc-managed'
paths=dict(java_core=core/'java_20261128_002854_2a650ebd',java_mutation=core/'java-mutation_20261128_023439_58ee39f8',mutations=core/'mutations_20261128_044018_2772533d',rust199=core/'regression_1.99_20261128_044018_90406033',rust185=core/'regression_1.85_20261128_044018_db0acdb6',mysql=managed/'mysql_20261128_044018_14dd0f57',process=managed/'process_20261128_044018_2bd16c1c',dynamic=managed/'dynamic_20261128_044018_b9d71850',dynamic_mutations=managed/'dynamic-mutations_20261128_045119_a63344d8',java=managed/'java_20261128_044018_3b2e9189',java_fault=managed/'java-fault_20261128_044018_d17034b7',dynamic_java=managed/'java-dynamic_20261128_044018_ebd1e26f')
paths['java_util']=core/'java-util_20261128_042633_6737e2e6'
records=[]
def run(label,script,args):
    src=ROOT/'tools/formal-source-junit'/script
    shutil.copyfile(src,OUT/script)
    argv=[sys.executable,'-B',str(src),*args]
    value=subprocess.run(argv,cwd=ROOT,env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1'),capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (OUT/(label+'.stdout')).write_bytes(value.stdout);(OUT/(label+'.stderr')).write_bytes(value.stderr)
    records.append(dict(label=label,argv=argv,exit_code=value.returncode,validator_sha256=sha(src)))
    (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    print(label,value.returncode,value.stdout.decode(errors='replace'),value.stderr.decode(errors='replace')[-1500:],flush=True)
    if value.returncode:raise RuntimeError(label+' failed')
def flags(names):
    result=[]
    for name,key in names:
        p,d=pair(paths[key]);result += ['--'+name,p,'--'+name+'-sha',d]
    return result
run('java-util','validate_tcc_util.py',pair(paths['java_util'])+['--weakenings'])
run('core','validate_tcc_core.py',flags([('java','java_core'),('mutations','mutations'),('rust199','rust199'),('rust185','rust185')]))
run('java-mutation','validate_tcc_context_mutation.py',pair(paths['java_mutation'])+['--weakenings'])
run('process','validate_tcc_process.py',pair(paths['process'])+['--weakenings'])
if '--all' in sys.argv:
    run('managed','validate_tcc_managed.py',flags([('mysql','mysql'),('java','java'),('rust199','rust199'),('rust185','rust185')])+['--weakenings'])
    run('java-fault','validate_tcc_java_faults.py',pair(paths['java_fault'])+['--weakenings'])
    run('dynamic','validate_tcc_dynamic.py',flags([('mysql','dynamic'),('java','dynamic_java')])+['--weakenings'])
    if '--checks' in sys.argv:sys.exit(0)
    src=ROOT/'tools/formal-source-junit/validate_tcc_dynamic.py'
    code="import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from validate_tcc_dynamic import verify_mutations;verify_mutations(Path(sys.argv[2]),sys.argv[3]);print('Verified twelve actual behavioral dynamic source counterexamples')"
    argv=[sys.executable,'-B','-c',code,str(src.parent),*pair(paths['dynamic_mutations'])]
    value=subprocess.run(argv,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (OUT/'dynamic-mutations.stdout').write_bytes(value.stdout);(OUT/'dynamic-mutations.stderr').write_bytes(value.stderr)
    records.append(dict(label='dynamic-mutations',argv=argv,exit_code=value.returncode,validator_sha256=sha(src)))
    (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    print('dynamic-mutations',value.returncode,value.stdout.decode(),value.stderr.decode(),flush=True)
    assert value.returncode==0
    shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
    (OUT/'authority.json').write_text(json.dumps({k:dict(path=v.relative_to(ROOT).as_posix(),catalog_sha256=sha(v/'catalog.json')) for k,v in paths.items()},indent=2)+'\n',encoding='utf-8')
    (OUT/'result.json').write_text(json.dumps(dict(status='ALL_SELECTED_UTIL_DYNAMIC_AND_REGRESSION_EVIDENCE_VERIFIED',whole_plan_complete=False,TCC_full_gate_complete=False),indent=2)+'\n',encoding='utf-8')
    (OUT/'catalog.json').write_text(json.dumps([dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json'],indent=2)+'\n',encoding='utf-8')
    print('VALIDATION_CATALOG',sha(OUT/'catalog.json'))
