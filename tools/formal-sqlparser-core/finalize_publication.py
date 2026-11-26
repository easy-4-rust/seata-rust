"""Publish portable audit references and checksum the current honest partial state."""
import json
import subprocess
import sys
from pathlib import Path
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need

GATE = ROOT/'docs/phase-2/evidence/sqlparser-core-values-gate/run_20261118_022222_bb4f2677'
FAMILY = GATE.parent


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def finalize():
    need(sha(GATE/'catalog.json') == 'b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd', 'Gate catalog drift')
    for row in read(GATE/'catalog.json')['members']:
        need(sha(GATE/row['path']) == row['sha256'], 'Frozen gate member drift')
    verification = DOC/'verification'
    verification.mkdir()
    aliases = []
    for source, name in [
        (GATE/'result.json', 'result.json'),
        (GATE/'verification/1.99-differential.json', '1.99-differential.json'),
        (GATE/'verification/rust-commands.json', 'rust-commands.json'),
        (GATE/'oracle/commands.json', 'java-commands.json'),
        (GATE/'oracle/java-constants.tsv', 'java-constants.tsv'),
        (FAMILY/'explicit-acceptance-package-20261118.log', 'acceptance-package.log')]:
        target = verification/name
        target.write_bytes(source.read_bytes())
        need(sha(target) == sha(source), 'Audit evidence alias differs')
        aliases.append(dict(path=target.relative_to(DOC).as_posix(), sha256=sha(target),
                            immutable_origin=str(source.relative_to(ROOT))))
    save(verification/'exact-evidence-aliases.json', dict(aliases=aliases,
        purpose='Portable skill manifest paths must remain inside its directory; these are exact byte copies of frozen gate evidence'))
    manifest = read(DOC/'source-test-parity.json')
    for row in manifest['source_tests']:
        if row['disposition'] == 'MIRRORED':
            row['evidence'] = 'verification/1.99-differential.json'
    module = manifest['acceptance_module']
    module.update(command='cargo test --offline --locked -p seata-test --all-features',
                  artifact='verification/acceptance-package.log', skipped=0,
                  scoped_test_execution=dict(status='PASS', ordinary_passed=30, failed=0, skipped=0),
                  scope='PARTIAL source migration: 23 of 76 cases; this package test run is not full project migration acceptance')
    manifest['runs']['java']['artifact'] = 'verification/java-commands.json'
    manifest['runs']['rust']['artifact'] = 'verification/rust-commands.json'
    manifest['runs']['differential']['artifact'] = 'verification/1.99-differential.json'
    manifest['generated_Rust_oracle_assets'][0]['source'] = 'verification/java-constants.tsv'
    manifest['policy_overrides'] = ['User requires snake_case directory seata_test; Cargo package is seata-test']
    save(DOC/'source-test-parity.json', manifest)
    code = subprocess.run([sys.executable, str(ROOT/'tools/formal-sqlparser-core/audit_current.py')],
                          cwd=ROOT, timeout=60, creationflags=subprocess.CREATE_NO_WINDOW).returncode
    need(code == 0, 'Honest current module audit did not validate')
    audit = read(FAMILY/'module-audit-current-20261118.json')
    need(audit['source_parity']['mapped_java_tests'] == 76, 'Full source mapping missing')
    need(not any('unsafe manifest path' in message or 'manifest source test was not found' in message
                 for message in audit['source_parity']['errors']), 'Current audit still has structural manifest errors')
    review = dict(reviewer='/root/sqlparser_values_review', mode='READ_ONLY', date='2026-11-18',
        confirmed_P1=0, confirmed_P2=0,
        verified=['11 source objects and all original 23 case assertions',
                  '441 gate members and all 8546 authenticated parent members',
                  '393 frozen verification inputs and 219 bound original runtime inputs',
                  'raw 23-case results in each toolchain and 6 real runtime mutant assertions',
                  '13 original JUnit controls and 4 actual differential controls recomputed independently',
                  'current four docs, inventory, 23/53 parity split and 1112/1304 remaining global source counts'],
        limitations=['Lone surrogate Java String domain remains a real PARTIAL gap',
                     'Portable skill evidence aliases added after review and byte-checked by final publication; gate unchanged'],
        full_module_complete=False)
    save(FAMILY/'readonly-review-20261118.json', review)
    files = [ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'README.md', ROOT/'docs/delivery-progress.md',
             ROOT/'docs/phase-2/wp05-sqlparser-core-values.json', FAMILY/'README.md',
             FAMILY/'module-audit-current-20261118.json', FAMILY/'module-audit-20261118.json',
             FAMILY/'readonly-review-20261118.json', FAMILY/'explicit-acceptance-package-20261118.log']
    for directory in [DOC, ROOT/'tools/formal-sqlparser-core', ROOT/'crates/seata_sqlparser', ROOT/'crates/seata_test']:
        files.extend(path for path in directory.rglob('*') if path.is_file())
    publication = dict(status='FOUNDATION_VALUES_PUBLISHED_FULL_PLAN_OPEN', gate=GATE.relative_to(ROOT).as_posix(),
        catalog_sha256=sha(GATE/'catalog.json'), gate_members=441,
        object_states=dict(IMPLEMENTED=9, PARTIAL=2, MISSING=20), source_cases=dict(MATCH=23, MISSING=53),
        full_module_complete=False, WP05_complete=False, WP06_complete=False, full_V1_complete=False,
        TC_safety_gate_passed=False, external_TC_risk_accepted=False,
        initial_audit_diagnostic='Native Windows source-key separators and parent-directory evidence references did not fit the skill parser. Current audit normalizes portable keys and uses checksum-verified local evidence copies. Original diagnostic retained.',
        current_audit=dict(full_module_completion_blocked=True, mapped_source_methods=76,
                           source_case_denominator=76, outstanding_errors=len(audit['source_parity']['errors']),
                           note='Errors for 53 MISSING cases, partial run scopes and explicit user directory-name override remain; no completion waiver'),
        files=[dict(path=path.relative_to(ROOT).as_posix(), sha256=sha(path)) for path in sorted(set(files))])
    target = FAMILY/'publication-20261118.json'
    need(not target.exists(), 'Do not overwrite an existing publication')
    save(target, publication)
    print(json.dumps(dict(publication=str(target.relative_to(ROOT)), sha256=sha(target),
                          mapped_source_methods=76, MATCH=23, MISSING=53,
                          full_module_complete=False)), flush=True)


if __name__ == '__main__':
    finalize()
