"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-2/evidence/mysql-undo-events'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    rows=[]
    variants=[
        ('finish_event','my_sql_undo_event.rs',b'self.emit(outcome, observation);',b'let _ = (outcome, observation);','original_undo_observations_and_read_only_repeats_keep_actual_commit_boundary','missing original Undo result event'),
        ('rollback_commit_marker','data_source_proxy.rs',b'                if let Some(event) = event {\n                    event.mark_commit_attempted();\n                }',b'                if let Some(event) = event {\n                    if false { event.mark_commit_attempted(); }\n                }','actual_undo_commit_ok_held_remains_unknown_or_cancelled_then_original_identity_recovers','actual Undo COMMIT marker missing'),
        ('fence_commit_marker','data_source_proxy.rs',b'                    if let Some(event) = event {\n                        event.mark_commit_attempted();\n                    }',b'                    if let Some(event) = event {\n                        if false { event.mark_commit_attempted(); }\n                    }','actual_undo_commit_ok_held_remains_unknown_or_cancelled_then_original_identity_recovers','actual Undo COMMIT marker missing'),
        ('cancel_event','my_sql_undo_event.rs',b'self.emit("cancelled", "not_observed");',b'let _ = &self;','actual_undo_row_wait_cancel_and_closed_pool_do_not_attempt_commit','Undo cancellation event missing'),
    ]
    originals={name:(ROOT/'crates/seata_db_mysql/src'/name).read_bytes() for _,name,_,_,_,_ in variants}
    try:
        for version in ['1.99','1.85']:
            for name,file,site,replacement,case,marker in variants:
                path=ROOT/'crates/seata_db_mysql/src'/file;original=originals[file]
                site=site.replace(b'\n',b'\r\n') if b'\r\n' in original else site
                replacement=replacement.replace(b'\n',b'\r\n') if b'\r\n' in original else replacement
                need(original.count(site)==1,'Unique actual EOL production site missing')
                path.write_bytes(original.replace(site,replacement))
                value=subprocess.run(['python','-B','tools/formal-source-junit/mysql_undo_events_live.py','--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
                (out/(version+'-'+name+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+name+'.stderr')).write_bytes(value.stderr)
                need(value.returncode==0,'Mutation did not produce the required behavioral failure: '+value.stderr.decode(errors='replace'))
                last=json.loads(value.stdout.splitlines()[-1]);folder=Path(last['evidence']);rows.append(dict(Rust=version,operation=name,case=case,marker=marker,evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=last['catalog_sha256']))
                save(out/'mutations.json',rows);path.write_bytes(original)
                print('Rejected real '+name+' mutation Rust '+version,flush=True)
    finally:
        for name,original in originals.items():(ROOT/'crates/seata_db_mysql/src'/name).write_bytes(original)
    save(out/'restored.json',[dict(path=(ROOT/'crates/seata_db_mysql/src'/name).relative_to(ROOT).as_posix(),sha256=sha(ROOT/'crates/seata_db_mysql/src'/name),exact_original_restored=(ROOT/'crates/seata_db_mysql/src'/name).read_bytes()==original) for name,original in originals.items()])
    (out/Path(__file__).name).write_bytes(Path(__file__).read_bytes())
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run()
