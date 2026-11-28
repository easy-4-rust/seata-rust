from pathlib import Path
import subprocess,os,json
r=Path.cwd();out=r/'docs/phase-2/evidence/seaorm-borrowed/20261128_173042_eb40812f'
env=os.environ|dict(CARGO_HOME=str(r.parent/'.seata-cargo-cache'),CARGO_TARGET_DIR=str(r.parent/'.seata-target-nodebug'),TEMP='D:/Temp',TMP='D:/Temp')
commands=[]
for name,args,expected in [
 ('core-default-tree',['cargo','+1.99.0','tree','--offline','--locked','-p','seata-db-mysql','-e','normal'],0),
 ('seaorm-default-tree',['cargo','+1.99.0','tree','--offline','--locked','-p','sea-orm','-e','normal'],0),
 ('workspace-normal-tree',['cargo','+1.99.0','tree','--offline','--locked','--workspace','--all-features','-e','normal'],0),
 ('cargo-audit',['cargo-audit','audit','--file',str(r/'Cargo.lock'),'--db',str(r.parent/'.seata-advisory-db'),'--no-fetch','--json'],1)]:
 p=subprocess.run(args,cwd=r,env=env,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
 (out/(name+'.stdout')).write_bytes(p.stdout);(out/(name+'.stderr')).write_bytes(p.stderr)
 commands.append(dict(label=name,argv=args,exit_code=p.returncode));assert p.returncode==expected,(name,p.returncode)
(out/'boundary-commands.json').write_text(json.dumps(commands,indent=2),encoding='utf-8')
print('BOUNDARY_AUDIT_CAPTURED',[(c['label'],c['exit_code']) for c in commands])
