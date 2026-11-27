import sys, json, hashlib, subprocess, shutil, os
from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
OUT=ROOT/'docs/phase-4/evidence/tcc-managed/validation_configuration_20261128_final'
OUT.mkdir(exist_ok=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def pair(p):return [str(p),sha(p/'catalog.json')]
core=ROOT/'docs/phase-4/evidence/tcc-core'
managed=ROOT/'docs/phase-4/evidence/tcc-managed'
paths=dict(java_core=core/'java_20261128_002854_2a650ebd',java_mutation=core/'java-mutation_20261128_023439_58ee39f8',mutations=core/'mutations_20261128_051031_07579471',rust199=core/'regression_1.99_20261128_060128_7432dcd2',rust185=core/'regression_1.85_20261128_060128_4aa5ed86',mysql=managed/'mysql_20261128_060128_c41ee82d',process=managed/'process_20261128_061252_ec601757',dynamic=managed/'dynamic_20261128_060128_3275a06c',dynamic_mutations=managed/'dynamic-mutations_20261128_051550_81babb78',java=managed/'java_20261128_061252_881d3cf7',java_fault=managed/'java-fault_20261128_061252_bbdf17ad',dynamic_java=managed/'java-dynamic_20261128_061252_38da0e84')
paths['java_util']=core/'java-util_20261128_042633_6737e2e6'
paths['joint']=managed/'java-dynamic-fault_20261128_061252_99ff9d14'
paths.pop('dynamic_mutations')
paths['configuration']=managed/'configuration_20261128_060128_98028e41'
paths['configuration_mutations']=managed/'configuration-mutations_20261128_061341_f39c5943'
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
    run('joint','validate_tcc_dynamic_faults.py',pair(paths['joint'])+['--weakenings'])
    if '--checks' in sys.argv:sys.exit(0)
    run('configuration','validate_tcc_configuration.py',pair(paths['configuration'])+['--weakenings'])
    run('configuration-mutations','validate_tcc_configuration.py',pair(paths['configuration_mutations'])+['--mutations'])
    type_code="import sys,json;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from validate_tcc_dynamic import expected_app;root=Path(sys.argv[2]);a=json.loads((root/'1.99-delayed-report_lost-rollback/observations/proof/original-java-tcc-dynamic-fault.json').read_bytes())['reported_application_data'];b=json.loads((root/'1.99-immediate-duplicate-commit/observations/proof/original-java-tcc-dynamic-fault.json').read_bytes())['reported_application_data'];changes=[(a.replace('\"useTCCFence\":true','\"useTCCFence\":1'),'delayed'),(a.replace('\"id\":1','\"id\":true'),'delayed'),(a.replace('\"id\":1','\"id\":1.0'),'delayed'),(a.replace('\"amount\":9','\"amount\":9.0'),'delayed'),(b.replace('\"final\":9','\"final\":9.0'),'immediate')];count=0\nfor app,mode in changes:\n try:expected_app(app,mode,True)\n except ValueError:count+=1\nassert count==5;print('Rejected five JSON bool/integer type substitutions')"
    argv=[sys.executable,'-B','-c',type_code,str(ROOT/'tools/formal-source-junit'),str(paths['joint'])]
    value=subprocess.run(argv,cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (OUT/'report-types.stdout').write_bytes(value.stdout);(OUT/'report-types.stderr').write_bytes(value.stderr)
    records.append(dict(label='report-types',argv=argv,exit_code=value.returncode,validator_sha256=sha(ROOT/'tools/formal-source-junit/validate_tcc_dynamic.py')))
    (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    assert value.returncode==0,(value.stdout,value.stderr)
    fixture=ROOT/'tools/formal-rm-java/tcc_dynamic_proxy_frames.json'
    data=json.loads(fixture.read_bytes());old_case=ROOT/data['source_case']
    assert sha(old_case.parent.parent/'catalog.json')==data['source_catalog_sha256']
    events=[json.loads(line) for line in old_case.read_bytes().splitlines()]
    for kind,raw in data['frames'].items():
        assert any(row.get('event')=='frame' and row.get('kind')==int(kind) and row['frame_hex']==raw for row in events)
    proxy=ROOT/'tools/formal-rm-java/test_tcc_dynamic_proxy.py'
    argv=[sys.executable,'-B',str(proxy)]
    value=subprocess.run(argv,cwd=ROOT,env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',PYTHONDONTWRITEBYTECODE='1'),capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (OUT/'proxy-rules.stdout').write_bytes(value.stdout);(OUT/'proxy-rules.stderr').write_bytes(value.stderr)
    records.append(dict(label='proxy-rules',argv=argv,exit_code=value.returncode,source_sha256=sha(proxy)))
    (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    assert value.returncode==0 and b'Ran 5 tests' in value.stderr and b'OK' in value.stderr
    inputs=[]
    for name in ['test_tcc_dynamic_proxy.py','tcc_dynamic_proxy.py','tcc_dynamic_proxy_frames.json','rm_proxy.py']:
        source=ROOT/'tools/formal-rm-java'/name;shutil.copyfile(source,OUT/name);inputs.append(dict(path=source.relative_to(ROOT).as_posix(),sha256=sha(source)))
    (OUT/'proxy-inputs.json').write_text(json.dumps(inputs,indent=2)+'\n',encoding='utf-8')
    shutil.copyfile(Path(__file__),OUT/Path(__file__).name)
    (OUT/'authority.json').write_text(json.dumps({k:dict(path=v.relative_to(ROOT).as_posix(),catalog_sha256=sha(v/'catalog.json')) for k,v in paths.items()},indent=2)+'\n',encoding='utf-8')
    (OUT/'result.json').write_text(json.dumps(dict(status='ALL_SELECTED_CONFIGURATION_TRANSITIONS_AND_CURRENT_REGRESSIONS_VERIFIED',whole_plan_complete=False,TCC_full_gate_complete=False),indent=2)+'\n',encoding='utf-8')
    (OUT/'catalog.json').write_text(json.dumps([dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json'],indent=2)+'\n',encoding='utf-8')
    print('VALIDATION_CATALOG',sha(OUT/'catalog.json'))
