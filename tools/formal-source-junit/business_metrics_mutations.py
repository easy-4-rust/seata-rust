"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-2/evidence/business-metrics'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    path=ROOT/'crates/seata_db_mysql/src/connection_proxy.rs';original=path.read_bytes();rows=[]
    variants=[('execute',b'guard.finish_execute(&result);','completed_dml_is_not_committed_and_delegated_rollback_counts_once','business DML settlement missing'),('rollback',b'guard.finish_rollback(&result);','prepared_and_checked_delegates_each_count_one_explicit_rollback','explicit rollback settlement missing')]
    try:
        for version in ['1.99','1.85']:
            for name,site,case,marker in variants:
                need(original.count(site)==1,'Unique production settlement site missing')
                path.write_bytes(original.replace(site,b'drop(guard);'))
                value=subprocess.run(['python','-B','tools/formal-source-junit/business_metrics_live.py','--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
                (out/(version+'-'+name+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+name+'.stderr')).write_bytes(value.stderr)
                need(value.returncode==0,'Mutation did not produce the required behavioral failure: '+value.stderr.decode(errors='replace'))
                last=json.loads(value.stdout.splitlines()[-1]);folder=Path(last['evidence']);rows.append(dict(Rust=version,operation=name,case=case,marker=marker,evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=last['catalog_sha256']))
                save(out/'mutations.json',rows);path.write_bytes(original)
                print('Rejected real '+name+' mutation Rust '+version,flush=True)
    finally:path.write_bytes(original)
    save(out/'restored.json',dict(path=path.relative_to(ROOT).as_posix(),sha256=sha(path),exact_original_restored=path.read_bytes()==original))
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run()
