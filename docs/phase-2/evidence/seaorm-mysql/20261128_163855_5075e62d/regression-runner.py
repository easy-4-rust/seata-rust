import os,subprocess,re,json
from pathlib import Path
root=Path.cwd();out=Path('D:/codex-builds/seata-statement-result-regression-20261128');out.mkdir(exist_ok=True)
env=os.environ|dict(CARGO_HOME=str(root.parent/'.seata-cargo-cache'),TEMP='D:/Temp',TMP='D:/Temp',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2')
records=[]
for version in ('1.99','1.85'):
 env['CARGO_TARGET_DIR']=str(root.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
 for label,tail in [('workspace',['test','--workspace','--all-features']),('clippy',['clippy','-p','seata-db-mysql','-p','seata-orm-seaorm','--all-targets','--all-features','--','-D','warnings'])]:
  args=['cargo','+'+version+'.0',*tail[:1],'--offline','--locked',*tail[1:]]
  p=subprocess.run(args,cwd=root,env=env,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
  name=version+'-'+label;(out/(name+'.stdout')).write_bytes(p.stdout);(out/(name+'.stderr')).write_bytes(p.stderr)
  summaries=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',p.stdout)
  row=dict(version=version,label=label,argv=args,exit_code=p.returncode,passed=sum(int(s[0]) for s in summaries),ignored=sum(int(s[2]) for s in summaries));records.append(row)
  (out/'result.json').write_text(json.dumps(records,indent=2),encoding='utf-8');print(json.dumps(row),flush=True)
  if p.returncode:print((p.stdout+p.stderr)[-3500:].decode(errors='replace'),flush=True);raise SystemExit(p.returncode)
