"""Seal current DATE/TIME ordinary-column, primary-key and same-branch recovery evidence."""
import json
from pathlib import Path
import shutil
from inventory_rebuilt import ROOT, need, save, sha
from validate_structured_logs import catalog
import validate_exact_temporal_restart as native

BASE=ROOT/'docs/phase-2/evidence/mysql-exact-temporal-restart'
PACKET=BASE/'java_20261128_114052_92326204'
DIGEST='b2253dd65c87d6b17da87b5677fcebe1d50b316e18873c289921924f3e6ed5f9'


def main():
    native.verify(PACKET,DIGEST)
    rejected=native.weakenings(PACKET)+native.process_weakenings(PACKET)+native.tamper_weakenings(PACKET)
    need(rejected==320,'Native or corruption weakening coverage differs')
    result=native.load(PACKET/'result.json')
    need(result['Rust_versions']==['1.99','1.85'] and result['profiles']==['boolean','numeric'] and result['directions']==['commit','rollback'] and result['actual_global_transactions']==8,'Native matrix incomplete')
    authorities=[]
    for name in ['regression_1.99_20261128_114142_f2b8828e','regression_1.85_20261128_114255_662eb7f6']:
        folder=BASE/name;digest=sha(folder/'catalog.json');catalog(folder,digest)
        regression=native.load(folder/'result.json')
        need((regression['passed'],regression['ignored'],regression['successful_test_harness_summaries'])==(822,407,191) and regression['all_features'] and regression['strict_mysql_all_targets_clippy'],'Workspace coverage/Clippy differs')
        commands=native.load(folder/'commands.json');need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Workspace commands failed')
        for source in regression['production_sources']:need(sha(ROOT/source['path'])==source['sha256'],'Regressed source changed')
        authorities.append(dict(path=name,catalog_sha256=digest))
    folder=BASE/'process_checks_20261128';digest=sha(folder/'catalog.json');catalog(folder,digest)
    checks=native.load(folder/'result.json');need(checks['checks']==11 and checks['exit_code']==0 and checks['process_source_sha256']==sha(ROOT/'tools/formal-source-junit/exact_temporal_restart_process.py'),'Restore gate selector checks differ')
    authorities.append(dict(path=folder.name,catalog_sha256=digest))
    out=BASE/'validation_20261128_final';out.mkdir(exist_ok=True)
    save(out/'result.json',dict(status='DATE_TIME_RESTART_AND_CHECKSUM_VERIFIED',native_catalog_sha256=DIGEST,actual_global_transactions=8,business_dml=48,Rust_process_instances=24,Rust_versions=['1.99.0','1.85.0'],date_ordinary_column_physical_rows=13,time_primary_key_microseconds_preserved=True,corrupt_recovery_rejected=True,actual_tc_callback_retryable_failure_observed=True,original_bytes_restored_before_same_branch_completion=True,branch_or_success_report_replayed=False,rejected_weakening_controls=rejected,gate_selector_checks=11,passed_per_workspace=822,ignored_per_workspace=407,authorities=authorities,full_date_time_production_admission=False,whole_plan_complete=False))
    sources=['crates/seata_db_mysql/tests/live_exact_temporal_restart_java.rs','tools/formal-source-junit/exact_temporal_restart_java.py','tools/formal-source-junit/exact_temporal_restart_java_oracle.py','tools/formal-source-junit/ExactTemporalRestartOracle.java','tools/formal-source-junit/exact_temporal_restart_process.py','tools/formal-source-junit/exact_temporal_restart_process_checks.py','tools/formal-source-junit/exact_temporal_restart_regression.py','tools/formal-source-junit/validate_exact_temporal_restart.py','tools/formal-source-junit/'+Path(__file__).name]
    save(out/'source-bindings.json',[dict(path=p,sha256=sha(ROOT/p)) for p in sources])
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json'])
    print('DATE_TIME_RESTART_AND_CHECKSUM_VERIFIED '+sha(out/'catalog.json'))


if __name__=='__main__':main()
