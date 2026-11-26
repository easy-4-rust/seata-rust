"""Freeze only the successful explicit run and bind it to the authenticated Java parent."""
import argparse
import datetime
import hashlib
import json
import shutil
import tomllib
import uuid
from pathlib import Path
from urllib.parse import unquote, urlparse
from freeze import ROOT, DOC, SOURCE, FIRST
from verify import need, parse_values, compare

HERE = Path(__file__).resolve().parent
PARENT = ROOT/'docs/phase-2/evidence/source-junit-gate/run_20261118_013909_5a5c9888'
PARENT_SHA = '373012b628b4e74b683c6c2160cca550fdde19b4e48d8d914db8dfba3d37d5a9'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def run(verification):
    verification = verification.resolve()
    result = read(verification/'verification-result.json')
    oracle = Path(result['source_oracle'])
    need(result['core_source_cases_MATCH'] == 23 and not result['full_core_migration_complete'], 'Wrong partial scope')
    need(sha(PARENT/'catalog.json') == PARENT_SHA, 'Wrong authenticated parent catalog')
    catalog = read(PARENT/'catalog.json')
    for member in catalog['members']:
        path = (PARENT/member['path']).resolve()
        need(path.is_relative_to(PARENT.resolve()) and sha(path) == member['sha256'], 'Parent evidence drift')
    inventory = read(DOC/'inventory.json')
    sources = read(PARENT/'source-inputs.json')
    existing_rust = []
    for row in sources:
        path = Path(row['path'])
        if path.is_relative_to(ROOT/'crates') and path.suffix in {'.rs', '.toml'}:
            need(sha(path) == row['sha256'], 'Pre-existing Rust implementation changed')
            existing_rust.append(row)
    old_lock = next(row for row in sources if row['path'] == str(ROOT/'Cargo.lock'))
    before = tomllib.loads((PARENT/old_lock['snapshot']).read_text('utf-8'))['package']
    after = tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package']
    def registry(packages):
        return sorted((p['name'], p['version'], p.get('source'), p.get('checksum'))
                      for p in packages if 'source' in p)
    need(registry(before) == registry(after), 'Unexpected registry dependency change')
    prior_runtime = ROOT/'docs/phase-2/evidence/source-junit-literal/runtime_20261118_013107_9651fdfd'
    bindings = {row['path']: row['sha256'] for row in read(prior_runtime/'preexisting-runtime-inputs.json')}
    bindings.update({row['path']: row['sha256'] for row in read(prior_runtime/'helper-artifacts.json')})
    runtime_inputs = read(oracle/'runtime-inputs.json')
    for path, digest in runtime_inputs.items():
        need(bindings.get(path) == digest == sha(Path(path)), 'Original classpath differs from authenticated parent')
    expected_types = {row['java_type'] for row in inventory['objects'] if row['java_type'].split('.')[-1] in FIRST}
    origins = {}
    for line in (oracle/'original-class-origins.tsv').read_text('utf-8').splitlines():
        name, uri = line.split('\t')
        need(name not in origins, 'Duplicate original class origin')
        origins[name] = Path(unquote(urlparse(uri).path).lstrip('/')).resolve()
    core_classes = SOURCE/'sqlparser/seata-sqlparser-core/target/classes'
    need(set(origins) == expected_types and all(path == core_classes.resolve() for path in origins.values()),
         'Original value classes were shadowed or loaded from another artifact')
    oracle_result = read(oracle/'oracle-result.json')
    need(sha(oracle/'oracle-class.class') == sha(Path(oracle_result['work'])/'classes/CoreValueOracle.class'),
         'Observation helper bytecode differs')
    commands = read(verification/'rust-commands.json')
    need(len(commands) == 21 and len(result['runtime_mutants']) == 6, 'Incomplete actual command closure')
    need(all(command['exit_code'] == (101 if command['log'].startswith('mutant-') else 0)
             for command in commands), 'Command result differs')
    for toolchain in ['1.99', '1.85']:
        need(result['Rust'][toolchain] == dict(ordinary_passed=369, harness_results=81,
             ignored=143, doctests_passed=4, source_cases_MATCH=23), 'Regression floor differs')
        observed = read(verification/(toolchain+'-differential.json'))
        expected = [row['id'] for row in observed]
        need(len(expected) == len(set(expected)) == 23, 'Wrong actual differential cases')
        fresh = compare(parse_values(oracle/'java-values.tsv', expected),
                        parse_values(verification/(toolchain+'-rust-values.tsv'), expected))
        need(fresh == observed, 'Differential derivative differs from actual raw outputs')
    for row in inventory['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source drift')
    for row in inventory['cases']:
        need(sha(SOURCE/row['source']) == row['source_sha256'] == sha(ROOT/row['immutable_copy']), 'Original test drift')
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-2/evidence/sqlparser-core-values-gate'/('run_'+stamp)
    out.mkdir(parents=True)
    inputs = read(verification/'verification-inputs.json')
    for row in inputs:
        source = ROOT/row['path']
        need(sha(source) == row['sha256'], 'Verified input drift before freeze')
        target = out/'inputs'/row['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        need(sha(target) == row['sha256'], 'Input snapshot differs')
    # Keep raw oracle and successful run separately; failed attempts remain explicit historical diagnostics.
    (out/'verification').mkdir(exist_ok=True)
    for path in verification.iterdir():
        if path.is_file():
            shutil.copy2(path, out/'verification'/path.name)
    (out/'oracle').mkdir()
    for name in ['CoreValueOracle.java', 'oracle-class.class', 'commands.json', 'runtime-inputs.json',
                 'junit-events.jsonl', 'original-junit.log', 'junit-validation.json', 'junit-counterexamples.json',
                 'oracle-result.json', 'java-values.tsv', 'java-constants.tsv', 'original-class-origins.tsv',
                 'original-oracle.log', 'oracle-compile.log']:
        shutil.copy2(oracle/name, out/'oracle'/name)
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    for label, path in [('initial-structural-red.log', Path('D:/Temp/seata-core-values-structural-red.log')),
                        ('initial-obligation-structural-red.log', Path('D:/Temp/seata-core-values-obligation-red.log')),
                        ('layout-audit.json', Path('D:/Temp/seata-core-layout-final.json'))]:
        shutil.copy2(path, out/label)
    layout = read(out/'layout-audit.json')
    need(layout['errors'] == layout['strict_migration_blockers'] == 20 and layout['warnings'] == 0,
         'Unexpected layout gaps or warnings')
    current = result | dict(parent_catalog_sha256=PARENT_SHA, parent=str(PARENT),
        unchanged_preexisting_Rust_files=len(existing_rust), registry_packages=len(registry(after)),
        registry_dependency_closure_changed=False, original_runtime_inputs_bound=len(runtime_inputs),
        object_states=dict(IMPLEMENTED=9, PARTIAL=2, MISSING=20),
        partial_objects=['SQLParsingException', 'SqlSequenceExpr'],
        original_JUnit_negative_controls=13, module_layout_completion_blocked=True,
        strict_module_layout_blockers=20,
        historical_attempts=[dict(path=str(oracle/'rust-commands.json'), reason='Rust1.85 Clippy format_collect in observation helper; fixed'),
            dict(path=str(oracle/'verification_20261118_021741_689388'), reason='Runtime mutant correctly failed via Result::Err; initial qualifier required assertion panic. Invalid-enum mirror now has explicit assertion; fresh full run used')])
    (out/'result.json').write_text(json.dumps(current, indent=2)+'\n', 'utf-8')
    members = [dict(path=path.relative_to(out).as_posix(), sha256=sha(path))
               for path in sorted(out.rglob('*')) if path.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members), indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(root=str(out), catalog_sha256=sha(out/'catalog.json'),
                         members=len(members), registry_packages=current['registry_packages'],
                         unchanged_Rust_files=len(existing_rust), MATCH=23)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--verification', type=Path, required=True)
    run(parser.parse_args().verification)
