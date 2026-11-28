import os,subprocess,re,json,hashlib,urllib.request
from pathlib import Path
root=Path.cwd();out=Path('D:/codex-builds/seata-rbatis-regression-20261128');out.mkdir(exist_ok=False)
packet=root/'docs/phase-2/evidence/rbatis-mysql/20261128_202109_4a57fd2e'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
paths=[root/row['path'] for row in json.loads((packet/'source-inputs.json').read_bytes())]
paths += [p for p in (root/'tools/rbatis-compatibility').rglob('*') if p.is_file()]
frozen={p.relative_to(root).as_posix():sha(p) for p in sorted(set(paths))}
(out/'source-inputs.json').write_text(json.dumps(frozen,indent=2),encoding='utf-8')
env=os.environ|dict(CARGO_HOME=str(root.parent/'.seata-cargo-cache'),TEMP='D:/Temp',TMP='D:/Temp',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2',PYTHONDONTWRITEBYTECODE='1',PYTHONUTF8='1')
records=[]
def command(label,args,allowed=(0,)):
    print(label,flush=True)
    p=subprocess.run(list(map(str,args)),cwd=root,env=env,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=1200)
    (out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
    summaries=re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',p.stdout)
    row=dict(label=label,argv=list(map(str,args)),exit_code=p.returncode,passed=sum(int(s[0]) for s in summaries),failed=sum(int(s[1]) for s in summaries),ignored=sum(int(s[2]) for s in summaries));records.append(row)
    (out/'commands.json').write_text(json.dumps(records,indent=2),encoding='utf-8');print(json.dumps(row),flush=True)
    assert p.returncode in allowed,(label,(p.stdout+p.stderr)[-3500:].decode(errors='replace'))
for version in ('1.99','1.85'):
    env['CARGO_TARGET_DIR']=str(root.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
    command(version+'-workspace',['cargo','+'+version+'.0','test','--offline','--locked','--workspace','--all-features'])
    command(version+'-clippy',['cargo','+'+version+'.0','clippy','--offline','--locked','-p','seata-at','-p','seata-db-mysql','-p','seata-orm-seaorm','-p','seata-orm-rbatis','--all-targets','--all-features','--','-D','warnings'])
    command(version+'-compatibility',['cargo','+'+version+'.0','test','--offline','--locked','--manifest-path',root/'tools/rbatis-compatibility/Cargo.toml'])
    command(version+'-compatibility-clippy',['cargo','+'+version+'.0','clippy','--offline','--locked','--manifest-path',root/'tools/rbatis-compatibility/Cargo.toml','--all-targets','--','-D','warnings'])
command('core-default-tree',['cargo','+1.99.0','tree','--offline','--locked','-p','seata-db-mysql','--edges','normal'])
command('seaorm-default-tree',['cargo','+1.99.0','tree','--offline','--locked','-p','sea-orm','--edges','normal'])
command('workspace-normal-tree',['cargo','+1.99.0','tree','--offline','--locked','--workspace','--all-features','--edges','normal'])
command('lru-tree',['cargo','+1.99.0','tree','--offline','--locked','-i','lru'])
command('sources',['python','-B',root/'tools/rbatis-compatibility/verify_sources.py'])
db=root.parent/'.seata-advisory-db';command('advisory-pin',['git','-C',db,'rev-parse','HEAD'])
command('cargo-audit',['cargo-audit','audit','--file',root/'Cargo.lock','--db',db,'--no-fetch','--json'],allowed=(1,))
command('compatibility-audit',['cargo-audit','audit','--file',root/'tools/rbatis-compatibility/Cargo.lock','--db',db,'--no-fetch','--json'])
meta=json.load(urllib.request.urlopen(urllib.request.Request('https://crates.io/api/v1/crates/lru/0.18.2',headers={'User-Agent':'seata-rust-rbatis-validation'})))
assert meta['version']['checksum']=='5d2f2f9b4ba7e6b24d95e7e899329d35be83bcded72c8540cdd5368932d1d90a' and not meta['version']['yanked']
(out/'lru-publication.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
assert all(sha(root/name)==digest for name,digest in frozen.items())
(out/'source-end.json').write_text(json.dumps(frozen,indent=2),encoding='utf-8')
print('RBATIS_REGRESSIONS_COMPLETE',flush=True)
