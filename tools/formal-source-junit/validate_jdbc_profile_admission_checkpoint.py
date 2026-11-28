"""Verify full JDBC profile admission, physical row locks and both guarded preparation paths."""
from pathlib import Path
import shutil
from inventory_rebuilt import ROOT, need, save, sha
from validate_structured_logs import catalog
import validate_jdbc_profile_admission as native

BASE=ROOT/'docs/phase-2/evidence/mysql-jdbc-profile-admission'
PACKET=BASE/'java_20261128_120508_5f38fa21'
DIGEST='4c866f209dd2a222763b0f1165b1e2cb3f98810ed5a097c5724c5a0e94d379e8'


def main():
    native.verify(PACKET,DIGEST)
    rejected=native.weakenings(PACKET)+native.admission_weakenings(PACKET)
    need(rejected==218,'Admission weakening coverage differs')
    result=native.load(PACKET/'result.json')
    need(result['Rust_versions']==['1.99','1.85'] and result['profiles']==['boolean','numeric'] and result['directions']==['commit','rollback'] and result['actual_global_transactions']==8,'Native matrix incomplete')
    authorities=[]
    for name in ['regression_1.99_20261128_121041_d27751db','regression_1.85_20261128_121249_d8fe6cec']:
        folder=BASE/name;digest=sha(folder/'catalog.json');catalog(folder,digest)
        regression=native.load(folder/'result.json')
        need((regression['passed'],regression['ignored'],regression['successful_test_harness_summaries'])==(824,409,192) and regression['all_features'] and regression['strict_mysql_all_targets_clippy'],'Workspace or Clippy coverage differs')
        commands=native.load(folder/'commands.json');need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Workspace command failed')
        for source in regression['production_sources']:need(sha(ROOT/source['path'])==source['sha256'],'Regressed source changed')
        authorities.append(dict(path=name,catalog_sha256=digest))
    out=BASE/'validation_20261128_final';out.mkdir(exist_ok=True)
    save(out/'result.json',dict(status='FULL_JDBC_PROFILE_BINDING_COMPONENT_VERIFIED',native_catalog_sha256=DIGEST,actual_global_transactions=8,accepted_business_dml=48,rejected_profile_preparations=40,actual_local_profile_checks=dict(accepted_per_Rust=6,rejected_per_Rust=30),guard_owners={'1.99':'transaction pool','1.85':'manager pool'},physical_waiting_update_observed=True,legacy_005_and_011_paths_preserved=True,nullable_migration_and_strict_null_rejection=True,unknown_code_mysql_error=3819,Rust_versions=['1.99.0','1.85.0'],passed_per_workspace=824,ignored_per_workspace=409,rejected_weakening_controls=rejected,authorities=authorities,legacy_defaults_changed=False,all_writers_forced_to_enroll=False,full_sql_or_production_gate=False,whole_plan_complete=False))
    sources=['crates/seata_db_mysql/src/mysql_resource_binding.rs','crates/seata_db_mysql/src/mysql_resource_binding_jdbc_tests.rs','crates/seata_db_mysql/src/connection_proxy.rs','crates/seata_db_mysql/src/data_source_proxy.rs','crates/seata_db_mysql/src/mysql_at_registration_checks.rs','crates/seata_db_mysql/src/mysql_options.rs','crates/seata_db_mysql/src/mysql_options_java_tc_profile_tests.rs','crates/seata_db_mysql/migrations/021_seata_rs_at_jdbc_profile_binding.sql','crates/seata_db_mysql/tests/live_jdbc_profile_admission_java.rs','tools/formal-source-junit/jdbc_profile_admission_java.py','tools/formal-source-junit/jdbc_profile_java_oracle.py','tools/formal-source-junit/JdbcProfileAdmissionOracle.java','tools/formal-source-junit/validate_jdbc_profile_admission.py','tools/formal-source-junit/jdbc_profile_admission_regression.py','tools/formal-source-junit/'+Path(__file__).name]
    save(out/'source-bindings.json',[dict(path=p,sha256=sha(ROOT/p)) for p in sources])
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json'])
    print('FULL_JDBC_PROFILE_BINDING_COMPONENT_VERIFIED '+sha(out/'catalog.json'))


if __name__=='__main__':main()
