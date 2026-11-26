"""Checksum the current column delivery and the independent read-only review."""
import json
import re
import subprocess
import sys
from pathlib import Path
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need

GATE = ROOT/'docs/phase-2/evidence/sqlparser-column-gate/run_20261118_030159_4362448c'
FAMILY = GATE.parent


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def finalize():
    need(sha(GATE/'catalog.json') == '1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da', 'Gate catalog changed')
    for row in read(GATE/'catalog.json')['members']:
        need(sha(GATE/row['path']) == row['sha256'], 'Frozen gate drift')
    log = FAMILY/'explicit-acceptance-package-20261118.log'
    need(sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log.read_text('utf-8')))) == 67, 'Current explicit package floor differs')
    target = DOC/'verification/column-acceptance-package.log'
    need(not target.exists(), 'Do not overwrite an earlier package run')
    target.write_bytes(log.read_bytes())
    manifest = read(DOC/'source-test-parity.json')
    manifest['acceptance_module'].update(command='cargo test --offline --locked -p seata-test --all-features',
        artifact='verification/column-acceptance-package.log', skipped=0,
        scoped_test_execution=dict(status='PASS', ordinary_passed=67, failed=0, skipped=0),
        scope='Explicit package execution passed; source migration remains41 of76 with35 MISSING')
    save(DOC/'source-test-parity.json', manifest)
    code = subprocess.run([sys.executable, str(ROOT/'tools/formal-sqlparser-core/audit_column_current.py')],
        cwd=ROOT, timeout=60, creationflags=subprocess.CREATE_NO_WINDOW).returncode
    need(code == 0, 'Current full-denominator audit did not retain the expected block')
    review = dict(reviewer='/root/sqlparser_values_review', mode='READ_ONLY', date='2026-11-18',
        confirmed_remaining_P1=0, confirmed_remaining_P2=0,
        confirmed_and_fixed_P2=['Debug calls user formatting while holding a metadata read lock',
                                'Whole-state snapshot loses tail changes made by Object.equals/hashCode/toString callbacks'],
        verified=['All483 gate members and441 parent members', 'All422 frozen inputs and219 original runtime inputs',
                  'Oracle source/class binding, actualJUnit76 and13 negative controls',
                  'Both toolchains406 ordinary/85 harness/143 ignored/4doc',
                  'Actual41 source and21 edge observations independently decoded and compared',
                  'Five mutants with8 real runtime failures;8 comparison controls recomputed',
                  'Original18 ColumnMeta assertions and rawUTF16/cause identity/old API preservation'],
        cohesive_file=dict(path='crates/seata_sqlparser/src/struct/column_meta.rs', lines=606,
            conclusion='One Java ColumnMeta and private state; field accessors plus equals/hash/toString; reviewed501–800 line exception'),
        scope=dict(IMPLEMENTED=12, MISSING=19, original_cases_MATCH=41, original_cases_MISSING=35),
        full_core_complete=False, full_V1_complete=False)
    save(FAMILY/'readonly-review-20261118.json', review)
    files = [ROOT/'README.md', ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'docs/delivery-progress.md',
             ROOT/'docs/phase-2/wp05-sqlparser-column.json', FAMILY/'README.md',
             FAMILY/'readonly-review-20261118.json', FAMILY/'current-module-audit-20261118.json',
             FAMILY/'current-module-final-audit-20261118.json', log]
    for directory in [DOC, ROOT/'tools/formal-sqlparser-core', ROOT/'crates/seata_sqlparser', ROOT/'crates/seata_test']:
        files.extend(path for path in directory.rglob('*') if path.is_file())
    publication = dict(status='COLUMN_METADATA_AND_UTF16_PUBLISHED_FULL_GOAL_OPEN', gate=GATE.relative_to(ROOT).as_posix(),
        catalog_sha256=sha(GATE/'catalog.json'), members=483, verification_inputs=422,
        object_states=dict(IMPLEMENTED=12, MISSING=19), original_case_states=dict(MATCH=41, MISSING=35),
        additional_original_edge_observations_MATCH=21, package_test_execution_passed=67,
        full_core_complete=False, WP05_complete=False, WP06_complete=False, full_V1_complete=False,
        TC_safety_gate_passed=False, external_TC_risk_accepted=False, shared_lock_issue='deferred_by_user',
        post_gate_tooling_change='run_column_oracle now preserves an existing identical derived fixture instead of failing; it refuses differing bytes. No Rust or actual Java producer source changed after final gate.',
        current_module_audit='Full-denominator completion remains blocked; user snake_case directory override is explicit, not a waiver for missing source cases',
        files=[dict(path=path.relative_to(ROOT).as_posix(), sha256=sha(path)) for path in sorted(set(files))])
    target = FAMILY/'publication-20261118.json'
    need(not target.exists(), 'Do not overwrite an existing publication')
    save(target, publication)
    print(json.dumps(dict(publication=str(target.relative_to(ROOT)), sha256=sha(target), files=len(publication['files']),
                         original_cases_MATCH=41, source_cases_MISSING=35, full_core_complete=False)), flush=True)


if __name__ == '__main__':
    finalize()
