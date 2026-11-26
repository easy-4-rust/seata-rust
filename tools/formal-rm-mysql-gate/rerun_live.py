"""Bounded independent owned-DB matrix; snapshot source remains frozen until all jobs exit."""
from concurrent.futures import ThreadPoolExecutor,as_completed
import hashlib,json,os,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=Path('D:/codex-builds/seata-rm-recovery-final-matrix-20261117');OUT.mkdir(parents=True,exist_ok=True)
def run(rust,role,options):
 env=os.environ|dict(PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1',TEMP='D:/Temp',TMP='D:/Temp')
 args=[sys.executable,'-B','tools/formal-rm-mysql/live.py','--rust',rust]+options
 v=subprocess.run(args,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
 (OUT/(rust+'-'+role+'.log')).write_bytes(v.stdout)
 if v.returncode:raise RuntimeError(rust+' '+role+' failed: '+v.stdout[-1800:].decode(errors='replace'))
 item=json.loads(v.stdout.decode('utf-8').splitlines()[-1]);p=Path(item['run'])
 if hashlib.sha256((p/'catalog.json').read_bytes()).hexdigest()!=item['catalog_sha256']:raise RuntimeError('Actual child catalog differs')
 return rust,role,dict(path=p.relative_to(ROOT).as_posix(),catalog_sha256=item['catalog_sha256'])
roles=[('native',[]),('drop_ok',['--fault','drop_ok']),('block_commit',['--fault','block_commit']),('delay_ok',['--fault','delay_ok']),('denial',['--observe-denial']),('concurrency',['--observe-concurrency']),('fence',['--target','live_undo_fence']),('rollback',['--target','live_undo_rollback'])]
values={'1.99':{},'1.85':{}}
with ThreadPoolExecutor(max_workers=4) as pool:
 futures=[pool.submit(run,rust,role,options) for rust in values for role,options in roles]
 for f in as_completed(futures):
  rust,role,item=f.result();values[rust][role]=item;print('VERIFIED',rust,role,item['path'],item['catalog_sha256'],flush=True)
manifest=json.loads((ROOT/'tools/formal-rm-mysql-gate/qualification-inputs-20261117.json').read_bytes())
manifest['profiles']={rust:{'native':p['native'],'faults':{m:p[m] for m in ('drop_ok','block_commit','delay_ok')},'denial':p['denial'],'concurrency':p['concurrency'],'fence':p['fence'],'rollback':p['rollback']} for rust,p in values.items()}
p=ROOT/'tools/formal-rm-mysql-gate/qualification-inputs-20261117.json';p.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print('MATRIX_COMPLETE',hashlib.sha256(p.read_bytes()).hexdigest(),flush=True)
