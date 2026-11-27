"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-2/evidence/failure-report-metrics'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    rows=[]
    variants=[
        ('rollback_report','mysql_prepared_transaction.rs',b'guard.finish_rollback_report(&result);','prepared_rollback_reports_failure_only_after_local_rollback'),
        ('prepare_report','mysql_prepare_error.rs',b'counter.finish_prepare_report(&result);','failure_report_metrics_cases::explicit_original_prepare_error_preserves_report_outcomes_and_blocks_replay'),
        ('automatic_report','data_source_manager.rs',b'guard.finish_prepare_and_report(&result);','failure_report_metrics_cases::automatic_prepare_failure_observed_has_separate_outer_and_report_results'),
    ]
    originals={name:(ROOT/'crates/seata_db_mysql/src'/name).read_bytes() for _,name,_,_ in variants}
    try:
        for version in ['1.99','1.85']:
            for name,file,site,case in variants:
                marker='left: 0';path=ROOT/'crates/seata_db_mysql/src'/file;original=originals[file]
                need(original.count(site)==1,'Unique production settlement site missing')
                path.write_bytes(original.replace(site,b'drop(counter);' if name=='prepare_report' else b'drop(guard);'))
                value=subprocess.run(['python','-B','tools/formal-source-junit/failure_report_metrics_live.py','--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
                (out/(version+'-'+name+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+name+'.stderr')).write_bytes(value.stderr)
                need(value.returncode==0,'Mutation did not produce the required behavioral failure: '+value.stderr.decode(errors='replace'))
                last=json.loads(value.stdout.splitlines()[-1]);folder=Path(last['evidence']);rows.append(dict(Rust=version,operation=name,case=case,marker=marker,evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=last['catalog_sha256']))
                save(out/'mutations.json',rows);path.write_bytes(original)
                print('Rejected real '+name+' mutation Rust '+version,flush=True)
    finally:
        for name,original in originals.items():(ROOT/'crates/seata_db_mysql/src'/name).write_bytes(original)
    save(out/'restored.json',[dict(path=(ROOT/'crates/seata_db_mysql/src'/name).relative_to(ROOT).as_posix(),sha256=sha(ROOT/'crates/seata_db_mysql/src'/name),exact_original_restored=(ROOT/'crates/seata_db_mysql/src'/name).read_bytes()==original) for name,original in originals.items()])
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run()
