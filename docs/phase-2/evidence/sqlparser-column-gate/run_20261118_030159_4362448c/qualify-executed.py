"""Freeze the exact successful column run, preserving the full remaining core denominator."""
import argparse
import copy
import datetime
import json
import shutil
import uuid
from pathlib import Path
from urllib.parse import unquote, urlparse
from freeze import ROOT, DOC, SOURCE
from qualify import read, sha
from verify import need, parse_values, compare
from verify_column import edges

PARENT = ROOT/'docs/phase-2/evidence/sqlparser-core-values-gate/run_20261118_022222_bb4f2677'


def run(verification):
    result = read(verification/'result.json')
    oracle = Path(result['oracle']); runtime = read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json') == 'b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd', 'Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:
        need(sha(PARENT/row['path']) == row['sha256'], 'Parent frozen member drift')
    bindings = read(PARENT/'oracle/runtime-inputs.json')
    need(bindings == runtime['original_runtime_inputs'], 'Original runtime scope changed')
    for path, digest in bindings.items():
        need(sha(Path(path)) == digest, 'Original classpath drift')
    inventory = read(DOC/'inventory.json')
    for row in inventory['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    for row in inventory['cases']:
        need(sha(SOURCE/row['source']) == row['source_sha256'] == sha(ROOT/row['immutable_copy']), 'Original test source drift')
    for row in runtime['producer_classes']:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual producer class drift')
    for name in ['ColumnMetaOracle.java', 'CoreValueOracle.java']:
        need((oracle/name).read_bytes() == (ROOT/'tools/formal-sqlparser-core'/name).read_bytes(), 'Actual oracle source drift')
    name, origin = (oracle/'column-origin.tsv').read_text('utf-8').strip().split('\t')
    need(name == 'org.apache.seata.sqlparser.struct.ColumnMeta' and
         Path(unquote(urlparse(origin).path).lstrip('/')).resolve() == (SOURCE/'sqlparser/seata-sqlparser-core/target/classes').resolve(), 'ColumnMeta was shadowed')
    need(sha(ROOT/'Cargo.lock') == sha(PARENT/'inputs/Cargo.lock'), 'Dependency lock changed')
    unchanged = []
    for path in (PARENT/'inputs/crates').rglob('*'):
        relative = path.relative_to(PARENT/'inputs')
        if path.is_file() and relative.parts[1] not in {'seata_sqlparser', 'seata_test'}:
            need(sha(ROOT/relative) == sha(path), 'Pre-existing component changed')
            unchanged.append(relative.as_posix())
    expected = {row['id'] for row in read(PARENT/'verification/1.99-differential.json')}
    expected |= {row['id'] for row in inventory['cases'] if '.struct.ColumnMetaTest#' in row['id']}
    need(len(expected) == 41, 'Wrong cumulative original source case floor')
    java = parse_values(verification/'java-source-values.tsv', expected)
    edge_java = edges(oracle/'edges-java.tsv')
    expected_edges = {f'yes-ignore-case-{i}' for i in range(3)} | {f'utf16-{i}' for i in range(7)} | {
        'all-utf16-units', 'sequence-raw-initial', 'sequence-raw-mutated', 'exception-raw',
        'exception-raw-cause', 'exception-null-cause-text', 'column-default', 'column-raw-overflow',
        'column-callback-equals', 'column-callback-hash', 'column-callback-text'}
    need(set(edge_java) == expected_edges, 'Source edge identities differ')
    for version in ['1.99', '1.85']:
        need(compare(java, parse_values(verification/(version+'-source.tsv'), expected)) == read(verification/(version+'-source-diff.json')), 'Source differential derivative differs')
        need(compare(edge_java, edges(verification/(version+'-edges.tsv'))) == read(verification/(version+'-edge-diff.json')), 'Edge differential derivative differs')
        need(result['Rust'][version] == dict(ordinary=406, harness=85, ignored=143, doc=4, source_MATCH=41, edge_MATCH=21), 'Full regression floor differs')
    commands = read(verification/'commands.json')
    need(len(commands) == 20 and all(row['exit_code'] == (101 if row['log'].startswith('mutant-') else 0) for row in commands), 'Incomplete actual command closure')
    need(len(result['mutants']) == 5 and sum(row['actual_failed_tests'] for row in result['mutants']) == 8, 'Runtime mutants incomplete')
    inputs = read(verification/'inputs.json')
    for row in inputs:
        need(sha(ROOT/row['path']) == row['sha256'], 'Verified input drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-2/evidence/sqlparser-column-gate'/('run_'+stamp)
    out.mkdir(parents=True)
    for row in inputs:
        target = out/'inputs'/row['path']; target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT/row['path']).read_bytes())
        need(sha(target) == row['sha256'], 'Snapshot drift')
    for source, label in [(verification, 'verification'), (oracle, 'oracle')]:
        (out/label).mkdir()
        for path in source.iterdir():
            if path.is_file():
                shutil.copy2(path, out/label/path.name)
    (out/'producer-classes').mkdir()
    for row in runtime['producer_classes']:
        shutil.copy2(row['path'], out/'producer-classes'/Path(row['path']).name)
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    controls = []
    def reject(name, action):
        try:
            action()
        except (ValueError, KeyError):
            controls.append(name)
            return
        raise ValueError('Counterexample accepted: '+name)
    for label, baseline in [('source', java), ('edge', edge_java)]:
        for kind in ['missing', 'foreign', 'mismatch']:
            changed = copy.deepcopy(baseline)
            if kind == 'missing': changed.pop(next(iter(changed)))
            if kind == 'foreign': changed['foreign-key'] = ['true']
            if kind == 'mismatch': changed[next(iter(changed))] = ['wrong']
            reject(label+'-'+kind, lambda: compare(baseline, changed))
    source_duplicate = out/'duplicate-source-control.tsv'
    source_duplicate.write_bytes((verification/'java-source-values.tsv').read_bytes()*2)
    reject('duplicate-source', lambda: parse_values(source_duplicate, expected))
    edge_duplicate = out/'duplicate-edge-control.tsv'
    edge_duplicate.write_bytes((oracle/'edges-java.tsv').read_bytes()*2)
    reject('duplicate-edge', lambda: edges(edge_duplicate))
    (out/'comparison-counterexamples.json').write_text(json.dumps(dict(count=8, all_rejected=True, cases=controls), indent=2)+'\n', 'utf-8')
    for name, path in [
        ('utf16-structural-red.log', Path('D:/Temp/seata-core-utf16-structural-red.log')),
        ('column-structural-red.log', Path('D:/Temp/seata-column-meta-structural-red.log')),
        ('autoincrement-behavior-red.log', Path('D:/Temp/seata-column-autoincrement-behavior-red.log')),
        ('reentrant-behavior-red.log', Path('D:/Temp/seata-column-reentrant-behavior-red.log')),
        ('layout-audit.json', Path('D:/Temp/seata-column-layout-final.json'))]:
        shutil.copy2(path, out/name)
    layout = read(out/'layout-audit.json')
    need(layout['errors'] == layout['strict_migration_blockers'] == 19 and layout['warnings'] == 0, 'Unexpected layout gaps')
    published = result | dict(parent=str(PARENT), parent_catalog_sha256=sha(PARENT/'catalog.json'),
        object_states=dict(IMPLEMENTED=12, MISSING=19), input_files=len(inputs),
        unchanged_existing_component_files=len(unchanged), registry_dependency_closure_changed=False,
        original_runtime_inputs_bound=len(bindings), comparison_counterexamples=8,
        original_JUnit_counterexamples=13, strict_module_layout_blockers=19,
        reviewed_cohesive_file=dict(path='crates/seata_sqlparser/src/struct/column_meta.rs', lines=606,
            reason='One Java ColumnMeta and its private shared state; 20 field accessors plus explicit equals/hash/toString; no unrelated source objects'),
        closed_findings=['P2 Debug invokes user formatting under lock', 'P2 stale tail fields after reentrant Object callbacks',
                         'Java YES.equalsIgnoreCase includes U+017F; ASCII-only comparison rejected'],
        historical_attempts=[dict(path=str(oracle/'verification_20261118_025509_021380'), reason='Mutation anchor whitespace mismatch; no qualified mutant set'),
                             dict(path=str(oracle/'verification_20261118_025633_141857'), reason='Successful precursor before source provenance doc additions; final input set comes from later explicit run')])
    (out/'result.json').write_text(json.dumps(published, indent=2)+'\n', 'utf-8')
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path)) for path in sorted(out.rglob('*')) if path.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members), indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(root=str(out), catalog_sha256=sha(out/'catalog.json'), members=len(members), input_files=len(inputs), source_MATCH=41, edge_MATCH=21)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--verification', type=Path, required=True)
    run(parser.parse_args().verification.resolve())
