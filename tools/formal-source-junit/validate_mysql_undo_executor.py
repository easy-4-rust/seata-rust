"""Read-only cross-validation of the original/source/real-database evidence."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import uuid
import urllib.parse
import zipfile

from inventory_rebuilt import BASELINE, ROOT, need, safe_path, save, sha
from mysql_undo_executor_rust import negative_controls, observations, verify
from mysql_undo_executor_source import CLASSES, PREFIX
from portable_rebuilt import verify as verify_portable
from qualify_rebuilt import verify_catalog


def run(inputs):
    config = json.loads(inputs.read_bytes())
    roots = {}
    for name, row in config.items():
        folder = safe_path(ROOT, row['path'])
        verify_catalog(folder, row['catalog_sha256'])
        roots[name] = folder
    oracle, rust, live = [roots[name] for name in ['oracle', 'rust', 'live']]
    package = BASELINE / 'portable_inputs_20261127'
    manifest = verify_portable(package)
    members = {r['member']: r['sha256'] for r in manifest['files']}
    java = json.loads((oracle / 'java-assertions.json').read_bytes())
    events = [json.loads(line) for line in (oracle / 'original-junit-events.jsonl').read_bytes().splitlines()]
    fixture = json.loads((ROOT / 'crates/seata_db_mysql/tests/fixtures/mysql_undo_executor_source.json').read_bytes())
    snapshots = json.loads((oracle / 'original-inputs.json').read_bytes())
    need(len(snapshots) == 12 and all(r['input'] == fixture for r in snapshots), 'Original input observation differs')
    for name in ['1.99', '1.85']:
        raw = (rust / (name + '-observations.tsv')).read_bytes()
        verify(java, observations(raw), events)
        need(len(negative_controls(java, observations(raw), events, raw)) == 7, 'Assertion controls incomplete')
    with zipfile.ZipFile(rust / 'executed-artifacts.zip') as artifacts:
        records = list(rust.glob('*-artifact.json'))
        need(len(records) == 6, 'Original, mutated and restored executable bindings missing')
        for path in records:
            row = json.loads(path.read_bytes())
            data = artifacts.read(row['sealed_member'])
            need(len(data) == row['bytes'] and hashlib.sha256(data).hexdigest() == row['sha256'],
                 'Executed Rust artifact snapshot differs')
    commands = json.loads((oracle / 'commands.json').read_bytes())
    original_command = next(r for r in commands if r['label'] == 'original-junit')
    cp = [Path(p) for p in original_command['argv'][original_command['argv'].index('-cp') + 1].split(os.pathsep)]
    work = cp[0].parent
    need(not (work / 'guard-denials.jsonl').exists() or (work / 'guard-denials.jsonl').stat().st_size == 0,
         'Original test guard denied an operation')
    sources = json.loads((oracle / 'source-bindings.json').read_bytes())
    need(len(sources) == 4, 'Original test/base source set differs')
    for row in sources:
        path = Path(row['path'])
        member = path.relative_to(work).as_posix()
        need(sha(path) == row['sha256'] == members[member], 'Original test source not pinned')
        if path.name != 'BaseExecutorTest.java':
            expected = path.read_text(encoding='utf-8').replace('Assertions.', 'seata.verification.MysqlUndoExecutorAssertions.')
            observed = work / 'observed-source' / path.name
            need(observed.read_text(encoding='utf-8') == expected, 'Observed derivative changed more than assertion dispatch')
    origins = {}
    def origin_path(value):
        path = urllib.parse.unquote(urllib.parse.urlparse(value).path)
        if re.match(r'^/[A-Za-z]:', path):
            path = path[1:]
        return Path(path).resolve()
    for role in ['original', 'observed']:
        log = (oracle / (role + '-class-load.log')).read_text(encoding='utf-8')
        loaded = {m[1]: m[2].replace('\\', '/') for m in re.finditer(r'\[class,load\] (\S+) source: ([^\r\n]+)', log)}
        for cls in CLASSES:
            need(origin_path(loaded[PREFIX + cls]) == (work / role).resolve(), 'Original/observed class origin differs')
            production = PREFIX + cls[:-len('Test')]
            location = work / 'stage/source/rm-datasource/target/classes'
            need(origin_path(loaded[production]) == location.resolve(), 'Production executor came from fallback')
            path = location / (production.replace('.', '/') + '.class')
            need(sha(path) == members[path.relative_to(work).as_posix()], 'Original production class drift')
        origins[role] = {name: loaded[name] for name in loaded if name.startswith(PREFIX + 'MySQLUndo')}
    live_state = json.loads((live / 'result.json').read_bytes())
    need(live_state['verified'] and live_state['cases_per_version'] == 5 and
         live_state['Rust_versions'] == ['1.99', '1.85'] and live_state['native_SQL_builder_mutation_rejected'] and
         live_state['restored_target_passed'], 'Real database scope incomplete')
    need(json.loads((live / 'cleanup.json').read_bytes())['owned_container_removed'], 'Owned database cleanup missing')
    need((live / 'server-after.stdout').read_text().splitlines() == ['0', '0'], 'Restored database/session state differs')
    wire = (live / 'mysql-compensation-statements.stdout').read_bytes()
    for sql in [b'Execute\tDELETE FROM `undo_work` WHERE `id` = 1',
                b'Prepare\tUPDATE `undo_business` SET `v` = ?, `amount` = ?, `payload` = ? WHERE `a` = ? AND `z` = ?',
                b'Prepare\tINSERT INTO `undo_business` (`v`, `amount`, `payload`, `a`, `z`) VALUES (?, ?, ?, ?, ?)']:
        need(sql in wire, 'Actual SQLx compensation statement/order missing')
    mutation_error = (live / 'mutated-update-sql-target.stderr').read_bytes()
    need(b'Undo update compensation' in mutation_error and b'1064' in mutation_error and b'42000' in mutation_error,
         'SQL builder mutation not rejected by real MySQL')
    for name, version in [('regression_199', '1.99'), ('regression_185', '1.85')]:
        result = json.loads((roots[name] / 'result.json').read_bytes())
        need(result['Rust'] == version and result['strict_mysql_all_targets_clippy'] and result['ignored'] == 318,
             'Dual workspace/strict Clippy regression incomplete')
        for row in result['production_sources']:
            need(sha(ROOT / row['path']) == row['sha256'], 'Regression production source drift')
    # The isolated lib-test recorder is not compiled into the live integration-test library.
    live_unconsumed = 'crates/seata_db_mysql/src/mysql/mysql_undo_executor_source.rs'
    module_text = (ROOT / 'crates/seata_db_mysql/src/mysql/mod.rs').read_text()
    need('#[cfg(test)]\nmod mysql_undo_executor_source;' in module_text, 'Live recorder exclusion is no longer test-only')
    exclusions = []
    for folder in [rust, live]:
        for row in json.loads((folder / 'source-inputs.json').read_bytes()):
            if folder == live and row['path'] == live_unconsumed:
                exclusions.append(dict(path=row['path'], recorded_sha256=row['sha256'], current_sha256=sha(ROOT / row['path']),
                     reason='cfg(test) unit observation encoder; not consumed by live integration-test library'))
                continue
            need(sha(ROOT / row['path']) == row['sha256'], 'Executed Rust input changed: ' + row['path'])
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-undo-executor-source' / ('validation_' + stamp)
    out.mkdir(parents=True)
    with zipfile.ZipFile(out / 'compiled-Java-observers-and-tests.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for role in ['helper', 'original', 'observed']:
            for path in sorted((work / role).rglob('*.class')):
                archive.writestr(role + '/' + path.relative_to(work / role).as_posix(), path.read_bytes())
    save(out / 'class-origins.json', origins)
    save(out / 'unconsumed-live-unit-recorder.json', exclusions)
    shutil.copyfile(inputs, out / 'inputs.json')
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'result.json', dict(status='MYSQL_UNDO_EXECUTOR_PROGRAMMATIC_READONLY_REVIEW_PASSED',
         original_cases=6, assertions=13, Rust_versions=['1.99', '1.85'], controls_per_version=7,
         source_mutations_rejected=3, real_database_cases_per_version=5,
         real_SQL_builder_mutation_rejected=True, original_inputs_preserved=True,
         current_consumed_production_and_fixture_bytes_match_executed_inputs=True,
         independent_human_or_other_agent_review='OPEN', full_Java_objects_migrated=False,
         global_Rust_MATCH_totals_recomputed=False, WP05_complete=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(validation=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--inputs', type=Path, required=True)
    run(parser.parse_args().inputs)
