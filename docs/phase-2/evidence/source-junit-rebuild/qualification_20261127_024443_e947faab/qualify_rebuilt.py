"""Cross-check the sealed AST, portable runtime and original Java case ledgers.

This is programmatic validation and does not replace the independent review gate.
"""
import argparse
import datetime
import json
from pathlib import Path
import shutil
import uuid

from inventory_rebuilt import BASELINE, BUILD, declaration_controls, need, safe_path, save, sha, verify_baseline
from portable_rebuilt import verify
from validate import bind_methods, controls, load, validate_events
import process_proof


def verify_catalog(folder, digest):
    need(sha(folder / 'catalog.json') == digest, 'Explicit catalog authority differs')
    rows = json.loads((folder / 'catalog.json').read_bytes())
    need(len(rows) == len({r['path'] for r in rows}), 'Duplicate catalog member')
    for row in rows:
        need(sha(safe_path(folder, row['path'])) == row['sha256'], 'Evidence member drift')


def qualify(inventory, runtime, inventory_digest, runtime_digest):
    verify_baseline(BASELINE)
    verify_catalog(inventory, inventory_digest)
    verify_catalog(runtime, runtime_digest)
    package = BASELINE / 'portable_inputs_20261127'
    manifest = verify(package)
    literal = {r['path']: r for r in json.loads((BASELINE / BUILD / 'literal-git-inputs.json').read_bytes())}
    inventory_result = json.loads((inventory / 'result.json').read_bytes())
    need(inventory_result['status'] == 'ORIGINAL_AST_AND_JUNIT_METHODS_BOUND' and
         inventory_result['source_files_unchanged'], 'AST inventory did not qualify')
    declarations = json.loads((inventory / 'source-method-bindings.json').read_bytes())
    raw = load(inventory / 'parser-raw-methods.jsonl')
    need(len(raw) == len(declarations), 'AST inventory declaration set changed')
    for method, original in zip(declarations, raw):
        for key in ('source', 'declaring_class', 'method', 'signature', 'line', 'end_line',
                    'annotations', 'direct_superclass_syntax', 'method_checkout_sha256'):
            need(method[key] == original[key], 'Original AST declaration binding drift')
        need(method['source_sha256'] == literal[method['source_git_relative_path']]['sha256'],
             'AST declaration is not bound to original Git bytes')
    extracted = json.loads((runtime / 'extraction.json').read_bytes())
    proof = json.loads((runtime / 'replay-proof.json').read_bytes())
    need(extracted['manifest_sha256'] == sha(package / 'manifest.json') == proof['package_manifest_sha256'],
         'Runtime package authority differs')
    need(proof['status'] == 'PORTABLE_ORIGINAL_JUNIT_EXECUTED' and proof['all_input_bytes_unchanged'] and
         proof['H2_teardown_verified'] and not proof['old_workdir_or_cache_used'] and
         not proof['Maven_or_oracle_used'], 'Portable original run incomplete')
    teardown = json.loads((runtime / 'owned-process-teardown.json').read_bytes())
    need(teardown['owned_JVMs_stopped'], 'Owned JVM teardown not verified')
    groups, negatives = [], []
    actual = json.loads((runtime / 'result.json').read_bytes())
    need(len(actual['modules']) == 2 and all(m['exit_code'] == 0 for m in actual['modules']),
         'Original module execution failed or incomplete')
    for name in ['seata-sqlparser-core', 'rm-datasource']:
        events = load(runtime / (name + '-events.jsonl'))
        groups.append(validate_events(events, name))
        negatives.append(dict(module=name, **controls(events, name)))
        properties = [process_proof.read(p) for p in (runtime / name / 'environments').glob('*.xml')]
        process_proof.validate(properties, name, events[0], Path(actual['work']))
        if name == 'rm-datasource':
            negatives.append(dict(module=name, kind='process', **process_proof.controls(properties, events[0], Path(actual['work']))))
    methods = bind_methods(declarations, groups)
    method_controls = declaration_controls(declarations, groups)
    for method in methods:
        method.pop('Rust_disposition')
        method.pop('differential')
    cases = [c for g in groups for c in g['cases']]
    for case in cases:
        case.pop('Rust_disposition')
        case.pop('differential')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = BASELINE / ('qualification_' + stamp)
    out.mkdir()
    save(out / 'authorities.json', dict(inventory=str(inventory), runtime=str(runtime),
         inventory_catalog_sha256=inventory_digest, runtime_catalog_sha256=runtime_digest,
         package_manifest_sha256=sha(package / 'manifest.json')))
    save(out / 'source-method-bindings.json', methods)
    save(out / 'concrete-source-cases.json', cases)
    save(out / 'negative-controls.json', dict(event_and_process=negatives, method_floor=method_controls))
    save(out / 'result.json', dict(status='REBUILT_ORIGINAL_JAVA_BASELINE_PROGRAMMATICALLY_VERIFIED',
         original_source_files=4269, original_test_java_files=189, original_source_assets=7,
         declared_methods=len(methods), Java_methods_executed=sum(m['Java_status'] == 'PASSED' for m in methods),
         Java_methods_disabled_JDK17=sum(m['Java_status'] == 'SOURCE_DISABLED_JDK17' for m in methods),
         concrete_case_identities=len(cases), Java_concrete_cases_passed=sum(c['Java_status'] == 'PASSED' for c in cases),
         Java_concrete_cases_disabled=sum(c['Java_status'] == 'SOURCE_DISABLED_JDK17' for c in cases),
         negative_controls=sum(v['count'] for v in negatives) + len(method_controls),
         package_members=len(manifest['files']), package_input_bytes=manifest['total_bytes'],
         all_runtime_inputs_portably_sealed=True, original_Surefire_test_goal_executed=False,
         all_JDK_profiles_executed=False, Rust_parity_assessed=False, current_global_Rust_MATCH_totals_recomputed=False,
         independent_human_or_other_agent_review='OPEN', WP05_complete=False, full_V1_complete=False))
    for name in ['qualify_rebuilt.py', 'inventory_rebuilt.py', 'portable_rebuilt.py', 'validate.py', 'process_proof.py']:
        shutil.copyfile(Path(__file__).with_name(name), out / name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(qualification=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--inventory-catalog-sha256', required=True)
    parser.add_argument('--runtime-catalog-sha256', required=True)
    args = parser.parse_args()
    qualify(args.inventory, args.runtime, args.inventory_catalog_sha256, args.runtime_catalog_sha256)
