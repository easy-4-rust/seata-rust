"""Verify default exporter omits SQLx and affected default interfaces remain valid."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha


def run():
    out=ROOT/'docs/phase-2/evidence/structured-log-file'/('default_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8])
    out.mkdir(parents=True)
    print('STRUCTURED_LOG_FILE_DEFAULT '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
    env.pop('SEATA_MYSQL_UNDO_EVENTS_MATERIAL',None)
    commands=[]
    packages=['-p','seata-observability']
    for version in ['1.99','1.85']:
        env['CARGO_TARGET_DIR']=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185'))
        for label,args in [
            ('runtime-tree',['tree','-p','seata-observability','--edges','normal','--no-default-features','--offline','--locked']),
            ('structured-tree',['tree','-p','seata-observability','--edges','normal','--no-default-features','--features','structured_logs','--offline','--locked']),
            ('structured-clippy',['clippy',*packages,'--all-targets','--no-default-features','--features','structured_logs','--offline','--locked','--','-D','warnings']),
            ('mysql-only-clippy',['clippy',*packages,'--all-targets','--no-default-features','--features','mysql','--offline','--locked','--','-D','warnings']),
            ('layer-only-clippy',['clippy',*packages,'--all-targets','--no-default-features','--features','structured_log_layer','--offline','--locked','--','-D','warnings']),
            ('layer-runtime-tree',['tree','-p','seata-observability','--edges','normal','--no-default-features','--features','structured_log_layer','--offline','--locked']),
            ('file-only-clippy',['clippy',*packages,'--all-targets','--no-default-features','--features','structured_log_file','--offline','--locked','--','-D','warnings']),
            ('file-runtime-tree',['tree','-p','seata-observability','--edges','normal','--no-default-features','--features','structured_log_file','--offline','--locked']),
            ('default-tests',['test',*packages,'--no-default-features','--offline','--locked']),
            ('default-clippy',['clippy',*packages,'--all-targets','--no-default-features','--offline','--locked','--','-D','warnings']),
        ]:
            argv=['cargo','+'+version+'.0',*args]
            result=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW)
            (out/(version+'-'+label+'.stdout')).write_bytes(result.stdout)
            (out/(version+'-'+label+'.stderr')).write_bytes(result.stderr)
            commands.append(dict(Rust=version,label=label,argv=argv,exit_code=result.returncode))
            save(out/'commands.json',commands)
            need(result.returncode==0,'Default MySQL-off exporter verification failed: '+label+'\n'+result.stderr[-1600:].decode(errors='replace'))
            if label in ['runtime-tree','structured-tree','layer-runtime-tree','file-runtime-tree']:need(b'sqlx' not in result.stdout,'Logging gained SQLx runtime dependencies')
            if label in ['runtime-tree','structured-tree','file-runtime-tree']:need(b'tracing-subscriber' not in result.stdout,'Default/plain logging gained layer dependency')
            if label=='layer-runtime-tree':need(b'tracing-subscriber v0.3.23' in result.stdout,'Pinned layer dependency missing')
    save(out/'result.json',dict(status='DEFAULT_OBSERVABILITY_MYSQL_OFF_AND_AFFECTED_INTERFACES_VERIFIED',Rust_versions=['1.99','1.85'],default_observability_sqlx_dependency=False,WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':run()
