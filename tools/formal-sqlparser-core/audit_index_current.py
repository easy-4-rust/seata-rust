"""Check the current full31/76 denominator; partial core completion must remain blocked."""
import dataclasses
import json
import sys
from pathlib import Path
from freeze import ROOT, DOC, MODULE
from qualify import read, sha
from verify import need


def run():
    sys.path.insert(0, 'C:/Users/hnxyh/.agents/skills/rust-java-migration-testing/scripts')
    import audit_migration_tests as audit
    originals = [dataclasses.replace(item, file=Path(item.file).as_posix()) for item in audit.extract_java_tests(MODULE)]
    assets = audit.discover_java_test_assets(MODULE, [])
    parity = audit.validate_source_parity_manifest(DOC/'source-test-parity.json', MODULE, ROOT, originals, assets)
    ledger = audit.parse_object_ledger(DOC/'对象级对照表.md')
    need(len(originals) == parity.mapped_java_tests == 76 and len(assets) == 0, 'Original mapping floor differs')
    need(ledger.rows_scanned == 31 and ledger.state_counts['IMPLEMENTED'] == 13 and
         ledger.state_counts['MISSING'] == 18 and ledger.state_counts['PARTIAL'] == 0, 'Object census differs')
    need(ledger.migration_completion_blocked and parity.migration_completion_blocked, 'Missing work must block full acceptance')
    need(not any('unsafe manifest path' in msg or 'source test was not found' in msg for msg in parity.errors), 'Structural manifest error')
    manifest = read(DOC/'source-test-parity.json'); facts = manifest['source_tests']
    need(sum(row['disposition'] == 'MIRRORED' and row['result_parity'] == 'MATCH' for row in facts) == 57 and
         sum(row['disposition'] == 'MISSING' for row in facts) == 19, 'Source split differs')
    need(manifest['acceptance_module']['scoped_test_execution']['ordinary_passed'] == 95, 'Package run floor differs')
    inventory = read(DOC/'inventory.json')
    need(len(inventory['objects']) == 31 and len(inventory['cases']) == 76, 'Full denominator changed')
    for row in inventory['objects']:
        if row['state'] == 'IMPLEMENTED': need(sha(ROOT/row['rust']) == row['rust_sha256'], 'Current source drift')
    for row in read(DOC/'verification/index-evidence-aliases.json')['aliases']:
        need(sha(DOC/row['path']) == row['sha256'] == sha(ROOT/row['origin']), 'Evidence alias drift')
    documents = []
    for path in sorted(DOC.glob('*.md')):
        current = path.read_text('utf-8').split('<!-- historical-design-appendix-start -->', 1)[0]
        need('13 IMPLEMENTED、18 MISSING' in current and '57 MATCH、19 MISSING' in current and
             sum(bool(line.strip()) for line in current.splitlines()) >= 45 and current.count('\n## ') >= 3,
             'Current document insufficient')
        documents.append(dict(path=path.relative_to(ROOT).as_posix(), current_nonblank_lines=sum(bool(x.strip()) for x in current.splitlines())))
    result = dict(status='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED', objects=dataclasses.asdict(ledger),
        source_parity=dataclasses.asdict(parity), current_documents=documents,
        note='User snake_case override remains explicit;18 source objects and19 original cases remain MISSING')
    target = ROOT/'docs/phase-2/evidence/sqlparser-index-gate/current-module-final-audit-20261118.json'
    need(not target.exists(), 'Do not overwrite existing audit')
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(status=result['status'], mapped_original_methods=76, MATCH=57, MISSING=19,
                         completion_errors=len(parity.errors))), flush=True)


if __name__ == '__main__': run()
