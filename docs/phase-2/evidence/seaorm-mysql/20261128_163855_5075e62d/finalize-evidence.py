import os,subprocess,json,re,hashlib,sys
from pathlib import Path
root=Path.cwd();out=root/'docs/phase-2/evidence/seaorm-mysql/20261128_163855_5075e62d';reg=Path('D:/codex-builds/seata-statement-result-regression-20261128')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v):p.write_text(json.dumps(v,indent=2,ensure_ascii=False),encoding='utf-8')
env=os.environ|dict(CARGO_HOME=str(root.parent/'.seata-cargo-cache'),CARGO_TARGET_DIR=str(root.parent/'.seata-target-nodebug'),TEMP='D:/Temp',TMP='D:/Temp',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2')
records=json.loads((reg/'result.json').read_bytes());assert len(records)==4 and all(r['exit_code']==0 for r in records)
for row in json.loads((out/'source-inputs.json').read_bytes()):assert sha(root/row['path'])==row['sha256']
components=[];commands=[]
def command(name,args,expected=0):
 p=subprocess.run(list(map(str,args)),cwd=root,env=env,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
 (out/(name+'.log')).write_bytes(p.stdout+p.stderr);row=dict(label=name,argv=list(map(str,args)),exit_code=p.returncode);commands.append(row);save(out/'component-commands.json',commands)
 assert p.returncode==expected,(name,p.returncode,(p.stdout+p.stderr)[-2000:]);return p
for version in ('1.99','1.85'):
 env['CARGO_TARGET_DIR']=str(root.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
 runs={r['label']:r for r in records if r['version']==version};assert set(runs)=={'workspace','clippy'}
 logs=[]
 for label,suffix in [('workspace','workspace'),('clippy','clippy')]:
  name='component-'+version+'-'+suffix+'.log';p=out/name;p.write_bytes((reg/(version+'-'+label+'.stdout')).read_bytes()+(reg/(version+'-'+label+'.stderr')).read_bytes());logs.append(dict(path=name,sha256=sha(p)))
 args=['cargo','+'+version+'.0','test','--offline','--locked','-p','seata-orm-seaorm','--all-features'];name='component-'+version+'-tests'
 p=command(name,args);rows=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',p.stdout);totals=[sum(int(r[i]) for r in rows) for i in range(3)];assert totals==[7,0,1],totals
 logs.append(dict(path=name+'.log',sha256=sha(out/(name+'.log'))))
 components.append(dict(version=version,exit_code=0,component=dict(passed=7,failed=0,ignored=1),workspace=dict(passed=runs['workspace']['passed'],failed=0,ignored=runs['workspace']['ignored']),logs=logs,test_argv=args,workspace_argv=runs['workspace']['argv'],clippy_argv=runs['clippy']['argv']))
save(out/'component-results.json',components)
env['CARGO_TARGET_DIR']=str(root.parent/'.seata-target-nodebug')
command('seaorm-tree-seata-db-mysql',['cargo','+1.99.0','tree','--offline','--locked','-p','seata-db-mysql','-e','normal'])
command('seaorm-tree-sea-orm',['cargo','+1.99.0','tree','--offline','--locked','-p','sea-orm','-e','normal'])
command('seaorm-workspace-normal-tree',['cargo','+1.99.0','tree','--offline','--locked','--workspace','--all-features','-e','normal'])
args=['cargo-audit','audit','--file',root/'Cargo.lock','--db',root.parent/'.seata-advisory-db','--no-fetch','--json'];p=command('seaorm-cargo-audit',args,1);audit=json.loads(p.stdout)
(out/'seaorm-cargo-audit.json').write_bytes(p.stdout)
save(out/'audit-command.json',dict(argv=list(map(str,args)),exit_code=p.returncode,lock_sha256=sha(root/'Cargo.lock'),database=audit['database'],ignore=audit['settings']['ignore']))
(out/'red-missing-api.log').write_bytes(Path('D:/Temp/seata-statement-result-red.log').read_bytes())
(out/'regression-runner.py').write_bytes(Path('D:/Temp/seata-statement-result-regression.py').read_bytes())
(out/'finalize-evidence.py').write_bytes(Path(__file__).read_bytes())
save(out/'regression-commands.json',records)
for row in json.loads((out/'source-inputs.json').read_bytes()):assert sha(root/row['path'])==row['sha256']
save(out/'catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json']))
print('FINALIZED',len(json.loads((out/'catalog.json').read_bytes())['members']),sha(out/'catalog.json'),flush=True)
