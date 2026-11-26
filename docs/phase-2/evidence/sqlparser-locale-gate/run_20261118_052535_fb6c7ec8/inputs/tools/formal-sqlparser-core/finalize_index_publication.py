"""Bind the current ledgers to the immutable run and independently reported read-only review."""
import json
import re
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need
from publish_index import GATE


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def finalize():
    family = GATE.parent; catalog = read(GATE/'catalog.json')['members']
    need(sha(GATE/'catalog.json') == '6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774', 'Gate drift')
    for row in catalog: need(sha(GATE/row['path']) == row['sha256'], 'Frozen member drift')
    # Production/test sources must still be the exact dual-toolchain verified bytes.
    for row in read(GATE/'verification/inputs.json'):
        if row['path'].startswith('crates/') or row['path'] in {'Cargo.toml', 'Cargo.lock'}:
            need(sha(ROOT/row['path']) == row['sha256'], 'Verified production/test source drift')
    log = family/'explicit-acceptance-package-20261118.log'
    need(sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log.read_text('utf-8')))) == 95, 'Package floor differs')
    audit = read(family/'current-module-final-audit-20261118.json')
    need(audit['status'] == 'FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED', 'Missing full-module block')
    review = dict(reviewer='/root/sqlparser_values_review', mode='READ_ONLY', date='2026-11-18',
        confirmed_remaining_P1=0, confirmed_remaining_P2=0,
        verified=['534 catalog members and444 inputs, parent catalog',
            'Actual both-toolchain434 ordinary/89 harness/143 ignored/4doc',
            '57 original source differences,21+144 computed observations',
            '4 identity inputs explicitly INPUT_NOT_RESULT, not MATCH',
            '24 actual commands and9 runtime mutants:6 assertion panics and3 target behavior errors; none compilation errors',
            'Original JUnit76/13 negative controls and8 comparison counterexamples independently recomputed',
            '219 original runtime bindings and actual producer source/class provenance',
            'Qualification has no confirmed omission or inflated count'],
        cohesive_file=dict(path='crates/seata_sqlparser/src/struct/column_meta.rs', lines=608,
            conclusion='Single Java object and private state, reviewed501–800 line exception'),
        scope=dict(IMPLEMENTED=13, MISSING=18, original_cases_MATCH=57, original_cases_MISSING=19),
        full_core_complete=False, full_V1_complete=False)
    target = family/'readonly-review-20261118.json'; need(not target.exists(), 'Do not overwrite a review'); save(target, review)
    files = [ROOT/'README.md', ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'docs/delivery-progress.md',
        ROOT/'docs/phase-2/wp05-sqlparser-index.json', family/'README.md', target,
        family/'current-module-final-audit-20261118.json', log]
    for directory in [DOC, ROOT/'tools/formal-sqlparser-core', ROOT/'crates/seata_sqlparser', ROOT/'crates/seata_test']:
        files.extend(path for path in directory.rglob('*') if path.is_file())
    publication = dict(status='INDEX_METADATA_AND_TYPED_CALLBACKS_PUBLISHED_FULL_GOAL_OPEN',
        gate=GATE.relative_to(ROOT).as_posix(), catalog_sha256=sha(GATE/'catalog.json'), members=len(catalog), verification_inputs=444,
        object_states=dict(IMPLEMENTED=13, MISSING=18), original_case_states=dict(MATCH=57, MISSING=19),
        computed_original_edges_MATCH=165, JVM_identity_inputs_not_MATCH=4, package_test_execution_passed=95,
        post_gate_change='Only current ledgers/docs, evidence qualification/publication tools and explicit package run; production/test bytes unchanged',
        full_core_complete=False, WP05_complete=False, WP06_complete=False, full_V1_complete=False,
        TC_safety_gate_passed=False, external_TC_risk_accepted=False, shared_lock_issue='deferred_by_user',
        files=[dict(path=path.relative_to(ROOT).as_posix(), sha256=sha(path)) for path in sorted(set(files))])
    target = family/'publication-20261118.json'; need(not target.exists(), 'Do not overwrite publication'); save(target, publication)
    for row in publication['files']: need(sha(ROOT/row['path']) == row['sha256'], 'Published file drift')
    print(json.dumps(dict(publication=target.relative_to(ROOT).as_posix(), sha256=sha(target), files=len(publication['files']),
        source_MATCH=57, source_MISSING=19, full_core_complete=False)), flush=True)


if __name__ == '__main__': finalize()
