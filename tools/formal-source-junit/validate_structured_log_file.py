"""Independently validate final source, native file gates and actual MySQL sink output."""
import argparse,copy,datetime,json,shutil,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha,safe_path
from validate_structured_logs import catalog,sources,mysql,lines,record

OPERATIONS={'omit_native_sync','omit_final_sync','omit_byte_budget','omit_threshold','lose_partial_tail'}

def native(folder):
    sources(folder)
    commands=json.loads((folder/'commands.json').read_bytes())
    need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Native file targets not executed')
    for label,count in [('tests',10),('unit',4)]:
        need(('test result: ok. '+str(count)+' passed; 0 failed;').encode() in (folder/(label+'.stdout')).read_bytes(),'Native file target summary differs')
    expected=['readonly_empty_target_exercises_native_sync_all','shutdown_drains_original_worker_once_and_caches_result','record_threshold_synchronizes_live_prefix_and_final_drain','timer_synchronizes_dirty_live_prefix_without_idle_churn','file_budget_rejects_next_record_and_synchronizes_existing_prefix','first_record_over_budget_writes_nothing','nonempty_or_displaced_files_are_rejected_without_modification','readonly_write_failure_is_typed_and_redacted','configuration_and_missing_runtime_are_explicit_errors','invalid_shutdown_budget_does_not_request_stop']
    output=(folder/'tests.stdout').read_bytes()
    need(all(('test '+name+' ... ok').encode() in output for name in expected),'Native case absent or filtered')
    output=(folder/'unit.stdout').read_bytes()
    need(all((name+' ... ok').encode() in output for name in ['shutdown_closes_admission_but_keeps_original_inflight_future','production_worker_keeps_exact_original_record_bytes','cancelled_worker_reports_unknown_and_caches_it','pending_and_cancelled_wait_keep_same_controlled_worker']),'Controlled worker target missing')

def production_proof(records,proof,byte_count):
    need(type(proof['production_file_sink']) is bool and proof['production_file_sink'] is True,'Test helper substituted for production sink')
    need(proof['file_write_all_sync_all_and_readback_confirmed'] is True,'File confirmation missing')
    need(proof['source']['layer_mode'] is True,'MySQL production sink did not use standard Layer')
    report=proof['report']
    need(set(report)=={'received_records','written_records','synchronized_records','written_bytes','sync_operations'} and all(type(v) is int for v in report.values()),'File report type/field mismatch')
    need((report['received_records'],report['written_records'],report['synchronized_records'],report['sync_operations'])==(3,3,3,2),'Production sink confirmation differs')
    need(len(records)==3 and report['written_bytes']==byte_count,'Production file byte count differs')
    for event in records:record(event)

def mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes())
    need(len(rows)==10 and {(r['Rust'],r['operation']) for r in rows}=={(v,k) for v in ['1.99','1.85'] for k in OPERATIONS},'Source counterexamples incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256'])
        need(json.loads((child/'commands.json').read_bytes())[0]['exit_code']==101,'Mutation did not fail real target')
        need(row['marker'].encode() in (child/'tests.stdout').read_bytes()+(child/'tests.stderr').read_bytes(),'Mutation failed for unrelated reason')
    for row in json.loads((folder/'restored.json').read_bytes()):
        need(row['exact_original_restored'] is True and sha(safe_path(ROOT,row['path']))==row['sha256'],'Production source not restored exactly')

def regression(folder,version):
    commands=json.loads((folder/'commands.json').read_bytes());need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Workspace/Clippy gate failed')
    result=json.loads((folder/'result.json').read_bytes());need(result['Rust']==version and result['passed']==780 and result['ignored']==348 and result['all_features'] is True and result['strict_observability_all_targets_clippy'] is True,'Unexpected full regression totals')
    for row in result['production_sources']:need(sha(safe_path(ROOT,row['path']))==row['sha256'],'Regression source drift')

def default(folder):
    commands=json.loads((folder/'commands.json').read_bytes())
    need(len(commands)==20 and all(c['exit_code']==0 for c in commands),'Feature matrix incomplete/failed')
    for version in ['1.99','1.85']:
        for name in ['runtime-tree','structured-tree','file-runtime-tree']:
            output=(folder/(version+'-'+name+'.stdout')).read_bytes();need(b'sqlx' not in output and b'tracing-subscriber' not in output,'File/default logging acquired optional integration dependency')
        need(b'sqlx' not in (folder/(version+'-layer-runtime-tree.stdout')).read_bytes(),'Layer acquired MySQL dependency')
    need(subprocess.check_output(['git','show','9580856207682d5a8ce2de5da157ffd24e03ad12:Cargo.lock'],cwd=ROOT)==(ROOT/'Cargo.lock').read_bytes(),'Unrelated lockfile changed')

def run(args):
    folders={key:safe_path(ROOT,getattr(args,key)) for key in ['rust199','rust185','mysql','mutations','regression199','regression185','default']}
    for key,folder in folders.items():catalog(folder,getattr(args,key+'_sha'))
    native(folders['rust199']);native(folders['rust185']);mysql(folders['mysql']);mutations(folders['mutations']);regression(folders['regression199'],'1.99');regression(folders['regression185'],'1.85');default(folders['default'])
    rejected={}
    for version in ['1.99','1.85']:
        obs=folders['mysql']/'observations'/version/'case-0';path=obs/'mysql-native.ndjson';records=lines(path);proof=json.loads((obs/'mysql-native-proof.json').read_bytes());production_proof(records,proof,len(path.read_bytes()));rejected[version]=[]
        def reject(name,changed_records,changed_proof,byte_count):
            try:production_proof(changed_records,changed_proof,byte_count)
            except (ValueError,AssertionError,KeyError,TypeError):rejected[version].append(name)
            else:raise ValueError('Weakened file confirmation accepted: '+name)
        for key,value in [('received_records',2),('written_records',2),('synchronized_records',0),('sync_operations',0),('written_bytes',True)]:
            changed=copy.deepcopy(proof);changed['report'][key]=value;reject('false-'+key,records,changed,len(path.read_bytes()))
        changed=copy.deepcopy(proof);changed['production_file_sink']=False;reject('helper-substitution',records,changed,len(path.read_bytes()))
        reject('missing-record',records[:-1],proof,len(path.read_bytes()));reject('duplicate-record',records+[records[0]],proof,len(path.read_bytes()));reject('wrong-byte-count',records,proof,len(path.read_bytes())+1)
        changed=copy.deepcopy(records);changed[0]['fields']['password']='private-password';reject('private-field',changed,proof,len(path.read_bytes()))
    out=ROOT/'docs/phase-2/evidence/structured-log-file'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='PRODUCTION_SINGLE_FILE_SINK_AND_ORIGINAL_MYSQL_FACTS_VALIDATED',authority={k:dict(path=getattr(args,k),catalog_sha256=getattr(args,k+'_sha')) for k in folders},rejected_controls=rejected,source_mutations=10,final_source_inputs_match_current_bytes=True,Windows_native_only=True,external_review='OPEN',rotation_and_retention='OPEN',power_loss_test=False,original_java_tc_rerun=False,WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['rust199','rust185','mysql','mutations','regression199','regression185','default']:parser.add_argument('--'+key,required=True);parser.add_argument('--'+key+'-sha',required=True)
    run(parser.parse_args())
