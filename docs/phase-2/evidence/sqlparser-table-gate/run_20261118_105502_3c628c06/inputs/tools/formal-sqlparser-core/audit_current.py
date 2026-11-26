"""Apply skill audits to the explicit module without traversing historical evidence copies."""
import dataclasses
import json
import sys
from pathlib import Path
from freeze import ROOT, DOC, MODULE
from verify import need


def run():
    skill = Path('C:/Users/hnxyh/.agents/skills/rust-java-migration-testing/scripts')
    sys.path.insert(0, str(skill))
    import audit_migration_tests as audit
    # The supplied skill emits native Windows separators, while the portable ledger uses POSIX paths.
    original = [dataclasses.replace(item, file=Path(item.file).as_posix())
                for item in audit.extract_java_tests(MODULE)]
    assets = audit.discover_java_test_assets(MODULE, [])
    parity = audit.validate_source_parity_manifest(DOC/'source-test-parity.json', MODULE, ROOT, original, assets)
    ledger = audit.parse_object_ledger(DOC/'对象级对照表.md')
    code_layout = []
    for name in ['seata_sqlparser', 'seata_test']:
        code_layout.extend(audit.rust_layout_errors(ROOT/'crates'/name))
        code_layout.extend(audit.rust_layout_warnings(ROOT/'crates'/name))
    need(len(original) == 76 and len(assets) == 0, 'Source denominator changed')
    need(ledger.rows_scanned == 31 and ledger.state_counts['IMPLEMENTED'] == 9 and
         ledger.state_counts['PARTIAL'] == 2 and ledger.state_counts['MISSING'] == 20, 'Current ledger drift')
    need(parity.mapped_java_tests == 76, 'Unmapped source test identity')
    need(ledger.migration_completion_blocked and parity.migration_completion_blocked,
         'Incomplete module must not pass the full completion audit')
    need(not code_layout, 'New Rust code has layout blockers')
    docs = []
    for path in sorted(DOC.glob('*.md')):
        text = path.read_text('utf-8')
        count = sum(bool(line.strip()) for line in text.splitlines())
        need(count >= 45 and text.count('\n## ') >= 3 and
             '9 IMPLEMENTED、2 PARTIAL、20 MISSING' in text, 'Current four-document contract drift')
        docs.append(dict(path=path.relative_to(ROOT).as_posix(), nonblank_lines=count))
    result = dict(status='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED',
                  source_tests=76, source_assets=0, module_layout_findings=code_layout,
                  object_ledger=dataclasses.asdict(ledger), source_parity=dataclasses.asdict(parity),
                  current_docs=docs,
                  policy_overrides=['User snake_case directory rule overrides skill seata-test directory name; Cargo package remains seata-test'],
                  note='Skill functions scoped to original Java module and new Rust crates; historical evidence trees intentionally not scanned as live code. Source keys normalized to portable POSIX separators.')
    target = ROOT/'docs/phase-2/evidence/sqlparser-core-values-gate/module-audit-current-20261118.json'
    need(not target.exists(), 'Do not overwrite the module audit')
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(status=result['status'], source_tests=76, source_assets=0,
                         full_module_errors=len(parity.errors), new_layout_errors=0,
                         object_states=ledger.state_counts)), flush=True)


if __name__ == '__main__':
    run()
