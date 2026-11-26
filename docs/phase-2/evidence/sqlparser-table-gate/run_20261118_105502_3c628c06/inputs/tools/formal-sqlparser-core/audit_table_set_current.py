"""Audit full source denominator after a dependency-only change; never promote TableMeta."""
import dataclasses
import json
import sys
from pathlib import Path
from freeze import ROOT, DOC, MODULE
from qualify import read, sha
from verify import need


def run():
    sys.path.insert(0,'C:/Users/hnxyh/.agents/skills/rust-java-migration-testing/scripts')
    import audit_migration_tests as audit
    originals=[dataclasses.replace(item,file=Path(item.file).as_posix()) for item in audit.extract_java_tests(MODULE)]
    assets=audit.discover_java_test_assets(MODULE,[])
    parity=audit.validate_source_parity_manifest(DOC/'source-test-parity.json',MODULE,ROOT,originals,assets)
    ledger=audit.parse_object_ledger(DOC/'对象级对照表.md')
    need(len(originals)==parity.mapped_java_tests==76 and len(assets)==0,'Original source mapping floor differs')
    need(ledger.rows_scanned==31 and ledger.state_counts['IMPLEMENTED']==13 and ledger.state_counts['MISSING']==18
         and ledger.state_counts['PARTIAL']==0,'Object ledger differs')
    need(ledger.migration_completion_blocked and parity.migration_completion_blocked,'Full module must stay blocked')
    need(not any('unsafe manifest path' in msg or 'source test was not found' in msg for msg in parity.errors),'Structural manifest error')
    inventory=read(DOC/'inventory.json');manifest=read(DOC/'source-test-parity.json')
    need(sum(row['state']=='MIRRORED' and row['differential']=='MATCH' for row in inventory['cases'])==57 and
         sum(row['state']=='MISSING' for row in inventory['cases'])==19,'Source case split differs')
    need(next(row for row in inventory['objects'] if row['java_type'].endswith('.TableMeta'))['state']=='MISSING','TableMeta dependency promoted into parent completion')
    need(manifest['acceptance_module']['scoped_test_execution']['ordinary_passed']==98,'Explicit package floor differs')
    for row in inventory['objects']:
        if row['state']=='IMPLEMENTED':need(sha(ROOT/row['rust'])==row['rust_sha256'],'Original object source drift')
    for row in read(DOC/'verification/table-set-evidence-aliases.json')['aliases']:
        need(sha(DOC/row['path'])==row['sha256']==sha(ROOT/row['origin']),'Alias drift')
    documents=[]
    for path in sorted(DOC.glob('*.md')):
        current=path.read_text('utf-8').split('<!-- historical-design-appendix-start -->',1)[0]
        need('13 IMPLEMENTED、18 MISSING' in current and '57 MATCH、19 MISSING' in current and
             'TableMeta本体仍为MISSING' in current and '23,370计算观察' in current and
             sum(bool(line.strip()) for line in current.splitlines())>=45,'Current documentation insufficient')
        documents.append(dict(path=path.relative_to(ROOT).as_posix(),current_nonblank_lines=sum(bool(line.strip()) for line in current.splitlines())))
    result=dict(status='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED',objects=dataclasses.asdict(ledger),
        source_parity=dataclasses.asdict(parity),current_documents=documents,
        note='Dependency-only progress; TableMeta remains MISSING, source denominator31/76 intact; snake_case override explicit')
    target=ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites-gate/current-module-final-audit-20261118.json'
    need(not target.exists(),'Do not overwrite audit');target.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n','utf-8')
    print(json.dumps(dict(status=result['status'],source_MATCH=57,source_MISSING=19,full_core_complete=False)),flush=True)


if __name__=='__main__':run()
