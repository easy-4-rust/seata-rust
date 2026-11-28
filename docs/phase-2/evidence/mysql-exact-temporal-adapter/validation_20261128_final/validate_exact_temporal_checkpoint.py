"""Seal the actual DATE/TIME component checkpoint, without claiming SQL/TC completion."""
import json
from pathlib import Path
import shutil
from inventory_rebuilt import ROOT, need, sha, save
import validate_exact_temporal_reference


def main():
    validate_exact_temporal_reference.main()
    base = ROOT / 'docs/phase-2/evidence/mysql-exact-temporal-adapter'
    authorities = []
    for name in ['regression_1.99_20261128_110808_9b3a6807', 'regression_1.85_20261128_111006_2ce9246a', 'reader_checks_20261128']:
        packet = base / name
        for row in json.loads((packet / 'catalog.json').read_bytes()):
            need(sha(packet / row['path']) == row['sha256'], 'Checkpoint evidence changed')
        authorities.append(dict(path=name, catalog_sha256=sha(packet / 'catalog.json')))
        commands = json.loads((packet / 'commands.json').read_bytes())
        need(len(commands) == 2 and all(c['exit_code'] == 0 for c in commands), 'Actual commands failed')
        if name.startswith('regression'):
            result = json.loads((packet / 'result.json').read_bytes())
            need((result['passed'], result['ignored'], result['successful_test_harness_summaries']) == (818, 403, 188), 'Workspace counts differ')
            need(result['strict_mysql_all_targets_clippy'] is True, 'Clippy absent')
            for row in result['production_sources']:
                need(sha(ROOT / row['path']) == row['sha256'], 'Tested Rust source changed')
        else:
            need('positive=15 rejected=24' in (packet / 'run.stdout').read_text(), 'Reader assertions absent')
            for row in json.loads((packet / 'sources.json').read_bytes()):
                need(sha(ROOT / row['path']) == row['sha256'] == sha(packet / 'source' / row['path']), 'Tested Java source changed')
    out = base / 'validation_20261128_final'
    out.mkdir(exist_ok=True)
    save(out / 'result.json', dict(status='DATE_TIME_COMPONENT_CHECKPOINT_VERIFIED', authorities=authorities, rust_versions=['1.99.0', '1.85.0'], passed_per_workspace=818, ignored_per_workspace=403, original_java_datasource_rows=328, full_date_time_sql_undo_or_tc_certified=False, whole_plan_complete=False))
    sources = list((ROOT / 'compat').rglob('*.java'))
    sources += [ROOT / s for s in ['crates/seata_datasource/src/lib.rs', 'crates/seata_datasource/src/struct/mod.rs', 'crates/seata_datasource/src/struct/local_date.rs', 'crates/seata_datasource/src/struct/internal_time.rs', 'crates/seata_datasource/tests/temporal_value_ranges.rs']]
    sources += [p for p in (ROOT / 'tools/formal-source-junit').iterdir() if p.name.startswith(('exact_temporal_', 'validate_exact_temporal_'))]
    save(out / 'source-bindings.json', [dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p)) for p in sources])
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'catalog.json'])
    print('CHECKPOINT_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    main()
