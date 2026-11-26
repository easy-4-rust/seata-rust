"""Audit the current full module ledger while keeping partial completion blocked."""
import dataclasses
import json
import sys
from pathlib import Path
from freeze import ROOT, DOC, MODULE
from qualify import read
from verify import need


def run():
    sys.path.insert(0, 'C:/Users/hnxyh/.agents/skills/rust-java-migration-testing/scripts')
    import audit_migration_tests as audit
    originals = [dataclasses.replace(item, file=Path(item.file).as_posix()) for item in audit.extract_java_tests(MODULE)]
    assets = audit.discover_java_test_assets(MODULE, [])
    parity = audit.validate_source_parity_manifest(DOC/'source-test-parity.json', MODULE, ROOT, originals, assets)
    ledger = audit.parse_object_ledger(DOC/'对象级对照表.md')
    need(len(originals) == parity.mapped_java_tests == 76 and len(assets) == 0, 'Original source mapping floor differs')
    need(ledger.rows_scanned == 31 and ledger.state_counts['IMPLEMENTED'] == 12 and ledger.state_counts['MISSING'] == 19
         and ledger.state_counts['PARTIAL'] == 0, 'Current object ledger differs')
    need(ledger.migration_completion_blocked and parity.migration_completion_blocked, 'Missing module work must block completion')
    need(not any('unsafe manifest path' in message or 'source test was not found' in message for message in parity.errors), 'Structural manifest error')
    facts = read(DOC/'source-test-parity.json')['source_tests']
    need(sum(row['disposition'] == 'MIRRORED' and row['result_parity'] == 'MATCH' for row in facts) == 41
         and sum(row['disposition'] == 'MISSING' for row in facts) == 35, 'Current source case split differs')
    documents = []
    for path in sorted(DOC.glob('*.md')):
        current = path.read_text('utf-8').split('<!-- historical-design-appendix-start -->', 1)[0]
        need('12 IMPLEMENTED、19 MISSING' in current and sum(bool(line.strip()) for line in current.splitlines()) >= 45
             and current.count('\n## ') >= 3, 'Current document facts insufficient')
        documents.append(dict(path=path.relative_to(ROOT).as_posix(), current_nonblank_lines=sum(bool(line.strip()) for line in current.splitlines())))
    result = dict(status='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED', objects=dataclasses.asdict(ledger),
        source_parity=dataclasses.asdict(parity), current_documents=documents,
        note='User snake_case directory override remains explicit; 19 MISSING objects and35 MISSING source cases prevent full acceptance')
    target = ROOT/'docs/phase-2/evidence/sqlparser-column-gate/current-module-final-audit-20261118.json'
    need(not target.exists(), 'Do not overwrite an earlier audit')
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(status=result['status'], mapped_original_methods=76, MATCH=41, MISSING=35,
                         completion_errors=len(parity.errors))), flush=True)


if __name__ == '__main__':
    run()
