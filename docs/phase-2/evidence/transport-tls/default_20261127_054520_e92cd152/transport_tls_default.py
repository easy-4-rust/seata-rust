"""Verify explicitly disabled transport TLS and all affected default interfaces."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha


def run():
    out=ROOT/'docs/phase-2/evidence/transport-tls'/('default_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8])
    out.mkdir(parents=True)
    print('TRANSPORT_TLS_DEFAULT '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
    env.pop('SEATA_TRANSPORT_TLS_MATERIAL',None)
    commands=[]
    packages=['-p','seata-transport','-p','seata-tm','-p','seata-rm','-p','seata-db-mysql']
    for version in ['1.99','1.85']:
        env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
        for label,args in [
            ('runtime-tree',['tree','-p','seata-transport','--edges','normal','--no-default-features','--offline','--locked']),
            ('default-tests',['test',*packages,'--no-default-features','--offline','--locked']),
            ('default-clippy',['clippy',*packages,'--all-targets','--no-default-features','--offline','--locked','--','-D','warnings']),
        ]:
            argv=['cargo','+'+version+'.0',*args]
            result=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW)
            (out/(version+'-'+label+'.stdout')).write_bytes(result.stdout)
            (out/(version+'-'+label+'.stderr')).write_bytes(result.stderr)
            commands.append(dict(Rust=version,label=label,argv=argv,exit_code=result.returncode))
            save(out/'commands.json',commands)
            need(result.returncode==0,'Default TLS-off verification failed: '+label+'\n'+result.stderr[-1600:].decode(errors='replace'))
            if label=='runtime-tree':need(b'rustls' not in result.stdout,'Default seata-transport gained TLS runtime dependencies')
    save(out/'result.json',dict(status='DEFAULT_TRANSPORT_TLS_OFF_AND_AFFECTED_INTERFACES_VERIFIED',Rust_versions=['1.99','1.85'],default_transport_rustls_dependency=False,WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':run()
