"""Freeze the successful IndexMeta run; identity context is input, never a result."""
import argparse
import copy
import datetime
import json
import re
import shutil
import sys
import uuid
from pathlib import Path
from urllib.parse import unquote, urlparse
from freeze import ROOT, DOC, SOURCE
from qualify import read, sha
from verify import need, parse_values, compare
from verify_index import table

PARENT = ROOT/'docs/phase-2/evidence/sqlparser-column-gate/run_20261118_030159_4362448c'


def run(verification):
    result = read(verification/'result.json')
    oracle = Path(result['oracle']); runtime = read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json') == '1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da', 'Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:
        need(sha(PARENT/row['path']) == row['sha256'], 'Frozen parent drift')
    bindings = read(PARENT/'oracle/oracle-result.json')['original_runtime_inputs']
    need(bindings == runtime['original_runtime_inputs'] and len(bindings) == 219, 'Original runtime scope differs')
    for path, digest in bindings.items():
        need(sha(Path(path)) == digest, 'Original classpath drift')
    inventory = read(DOC/'inventory.json')
    for row in inventory['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    for row in inventory['cases']:
        need(sha(SOURCE/row['source']) == row['source_sha256'] == sha(ROOT/row['immutable_copy']), 'Original test drift')
    for row in runtime['producer_classes']:
        need(sha(Path(row['path'])) == row['sha256'], 'Actual producer class drift')
    for name in ['CoreValueOracle.java', 'ColumnMetaOracle.java', 'IndexMetaOracle.java']:
        need((oracle/name).read_bytes() == (ROOT/'tools/formal-sqlparser-core'/name).read_bytes(), 'Actual producer source drift')
    name, origin = (oracle/'index-origin.tsv').read_text('utf-8').strip().split('\t')
    need(name == 'org.apache.seata.sqlparser.struct.IndexMeta' and
         Path(unquote(urlparse(origin).path).lstrip('/')).resolve() ==
         (SOURCE/'sqlparser/seata-sqlparser-core/target/classes').resolve(), 'IndexMeta shadowed')
    sys.path.insert(0, str(ROOT/'tools/formal-source-junit'))
    from validate import load, validate_events, controls
    events = load(oracle/'junit-events.jsonl')
    need(validate_events(events, 'seata-sqlparser-core') == runtime['original_JUnit_validation'], 'Original JUnit derivative differs')
    need(controls(events, 'seata-sqlparser-core') == runtime['original_JUnit_counterexamples'], 'JUnit negative controls differ')
    need(sha(ROOT/'Cargo.lock') == sha(PARENT/'inputs/Cargo.lock'), 'Dependency lock changed')
    unchanged = []
    for path in (PARENT/'inputs/crates').rglob('*'):
        relative = path.relative_to(PARENT/'inputs')
        if path.is_file() and relative.parts[1] not in {'seata_sqlparser', 'seata_test'}:
            need(sha(ROOT/relative) == sha(path), 'Existing component changed')
            unchanged.append(relative.as_posix())
    expected = {row['id'] for row in read(PARENT/'verification/1.99-source-diff.json')}
    expected |= {row['id'] for row in inventory['cases'] if '.struct.IndexMetaTest#' in row['id']}
    need(len(expected) == 57, 'Wrong source identity set')
    java = parse_values(verification/'source-java.tsv', expected)
    column = table(oracle/'column-edges-java.tsv', 21)
    all_index = table(oracle/'index-edges-java.tsv', 148)
    variants = ['PRIMARY', 'NORMAL', 'UNIQUE', 'FULL_TEXT']
    identity = {key: value for key, value in all_index.items() if key.startswith('enum-identity/')}
    need(set(identity) == {'enum-identity/'+name for name in variants}, 'Identity context differs')
    index = {key: value for key, value in all_index.items() if key not in identity}
    mutations = ['none', 'clear-left', 'clear-right', 'clear-both', 'set-left', 'set-right', 'null-left',
                 'null-right', 'remove-left', 'remove-right', 'grow-left-set', 'grow-right-set']
    edge_keys = {f'eq/{a}/{b}/{m}' for a in 'ADF' for b in 'ADF' for m in mutations}
    edge_keys |= {f'text/{a}/{m}' for a in 'ADF' for m in ['none', 'clear-left', 'remove-left', 'null-left', 'set-left', 'grow-left-set']}
    edge_keys |= {'failure/'+str(i) for i in range(7)} | {'index-hash/'+name for name in variants}
    edge_keys |= {'index-default', 'index-null-type', 'index-null-list-shortcut', 'index-ignored-cardinality',
                  'index-null-list-text', 'index-wide-min', 'index-wide-max'}
    need(set(index) == edge_keys and len(index) == 144, 'Computed edge identity set differs')
    for version in ['1.99', '1.85']:
        for label, baseline, observed in [('source', java, parse_values(verification/(version+'-source.tsv'), expected)),
            ('column', column, table(verification/(version+'-column.tsv'), 21)),
            ('index', index, table(verification/(version+'-index.tsv'), 144))]:
            need(compare(baseline, observed) == read(verification/(version+'-'+label+'-diff.json')), 'Differential derivative differs')
        need(table(verification/(version+'-identity-inputs.tsv'), 4) == identity, 'Identity input binding differs')
        log = (verification/(version+'-tests.log')).read_text('utf-8')
        totals = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', log)]
        need(len(totals) == 89 and sum(r[0] for r in totals) == 434 and sum(r[1] for r in totals) == 0 and sum(r[2] for r in totals) == 143, 'Regression floor differs')
        need(sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', (verification/(version+'-doc.log')).read_text('utf-8')))) == 4, 'Doc floor differs')
    commands = read(verification/'commands.json')
    need(len(commands) == 24 and all(row['exit_code'] == (101 if row['log'].startswith('mutant-') else 0) for row in commands), 'Command closure incomplete')
    need(len(result['mutants']) == 9, 'Mutants incomplete')
    for row in result['mutants']:
        log = (verification/('mutant-'+row['name']+'.log')).read_text('utf-8')
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log, 'Mutation was not behavioral RED')
        need(sha(Path(row['path'])) == row['mutant_sha256'], 'Mutant source drift')
    inputs = read(verification/'inputs.json')
    for row in inputs:
        need(sha(ROOT/row['path']) == row['sha256'], 'Verified input drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-2/evidence/sqlparser-index-gate'/('run_'+stamp); out.mkdir(parents=True)
    for row in inputs:
        target = out/'inputs'/row['path']; target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT/row['path']).read_bytes()); need(sha(target) == row['sha256'], 'Snapshot drift')
    for source, label in [(verification, 'verification'), (oracle, 'oracle')]:
        (out/label).mkdir()
        for path in source.iterdir():
            if path.is_file(): shutil.copy2(path, out/label/path.name)
    (out/'producer-classes').mkdir(); (out/'mutant-sources').mkdir()
    for row in runtime['producer_classes']:
        shutil.copy2(row['path'], out/'producer-classes'/Path(row['path']).name)
    for row in result['mutants']:
        shutil.copy2(row['path'], out/'mutant-sources'/(row['name']+'.rs'))
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    rejected = []
    def reject(label, action):
        try: action()
        except (ValueError, KeyError): rejected.append(label); return
        raise ValueError('Counterexample accepted: '+label)
    for label, baseline in [('source', java), ('index', index)]:
        for kind in ['missing', 'foreign', 'mismatch']:
            changed = copy.deepcopy(baseline)
            if kind == 'missing': changed.pop(next(iter(changed)))
            if kind == 'foreign': changed['foreign-key'] = ['true']
            if kind == 'mismatch': changed[next(iter(changed))] = ['wrong']
            reject(label+'-'+kind, lambda: compare(baseline, changed))
    duplicate = out/'duplicate-source-control.tsv'; duplicate.write_bytes((verification/'source-java.tsv').read_bytes()*2)
    reject('duplicate-source', lambda: parse_values(duplicate, expected))
    duplicate = out/'duplicate-index-control.tsv'; duplicate.write_bytes((oracle/'index-edges-java.tsv').read_bytes()*2)
    reject('duplicate-index', lambda: table(duplicate, 148))
    (out/'comparison-counterexamples.json').write_text(json.dumps(dict(count=8, all_rejected=True, cases=rejected), indent=2)+'\n', 'utf-8')
    for name in ['index-meta', 'index-obligations', 'object-callback-errors']:
        shutil.copy2('D:/Temp/seata-'+name+'-structural-red.log', out/(name+'-structural-red.log'))
    shutil.copy2('D:/Temp/seata-index-layout.json', out/'layout-audit.json')
    layout = read(out/'layout-audit.json')
    need(layout['errors'] == layout['strict_migration_blockers'] == 18 and layout['warnings'] == 0, 'Unexpected layout gaps')
    published = result | dict(parent=str(PARENT), parent_catalog_sha256=sha(PARENT/'catalog.json'),
        input_files=len(inputs), unchanged_existing_component_files=len(unchanged), registry_dependency_closure_changed=False,
        original_runtime_inputs_bound=len(bindings), comparison_counterexamples=8, original_test_sources=12,
        source_fixture_assets=0, original_source_changes=False, strict_module_layout_blockers=18,
        reviewed_cohesive_file=dict(path='crates/seata_sqlparser/src/struct/column_meta.rs', lines=608,
            reason='One Java ColumnMeta and private shared state; no unrelated source objects'),
        limitations=['143 environment-dependent tests ignored; no new DB qualification',
                    '18 source objects and19 original TableMeta cases remain missing',
                    'Java enum identity hashes require explicit same-run input context',
                    'Early Rust callback extension APIs now return Result; callers adapted'])
    (out/'result.json').write_text(json.dumps(published, indent=2)+'\n', 'utf-8')
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members), indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(root=str(out), catalog_sha256=sha(out/'catalog.json'), members=len(members), input_files=len(inputs))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--verification', type=Path, required=True)
    run(parser.parse_args().verification.resolve())
