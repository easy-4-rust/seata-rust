"""Run the complete affected Rust workspace and strict MySQL Clippy per toolchain."""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha


def run(version):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-timestamp-restart' / ('regression_' + version + '_' + stamp)
    out.mkdir(parents=True)
    print('TIMESTAMP_COLUMN_REGRESSION ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'),
        CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0',
        CARGO_TARGET_DIR=str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185')))
    for key in ['SEATA_MYSQL_UNDO_EXECUTOR_OBSERVATIONS', 'RUSTC', 'RUSTDOC']:
        env.pop(key, None)
    commands = []
    summaries = []
    for label, arguments in [('workspace', ['test', '--workspace', '--all-features', '--offline', '--locked']),
                              ('clippy', ['clippy', '-p', 'seata-db-mysql', '-p', 'seata-observability', '-p', 'seata-at', '-p', 'seata-datasource', '--all-targets', '--all-features', '--offline', '--locked', '--', '-D', 'warnings'])]:
        argv = ['cargo', '+' + version + '.0', *arguments]
        value = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=600,
                               creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(value.stdout)
        (out / (label + '.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=argv, exit_code=value.returncode))
        save(out / 'commands.json', commands)
        need(value.returncode == 0, label + ' regression failed: ' + value.stderr[-1600:].decode(errors='replace'))
        if label == 'workspace':
            summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', value.stdout)
            need(summaries and not any(int(failed) for _, failed, _ in summaries), 'Workspace test summaries incomplete')
    save(out / 'result.json', dict(status='RUST_WORKSPACE_AND_STRICT_MYSQL_CLIPPY_PASSED', Rust=version,
         passed=sum(int(passed) for passed, _, _ in summaries), ignored=sum(int(ignored) for _, _, ignored in summaries),
         successful_test_harness_summaries=len(summaries), includes_rustdoc_summaries=True,
         all_features=True, strict_mysql_all_targets_clippy=True,
         production_sources=[dict(path=p.relative_to(ROOT).as_posix(), sha256=sha(p))
             for p in [ROOT / name for name in ['crates/seata_datasource/src/struct/timestamp.rs', 'crates/seata_datasource/src/struct/local_date_time.rs', 'crates/seata_datasource/src/struct/column_domain.rs', 'crates/seata_datasource/src/struct/column_type.rs', 'crates/seata_datasource/src/struct/field.rs', 'crates/seata_datasource/src/struct/field_value.rs', 'crates/seata_datasource/src/struct/mysql_type.rs', 'crates/seata_at/src/exec/base_transactional_executor.rs', 'crates/seata_at/src/undo/parser/prepared_format.rs', 'crates/seata_at/src/undo/parser/restored_format.rs', 'crates/seata_at/src/undo/parser/mysql_jdbc_profile.rs', 'crates/seata_db_mysql/src/mysql_image_reader.rs', 'crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs', 'crates/seata_db_mysql/src/mysql/mysql_insert_executor.rs', 'crates/seata_db_mysql/src/mysql_commit_attempt.rs', 'crates/seata_db_mysql/src/mysql_commit_attempt_store.rs', 'crates/seata_db_mysql/src/mysql_resource_binding.rs', 'crates/seata_db_mysql/src/rm_branch_store.rs', 'crates/seata_db_mysql/tests/datetime_column.rs', 'crates/seata_db_mysql/tests/live_datetime_java.rs', 'crates/seata_db_mysql/src/mysql_options.rs', 'crates/seata_db_mysql/tests/live_timestamp_java.rs', 'crates/seata_db_mysql/tests/timestamp_column.rs', 'crates/seata_db_mysql/tests/live_timestamp_restart_java.rs', 'crates/seata_db_mysql/tests/support/timestamp_owned_process.rs']]]))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print((out / 'result.json').read_text(encoding='utf-8'), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--rust', choices=['1.99', '1.85'], required=True)
    run(parser.parse_args().rust)
