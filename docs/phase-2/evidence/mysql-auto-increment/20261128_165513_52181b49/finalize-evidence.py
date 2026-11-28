from pathlib import Path
import json,hashlib
r=Path.cwd();out=r/'docs/phase-2/evidence/mysql-auto-increment/20261128_165513_52181b49';reg=Path('D:/codex-builds/seata-auto-increment-regression-20261128')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,data):p.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf-8')
records=json.loads((reg/'result.json').read_bytes());assert len(records)==4 and all(row['exit_code']==0 for row in records)
components=[]
for version in ('1.99','1.85'):
 runs={row['label']:row for row in records if row['version']==version};assert runs['workspace']['passed']==845 and runs['workspace']['ignored']==417
 logs=[]
 for label,suffix in [('workspace','workspace'),('clippy','clippy')]:
  p=out/('component-'+version+'-'+suffix+'.log');p.write_bytes((reg/(version+'-'+label+'.stdout')).read_bytes()+(reg/(version+'-'+label+'.stderr')).read_bytes());logs.append(dict(path=p.name,sha256=sha(p)))
 components.append(dict(version=version,exit_code=0,passed=845,failed=0,ignored=417,test_argv=runs['workspace']['argv'],clippy_argv=runs['clippy']['argv'],logs=logs))
save(out/'component-results.json',components);save(out/'regression-commands.json',records)
(out/'verification-tools').mkdir(exist_ok=True);p=r/'tools/formal-source-junit/validate_auto_increment_regression.py';(out/'verification-tools'/p.name).write_bytes(p.read_bytes())
(out/'regression-runner.py').write_bytes(Path('D:/Temp/seata-auto-increment-regression.py').read_bytes());(out/'finalize-evidence.py').write_bytes(Path(__file__).read_bytes())
for row in json.loads((out/'source-inputs.json').read_bytes()):assert sha(r/row['path'])==row['sha256']
save(out/'catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json']))
print('FINALIZED',len(json.loads((out/'catalog.json').read_bytes())['members']),sha(out/'catalog.json'))
