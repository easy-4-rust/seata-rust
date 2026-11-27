"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-4/evidence/mysql-xa'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    rows=[]
    variants=[
        ('java_prefix','crates/seata_xa/src/xa_branch_xid.rs',b'format!("-{branch_id}")',b'format!("{branch_id}")','original_java_vectors_match_every_component','fixed Java XA byte identity changed','xa_rust.py'),
        ('omit_prepare','crates/seata_db_mysql/src/mysql_xa_transaction.rs',b'        MySqlXaDataSource::command(\n            &mut connection,\n            &format!("XA PREPARE {args}"),\n            deadline,\n            XaOperation::Prepare,\n        )\n        .await?;',b'        // omitted native prepare for counterexample','prepared_is_invisible_locked_and_resolved_only_by_original_identity','original Prepared branch missing','xa_mysql.py'),
        ('wrong_commit_direction','crates/seata_db_mysql/src/mysql_xa_data_source.rs',b'            "COMMIT"',b'            "ROLLBACK"','prepared_is_invisible_locked_and_resolved_only_by_original_identity','original XA COMMIT must publish original business row','xa_mysql.py'),
        ('ignore_format','crates/seata_db_mysql/src/mysql_xa_data_source.rs',b'if format != XaBranchXid::FORMAT_ID {',b'if false && format != XaBranchXid::FORMAT_ID {','original_hex_identity_and_active_rollback_preserve_values','foreign XA format was mistaken for Seata Prepared','xa_mysql.py'),
        ('ignore_qualifier','crates/seata_db_mysql/src/mysql_xa_data_source.rs',b'\n                    && data[global as usize..] == *xid.get_branch_qualifier()',b'','prepared_is_invisible_locked_and_resolved_only_by_original_identity','other branch qualifier was mistaken for Prepared','xa_mysql.py'),
        ('unknown_as_not_sent','crates/seata_db_mysql/src/mysql_xa_data_source.rs',b'.map_err(|_| MySqlXaError::Unknown {',b'.map_err(|_| MySqlXaError::NotSent {','original_xa_ok_loss_preserves_unknown_without_automatic_replay','original XA OK loss must preserve unknown','xa_mysql.py'),
        ('omit_constraints','crates/seata_db_mysql/src/mysql_xa_table.rs',b'if trigger || foreign_key {',b'if false && (trigger || foreign_key) {','unsafe_tables_are_rejected_before_dml','XA trigger was not rejected before DML','xa_mysql.py'),
    ]
    originals={name:(ROOT/name).read_bytes() for _,name,_,_,_,_,_ in variants}
    try:
        for version in ['1.99','1.85']:
            for name,file,site,replacement,case,marker,runner in variants:
                path=ROOT/file;original=originals[file]
                site=site.replace(b'\n',b'\r\n') if b'\r\n' in original else site
                replacement=replacement.replace(b'\n',b'\r\n') if b'\r\n' in original else replacement
                need(original.count(site)==1,'Unique actual EOL production site missing')
                path.write_bytes(original.replace(site,replacement))
                value=subprocess.run(['python','-B','tools/formal-source-junit/'+runner,'--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
                (out/(version+'-'+name+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+name+'.stderr')).write_bytes(value.stderr)
                need(value.returncode==0,'Mutation did not produce the required behavioral failure: '+value.stderr.decode(errors='replace'))
                last=json.loads(value.stdout.splitlines()[-1]);folder=Path(last['evidence']);rows.append(dict(Rust=version,operation=name,case=case,marker=marker,evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=last['catalog_sha256']))
                save(out/'mutations.json',rows);path.write_bytes(original)
                print('Rejected real '+name+' mutation Rust '+version,flush=True)
    finally:
        for name,original in originals.items():(ROOT/name).write_bytes(original)
    save(out/'restored.json',[dict(path=(ROOT/name).relative_to(ROOT).as_posix(),sha256=sha(ROOT/name),exact_original_restored=(ROOT/name).read_bytes()==original) for name,original in originals.items()])
    (out/Path(__file__).name).write_bytes(Path(__file__).read_bytes())
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run()
