"""Read-only closure checks for one rebuilt original Java runtime.

This audit establishes Java event/process evidence, not Rust parity or AST inventory.
"""
import argparse
import hashlib
import json
from pathlib import Path
from validate import load, validate_events, controls
import process_proof


def run(runtime):
    result = json.loads((runtime / 'result.json').read_bytes())
    work = Path(result['work'])
    report = {'runtime': str(runtime), 'modules': {}, 'Rust_parity_assessed': False,
              'AST_method_inventory_verified': False, 'independent_human_review': 'OPEN'}
    cases = []
    for module in ['seata-sqlparser-core', 'rm-datasource']:
        rows = load(runtime / (module + '-events.jsonl'))
        validated = validate_events(rows, module)
        for case in validated.pop('cases'):
            # A fresh Java execution cannot reset or certify earlier Rust bindings.
            case.pop('Rust_disposition')
            case.pop('differential')
            cases.append(case)
        environments = [process_proof.read(p) for p in sorted((runtime / module / 'environments').glob('*.xml'))]
        validated['event_controls'] = controls(rows, module)
        validated['processes'] = process_proof.validate(environments, module, rows[0], work)
        if module == 'rm-datasource':
            validated['process_controls'] = process_proof.controls(environments, rows[0], work)
        report['modules'][module] = validated
    report['concrete_Java_cases'] = len(cases)
    report['runtime_declaring_methods'] = len({(c['declaring_class'], c['method']) for c in cases})
    report['negative_controls_rejected'] = sum(m['event_controls']['count'] + m.get('process_controls', {}).get('count', 0) for m in report['modules'].values())
    report['transcript_sha256'] = {module: hashlib.sha256((runtime / (module + '-events.jsonl')).read_bytes()).hexdigest() for module in report['modules']}
    (runtime / 'rebuilt-Java-cases.jsonl').write_text(''.join(json.dumps(c, ensure_ascii=False) + '\n' for c in cases), encoding='utf-8')
    (runtime / 'closure-audit.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', type=Path, required=True)
    run(parser.parse_args().runtime)
