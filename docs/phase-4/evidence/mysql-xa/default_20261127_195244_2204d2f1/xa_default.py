"""Check opt-in XA, default AT interfaces and unchanged third-party dependency inputs."""
import datetime,json,os,subprocess,uuid,shutil,tomllib
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-4/evidence/mysql-xa'/('default_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True);print('XA_DEFAULT '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0');commands=[]
    for version in ['1.99','1.85']:
        env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
        for label,arguments in [
            ('default-tests',['test','-p','seata-db-mysql','--no-default-features','--offline','--locked']),
            ('default-clippy',['clippy','-p','seata-db-mysql','-p','seata-xa','--all-targets','--no-default-features','--offline','--locked','--','-D','warnings']),
            ('xa-only-clippy',['clippy','-p','seata-db-mysql','--all-targets','--no-default-features','--features','xa','--offline','--locked','--','-D','warnings']),
            ('default-normal-tree',['tree','-p','seata-db-mysql','--edges','normal','--no-default-features','--offline','--locked']),
            ('xa-normal-tree',['tree','-p','seata-db-mysql','--edges','normal','--no-default-features','--features','xa','--offline','--locked']),
            ('format',['fmt','--all','--check'])]:
            argv=['cargo','+'+version+'.0',*arguments];value=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW)
            (out/(version+'-'+label+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+label+'.stderr')).write_bytes(value.stderr);commands.append(dict(Rust=version,label=label,argv=argv,exit_code=value.returncode));save(out/'commands.json',commands)
            need(value.returncode==0,label+' failed: '+value.stderr[-1800:].decode(errors='replace'))
            if label=='default-normal-tree':need(b'seata-xa v' not in value.stdout,'Default AT gained XA dependency')
            if label=='xa-normal-tree':need(b'seata-xa v0.1.0' in value.stdout,'Explicit XA identity dependency missing')
    old=tomllib.loads(subprocess.check_output(['git','show','c73dc5871f6d382d80e534634f13403c8338bb57:Cargo.lock'],cwd=ROOT).decode());new=tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'));old={(p['name'],p['version']):p for p in old['package']};new={(p['name'],p['version']):p for p in new['package']}
    need(set(new)-set(old)=={('seata-xa','0.1.0')} and set(old)<=set(new),'Unrelated dependency version changed')
    for key,value in old.items():
        expected=dict(value)
        if key[0]=='seata-db-mysql':expected['dependencies']=sorted(value['dependencies']+['futures-util','seata-xa'])
        need(new[key]==expected,'Existing dependency input changed')
    save(out/'result.json',dict(status='DEFAULT_AT_AND_OPT_IN_XA_VERIFIED',Rust_versions=['1.99','1.85'],third_party_version_changes=0,default_xa_dependency=False,XA_RM_gate_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':run()
