"""Read-only cross-packet and expected-failure audit, after the execution parent has sealed."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text('utf-8'))


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(run):
    run = run.resolve()
    need(run.is_relative_to(ROOT / 'docs/phase-2/evidence/mysql-execution-gate'), 'Unexpected parent location')
    members = read(run / 'catalog.json')['members']
    for row in members:
        path = run / row['path']
        need(path.resolve().is_relative_to(run) and sha(path) == row['sha256'], 'Parent member differs')
    source_rows = read(run / 'source-inputs.json')
    parent = {Path(x['path']).resolve(): x['sha256'] for x in source_rows}
    for row in source_rows:
        need(sha(run / row['snapshot']) == row['sha256'], 'Parent source snapshot differs')
    # 父包没有直接列入原Java schema；将唯一额外现场输入绑定到此前封存的原字节。
    baseline = ROOT / 'docs/phase-2/evidence/mysql-row/run_20261114_045819_c62f4e63'
    need(sha(baseline / 'catalog.json') == '10285b1534c237d5f1eab8944aa039851a70a9798eedd0f80a66bccdb1993ea6', 'Original baseline catalog differs')
    baseline_index = next(x for x in read(baseline / 'catalog.json')['members'] if x['path'] == 'live-children.json')
    need(sha(baseline / 'live-children.json') == baseline_index['sha256'], 'Baseline child index differs')
    old_child = read(baseline / 'live-children.json')[0]
    old_directory = Path(old_child['path'])
    need(sha(old_directory / 'catalog.json') == old_child['catalog_sha256'], 'Original fixture catalog differs')
    fixture = next(x for x in read(old_directory / 'catalog.json')['members'] if x['path'] == 'java-undo.sql')
    need(sha(old_directory / 'java-undo.sql') == fixture['sha256'], 'Original fixture bytes differ')
    oracle_path = ROOT.parent / 'seata-java-phase09-oracle/script/client/at/db/mysql.sql'
    need(sha(oracle_path) == fixture['sha256'], 'Current original schema differs')
    expected = parent | {oracle_path.resolve(): fixture['sha256']}
    positives = []
    for child in read(run / 'live-children.json'):
        directory = Path(child['path']).resolve()
        need(sha(directory / 'catalog.json') == child['catalog_sha256'], 'Child catalog differs')
        for row in read(directory / 'catalog.json')['members']:
            need(sha(directory / row['path']) == row['sha256'], 'Child member differs')
        rows = read(directory / 'source-inputs.json')
        for row in rows:
            need(sha(directory / row['snapshot']) == row['sha256'], 'Child source archive differs')
            need(expected.get(Path(row['path']).resolve()) == row['sha256'], 'Positive child source differs from parent/original fixture epoch')
        artifact = read(directory / 'test-artifact.json')
        need(sha(directory / artifact['snapshot']) == artifact['sha256'], 'Positive child executed archive differs')
        positives.append(dict(rust=child['rust'], suite=child['suite'], source_inputs_bound=len(rows),
            catalog_sha256=child['catalog_sha256'], executed_artifact_sha256=artifact['sha256']))
    reasons = {
        'literal-cast': [b'adjacent_old_row_captured=true', b'assertion `left == right` failed'],
        'undo-count-poison': [b'assertion failed: matches!(tx.table_meta("execution_work").await, Err(DbError::Poisoned))'],
        'undo-payload': [b'assertion failed: matches!(tx.execute', b'Err(DbError::Image(ImageError::Budget'],
        'after-original-where': [b'after image must follow captured primary keys', b'Undo image direction or ordered primary identities'],
        'execute-transaction-owner': [b'cancel_query_packet_hex=', b'actual server session must clean temporary objects without proxy shutdown: Elapsed'],
        'temporary-shadow': [b'assertion failed: !temporary.starts_with(&format!("CREATE TEMPORARY TABLE `{business}` "))'],
    }
    negatives = []
    for control in read(run / 'live-controls.json'):
        directory = run / 'live-controls' / control['name'] / 'child'
        source_root = Path(control['actual_child']).resolve().parents[4]
        changed = Path(control['executed_source']).resolve()
        rows = read(directory / 'source-inputs.json')
        bound = mutations = 0
        for row in rows:
            path = Path(row['path']).resolve()
            need(sha(directory / row['snapshot']) == row['sha256'], 'Negative executed archive differs')
            mapped = ROOT / path.relative_to(source_root) if path.is_relative_to(source_root) else path
            need(mapped in expected, 'Unbound negative input: ' + str(mapped))
            if path == changed:
                need(row['sha256'] == control['sha256'] and row['sha256'] != parent[mapped], 'Declared mutation is absent or differs')
                mutations += 1
            else:
                need(row['sha256'] == expected[mapped], 'Undeclared negative source modification: ' + str(mapped))
                bound += 1
        need(mutations == 1, 'Expected exactly one mutated source per live control')
        artifact = read(directory / 'test-artifact.json')
        need(sha(directory / artifact['snapshot']) == artifact['sha256'], 'Negative executed artifact differs')
        log = (directory / 'live-tests.log').read_bytes()
        need(control['target'].encode() in log and b'test result: FAILED. 0 passed; 1 failed;' in log, 'Wrong failed target/count')
        for required in reasons[control['name']]:
            need(required in log, 'Expected failure reason missing: ' + control['name'] + ': ' + required.decode())
        negatives.append(dict(name=control['name'], target=control['target'], unchanged_inputs_bound=bound,
            mutated_inputs=mutations, mutation_sha256=control['sha256'], executed_artifact_sha256=artifact['sha256'],
            failure_kind='expected_cleanup_timeout_assertion' if control['name'] == 'execute-transaction-owner' else 'explicit_target_assertion',
            expected_diagnostics=[x.decode() for x in reasons[control['name']]]))
    need(len(positives) == 8 and len(negatives) == 6, 'Incomplete cross-packet audit')
    return dict(status='strict_cross_packet_and_failure_reason_audit_passed', parent=str(run),
        catalog_sha256=sha(run / 'catalog.json'), sealed_members=len(members), frozen_parent_inputs=len(source_rows),
        audit_source_sha256=sha(Path(__file__)), positives=positives, negatives=negatives,
        additional_live_only_input=dict(path=str(oracle_path),sha256=fixture['sha256'],
            original_sealed_fixture=str(old_directory/'java-undo.sql'),baseline_catalog_sha256=sha(baseline/'catalog.json')),
        note='Standalone verifier: six target failures are tied to declared mutations and expected diagnostics, not generic panic counts.',
        source_case_parity_complete=False, WP05_complete=False)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.run), ensure_ascii=False, indent=2))
