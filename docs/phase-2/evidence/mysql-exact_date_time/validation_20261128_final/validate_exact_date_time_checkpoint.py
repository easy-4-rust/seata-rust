"""Verify the SQL/Undo increment against original TC evidence and current source bytes."""
import hashlib
import json
from pathlib import Path
import shutil
import zipfile
from inventory_rebuilt import ROOT, need, save, sha
from validate_structured_logs import catalog
import exact_date_time_java_oracle
import validate_exact_date_time_column as native

BASE = ROOT / 'docs/phase-2/evidence/mysql-exact_date_time'
PACKET = BASE / 'java_20261128_112133_38926f66'
DIGEST = 'a30c2955b07293b2310bac4c4bb79e8a1bbcd8204a3ebf7a484a9451ad447bb3'


def source_inputs():
    rows = native.load(PACKET / 'source-inputs.json')
    differences = []
    with zipfile.ZipFile(PACKET / 'source-inputs.zip') as archive:
        need(set(archive.namelist()) == {r['path'] for r in rows}, 'Native source ZIP membership differs')
        for row in rows:
            original = archive.read(row['path'])
            need(hashlib.sha256(original).hexdigest() == row['sha256'], 'Native frozen source changed')
            current = (ROOT / row['path']).read_bytes()
            if current == original:
                continue
            # This helper is not compiled by the live_exact_date_time_java target. The full
            # workspace subsequently required precisely these exhaustive enum match arms.
            need(row['path'] == 'crates/seata_at/tests/fastjson_undo_native.rs', 'Native production or harness source changed')
            removed = current.decode('utf-8').replace('                ColumnType::LocalDate => ColumnDomain::LocalDate,\n                ColumnType::InternalTime => ColumnDomain::InternalTime {\n                    fractional_seconds: 6,\n                },\n', '', 1).replace('                        FieldValue::LocalDate(value) => value.to_sql_string(),\n                        FieldValue::InternalTime(value) => value.to_sql_string(6).unwrap(),\n', '', 1)
            need(removed.encode('utf-8') == original, 'Unrelated fixture changed beyond the six added match arms')
            differences.append(dict(path=row['path'], native_sha256=row['sha256'], final_sha256=sha(ROOT / row['path']), reason='Six exhaustive match arms in an unrelated legacy test helper; final workspace regressed'))
    need(len(differences) == 1, 'Expected legacy fixture reconciliation differs')
    return differences


def main():
    catalog(PACKET, DIGEST)
    differences = source_inputs()
    exact_date_time_java_oracle.verify(PACKET)
    result = native.load(PACKET / 'result.json')
    need(result['Rust_versions'] == ['1.99', '1.85'] and result['profiles'] == ['boolean', 'numeric'] and result['directions'] == ['commit', 'rollback'] and result['actual_global_transactions'] == 8 and result['whole_plan_complete'] is False, 'Native scope changed')
    need(result['Java_pin'] == native.guard.reg.PIN and result['Java_jar_sha256'] == native.guard.reg.JAR_SHA, 'Native original Java identity differs')
    need(all(row['exit_code'] == (1 if row['label'] == 'confirm-removed' else 0) for row in native.load(PACKET / 'commands.json')), 'Native command failed')
    need(native.load(PACKET / 'cleanup.json')['owned_container_removed'], 'Native server cleanup absent')
    initial, stopped = native.load(PACKET / 'inspect.stdout')[0], native.load(PACKET / 'inspect-stopped.stdout')[0]
    need(initial['Id'] == stopped['Id'] == (PACKET / 'create.stdout').read_text().strip() and initial['Image'] == stopped['Image'] == native.guard.IMAGE and initial['Mounts'] == stopped['Mounts'] and not stopped['State']['Running'] and stopped['State']['ExitCode'] == 0, 'Owned server identity/stop differs')
    binding = native.load(PACKET / 'auth-binding.json')
    need(set(binding['runtime']) == {'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler', 'org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class', 'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'}, 'Auth overlay replaced coordinator')
    for path, digest in binding['sources'].items(): need(sha(Path(path)) == digest, 'Auth source changed')
    for version in result['Rust_versions']:
        for profile in result['profiles']:
            for direction in result['directions']:
                native.case(PACKET, version, profile, direction)
    rejected = native.weakenings(PACKET)
    need(rejected == 160, 'Native weakening coverage differs')
    authorities = []
    for name in ['regression_1.99_20261128_112535_a8c2d04e', 'regression_1.85_20261128_112553_d9cc40ff']:
        folder = BASE / name
        digest = sha(folder / 'catalog.json')
        catalog(folder, digest)
        regression = native.load(folder / 'result.json')
        need((regression['passed'], regression['ignored'], regression['successful_test_harness_summaries']) == (822, 404, 190) and regression['all_features'] and regression['strict_mysql_all_targets_clippy'], 'Workspace verification differs')
        need(all(c['exit_code'] == 0 for c in native.load(folder / 'commands.json')), 'Workspace command failed')
        for source in regression['production_sources']: need(sha(ROOT / source['path']) == source['sha256'], 'Regressed final source changed')
        authorities.append(dict(path=name, catalog_sha256=digest))
    out = BASE / 'validation_20261128_final'
    out.mkdir(exist_ok=True)
    save(out / 'result.json', dict(status='DATE_TIME_SQL_UNDO_INCREMENT_VERIFIED', native_catalog_sha256=DIGEST, actual_global_transactions=8, business_dml=48, rejected_weakening_controls=rejected, Rust_versions=['1.99.0', '1.85.0'], passed_per_workspace=822, ignored_per_workspace=404, authorities=authorities, native_to_final_test_helper_difference=differences, time_primary_key_live_verified=False, dedicated_process_restart_verified=False, full_date_time_support=False, whole_plan_complete=False))
    tools = ['exact_date_time_column_java.py', 'exact_date_time_java_oracle.py', 'ExactDateTimeColumnOracle.java', 'validate_exact_date_time_column.py', 'exact_date_time_regression.py', Path(__file__).name]
    save(out / 'tool-bindings.json', [dict(path='tools/formal-source-junit/' + name, sha256=sha(ROOT / 'tools/formal-source-junit' / name)) for name in tools])
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json'])
    print('DATE_TIME_SQL_UNDO_INCREMENT_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    main()
