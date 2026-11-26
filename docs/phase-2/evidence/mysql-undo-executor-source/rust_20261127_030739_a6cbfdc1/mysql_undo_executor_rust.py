"""Compare every original Undo executor assertion and exercise production mutations."""
import argparse
import copy
import datetime
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha
from mysql_undo_executor_source import CLASSES, PREFIX


def observations(raw):
    result = {}
    for line in raw.decode('utf-8').splitlines():
        key, encoded = line.split('\t')
        need(key not in result, 'Duplicate original assertion case')
        result[key] = bytes.fromhex(encoded).decode('utf-8').split('\x1f')
    return result


def verify(java, rust, events):
    expected = {PREFIX + cls + '#' + method for cls in CLASSES for method in ['buildUndoSQL', 'getUndoRows']}
    need(set(java) == set(rust) == expected, 'Original case set differs')
    need(len(events) == 7 and events[-1] == dict(kind='summary', passed=6, failed=0, skipped=0),
         'Original JUnit execution incomplete')
    need(len({r['case_id'] for r in events[:-1]}) == 6, 'Duplicate original case identity')
    for case in events[:-1]:
        need(case['kind'] == 'case' and case['status'] == 'SUCCESSFUL', 'Original Java case failed')
        need(case['case_id'] == '[engine:junit-jupiter]/[class:' + case['class'] + ']/[method:' + case['method'] + '()]',
             'Original test identity differs')
    need({r['class'] + '#' + r['method'] for r in events[:-1]} == expected, 'Original namespace/method differs')
    need(sum(map(len, java.values())) == 13, 'Original assertion denominator differs')
    need(java == rust, 'Original assertion values/order differ')


def negative_controls(java, rust, events, raw):
    rejected = []
    key = PREFIX + 'MySQLUndoUpdateExecutorTest#buildUndoSQL'
    for label in ['missing_case', 'duplicate_case', 'wrong_verb', 'wrong_image_identity',
                  'missing_assertion', 'changed_original_namespace', 'forged_green_summary']:
        left, right, rows = copy.deepcopy(java), copy.deepcopy(rust), copy.deepcopy(events)
        if label == 'duplicate_case':
            try:
                observations(raw + raw.splitlines(keepends=True)[0])
            except ValueError:
                rejected.append(label)
                continue
            raise ValueError('Duplicate source case accepted')
        if label == 'missing_case':
            right.pop(key)
        elif label == 'wrong_verb':
            right[key][1] = 'true:false'
        elif label == 'wrong_image_identity':
            right[PREFIX + 'MySQLUndoInsertExecutorTest#getUndoRows'] = ['equals:object:false']
        elif label == 'missing_assertion':
            left[key].pop()
            right[key].pop()
        elif label == 'changed_original_namespace':
            right['unknown.Class#buildUndoSQL'] = right.pop(key)
        else:
            rows[0]['status'] = 'FAILED'
        try:
            verify(left, right, rows)
        except ValueError:
            rejected.append(label)
        else:
            raise ValueError('Weakened source proof accepted: ' + label)
    return rejected


def run(oracle):
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_mysql_undo_source_rust_' + stamp)
    work.mkdir()
    out = ROOT / 'docs/phase-2/evidence/mysql-undo-executor-source' / ('rust_' + stamp)
    out.mkdir(parents=True)
    print('MYSQL_UNDO_EXECUTOR_RUST ' + str(out), flush=True)
    java = json.loads((oracle / 'java-assertions.json').read_bytes())
    events = [json.loads(line) for line in (oracle / 'original-junit-events.jsonl').read_bytes().splitlines()]
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'),
                            CARGO_INCREMENTAL='0', CARGO_PROFILE_TEST_DEBUG='0')
    commands = []

    def command(label, version, selected='original_mysql_undo_', capture=None, expected=0):
        target = ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185')
        extra = dict(CARGO_TARGET_DIR=str(target))
        if capture:
            extra['SEATA_MYSQL_UNDO_EXECUTOR_OBSERVATIONS'] = str(capture)
        argv = ['cargo', '+' + version + '.0', 'test', '-p', 'seata-db-mysql', '--lib', selected, '--offline', '--locked']
        value = subprocess.run(argv, cwd=ROOT, env=env | extra, capture_output=True, timeout=300,
                               creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(value.stdout)
        (out / (label + '.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=argv, exit_code=value.returncode))
        save(out / 'commands.json', commands)
        need(value.returncode == expected, label + ' result differs: ' + value.stderr[-1200:].decode(errors='replace'))
        return value

    results = {}
    for version in ['1.99', '1.85']:
        path = work / (version + '-observations.tsv')
        command(version + '-source', version, capture=path)
        raw = path.read_bytes()
        shutil.copyfile(path, out / path.name)
        verify(java, observations(raw), events)
        controls = negative_controls(java, observations(raw), events, raw)
        save(out / (version + '-negative-controls.json'), controls)
        results[version] = dict(cases=6, assertions=13, negative_controls=len(controls))
    changes = [
        ('wrong-update-image', 'update', b'log.before_image()', b'log.after_image()', 'original_mysql_undo_update_get_undo_rows'),
        ('wrong-delete-verb', 'insert', b'"DELETE FROM {} WHERE {}"', b'"SELECT * FROM {} WHERE {}"', 'original_mysql_undo_insert_build_undo_sql'),
        ('wrong-insert-verb', 'delete', b'"INSERT INTO {} ({}) VALUES ({})"', b'"REPLACE INTO {} ({}) VALUES ({})"', 'original_mysql_undo_delete_build_undo_sql'),
    ]
    mutations = []
    for label, kind, before, after, selected in changes:
        path = ROOT / ('crates/seata_db_mysql/src/mysql/mysql_undo_' + kind + '_executor.rs')
        original = path.read_bytes()
        need(original.count(before) == 1, 'Production mutation site differs')
        try:
            path.write_bytes(original.replace(before, after))
            value = command(label, '1.99', selected=selected, expected=101)
            need(re.search(r'test .*' + selected + r' \.\.\. FAILED', value.stdout.decode('utf-8')) is not None,
                 'Production mutation did not reach target source assertion')
        finally:
            path.write_bytes(original)
        mutations.append(dict(mutation=label, target=selected, rejected=True, original_bytes_restored=True))
    restored = work / 'restored-observations.tsv'
    command('restored', '1.99', capture=restored)
    verify(java, observations(restored.read_bytes()), events)
    save(out / 'mutations.json', mutations)
    save(out / 'result.json', dict(status='MYSQL_UNDO_EXECUTOR_ORIGINAL_ASSERTIONS_MATCH',
         oracle=str(oracle), oracle_catalog_sha256=sha(oracle / 'catalog.json'),
         Rust=results, production_mutations=mutations, independent_human_review='OPEN', WP05_complete=False))
    for path in [Path(__file__), Path(__file__).with_name('mysql_undo_executor_source.py')]:
        shutil.copyfile(path, out / path.name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--oracle', type=Path, required=True)
    run(parser.parse_args().oracle)
