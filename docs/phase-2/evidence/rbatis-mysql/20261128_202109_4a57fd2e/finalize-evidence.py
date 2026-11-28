from pathlib import Path
import json,hashlib,shutil
root=Path.cwd();out=root/'docs/phase-2/evidence/rbatis-mysql/20261128_202109_4a57fd2e';reg=Path('D:/codex-builds/seata-rbatis-regression-20261128')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
rows=json.loads((reg/'commands.json').read_bytes());assert len(rows)==16,len(rows)
assert all(row['exit_code']==(1 if row['label']=='cargo-audit' else 0) for row in rows)
for version in ['1.99','1.85']:
    row=next(r for r in rows if r['label']==version+'-workspace');assert [row['passed'],row['failed'],row['ignored']]==[861,0,420]
inputs=json.loads((reg/'source-inputs.json').read_bytes());assert inputs==json.loads((reg/'source-end.json').read_bytes())
for name,digest in inputs.items():
    assert sha(root/name)==digest,name
    target=out/'source'/name;target.parent.mkdir(parents=True,exist_ok=True)
    if target.exists():assert sha(target)==digest,name
    else:target.write_bytes((root/name).read_bytes())
assert not (out/'native-catalog.json').exists();(out/'native-catalog.json').write_bytes((out/'catalog.json').read_bytes())
shutil.copytree(reg,out/'regression')
(out/'verification-tools').mkdir(exist_ok=True)
p=root/'tools/formal-source-junit/validate_rbatis_regression.py';(out/'verification-tools'/p.name).write_bytes(p.read_bytes())
(out/'regression-runner.py').write_bytes(Path('D:/Temp/seata-rbatis-regression.py').read_bytes())
(out/'finalize-evidence.py').write_bytes(Path(__file__).read_bytes())
for label,folder in [('timestamp','20261128_201747_04328ae1'),('time-sign','20261128_201922_72147417')]:
    before=out.parent/folder;target=out/'red-executions'/label;target.mkdir(parents=True)
    for rel in ['source-inputs.json','rust-binaries.json','result.json','1.99-commit/client-stdout.log','1.99-commit/client-stderr.log','1.99-commit/command.json']:
        dst=target/rel;dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes((before/rel).read_bytes())
    overrides=[]
    for row in json.loads((before/'source-inputs.json').read_bytes()):
        name=row['path'];source=before/'source'/name;assert sha(source)==row['sha256']
        if not (out/'source'/name).exists() or sha(out/'source'/name)!=row['sha256']:
            dst=target/'source-overrides'/name;dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(source.read_bytes());overrides.append(name)
    save(target/'source-overrides.json',overrides)
# The existing primary snapshot must still match every input used by the real TC run.
for row in json.loads((out/'source-inputs.json').read_bytes()):assert sha(root/row['path'])==row['sha256']
save(out/'catalog.json',{'members':[{'path':p.relative_to(out).as_posix(),'sha256':sha(p)} for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json']})
print('FINALIZED',len(json.loads((out/'catalog.json').read_bytes())['members']),sha(out/'catalog.json'))
