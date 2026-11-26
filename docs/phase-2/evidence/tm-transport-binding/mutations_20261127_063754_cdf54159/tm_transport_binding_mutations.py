"""Reject three trust-boundary weakenings in real TLS/MySQL, then restore exact source bytes."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid
from inventory_rebuilt import ROOT, need, save, sha


def run():
    base=ROOT/'docs/phase-2/evidence/tm-transport-binding'
    out=base/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir()
    print('TM_TRUST_MUTATIONS '+str(out),flush=True)
    variants=[
        ('query_guard','crates/seata_db_mysql/src/my_sql_launcher_snapshot.rs',
         b'if self.peer_trust_binding.as_ref() != manager.peer_trust_binding() {',
         b'if false && self.peer_trust_binding.as_ref() != manager.peer_trust_binding() {',
         'persisted_tls_origin_rejects_plaintext_replacement_before_status_request',
         'TLS source cannot trust a plaintext replacement'),
        ('locked_CAS','crates/seata_db_mysql/src/my_sql_launcher_store.rs',
         b'{} AND transport_binding=?',b'{} AND ? IS NOT NULL',
         'same_trust_stopped_owner_transfers_after_binding_race_is_rejected',
         'locked claim must compare persisted transport binding'),
        ('missing_column_fallback','crates/seata_db_mysql/src/my_sql_launcher_store.rs',
         b'Err(sqlx::Error::ColumnNotFound(_)) if version == 1 => None,',
         b'Err(sqlx::Error::ColumnNotFound(_)) if matches!(version, 1 | 2) => None,',
         'damaged_protected_binding_never_falls_back_and_original_bytes_allow_completion',
         'protected version with missing column cannot become plaintext'),
    ]
    records=[]
    for version in ['1.99','1.85']:
        for label,path,needle,replacement,case,marker in variants:
            source=ROOT/path;original=source.read_bytes();need(original.count(needle)==1,'Mutation target ambiguous: '+label)
            before=set(base.glob('red_*'))
            try:
                source.write_bytes(original.replace(needle,replacement));mutated_sha=sha(source)
                argv=[sys.executable,'-B',ROOT/'tools/formal-source-junit/tm_transport_binding_live.py','--rust',version,'--case',case,'--expect-failure',marker]
                result=subprocess.run(list(map(str,argv)),cwd=ROOT,env=os.environ|dict(PYTHONDONTWRITEBYTECODE='1',TEMP='D:/Temp',TMP='D:/Temp'),capture_output=True,timeout=360,creationflags=subprocess.CREATE_NO_WINDOW)
                (out/(version+'-'+label+'.stdout')).write_bytes(result.stdout);(out/(version+'-'+label+'.stderr')).write_bytes(result.stderr)
                need(result.returncode==0,'Real behavioral mutation did not qualify: '+label+'\n'+(result.stdout+result.stderr)[-1600:].decode(errors='replace'))
                created=set(base.glob('red_*'))-before;need(len(created)==1,'Mutation evidence ambiguous');folder=created.pop()
                need((folder/'catalog.json').is_file(),'Expected failure evidence incomplete')
                records.append(dict(Rust=version,mutation=label,path=path,mutation_sha256=mutated_sha,selected_case=case,
                    evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=sha(folder/'catalog.json')))
                save(out/'mutations.json',records)
                print('REJECTED '+version+' '+label,flush=True)
            finally:
                source.write_bytes(original)
    save(out/'result.json',dict(status='REAL_TLS_AND_MYSQL_TRUST_MUTATIONS_REJECTED',mutations_per_version=3,Rust_versions=['1.99','1.85'],exact_sources_restored=True,WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':run()
