"""Fresh dual-toolchain regression, explicit per-case differential and runtime mutants."""
import argparse
import copy
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from freeze import ROOT, DOC, SOURCE, JAVA_PIN, RUST_PIN

HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(value, why):
    if not value:
        raise ValueError(why)


def parse_values(path, expected):
    rows = {}
    for line in path.read_text('utf-8').splitlines():
        key, value = line.split('\t')
        need(key not in rows, 'Duplicate source case')
        rows[key] = bytes.fromhex(value).decode('utf-8').split('\u001f')
    need(set(rows) == set(expected), 'Missing or foreign concrete source case')
    return rows


def compare(java, rust):
    need(set(java) == set(rust), 'Missing or foreign Rust case')
    need(all(java[key] == rust[key] for key in java), 'Observable Java/Rust mismatch')
    return [dict(id=key, result='MATCH', Java=java[key], Rust=rust[key]) for key in sorted(java)]


def run(oracle):
    oracle = oracle.resolve()
    receipt = json.loads((oracle/'oracle-result.json').read_bytes())
    need(receipt['java_pin'] == JAVA_PIN and receipt['original_tests_passed'] == 76, 'Wrong source oracle')
    inventory = json.loads((DOC/'inventory.json').read_bytes())
    classes = {'EscapeSymbolTest', 'SQLTypeTest', 'SQLParsingExceptionTest',
               'struct.IndexTypeTest', 'struct.SqlSequenceExprTest', 'struct.SqlMethodExprTest',
               'struct.SqlDefaultExprTest', 'struct.NullTest', 'struct.NotPlaceHolderExprTest'}
    expected = [row['id'] for row in inventory['cases']
                if row['id'].split('org.apache.seata.sqlparser.')[1].split('#')[0] in classes]
    need(len(expected) == len(set(expected)) == 23, 'Original foundation case floor changed')
    java = parse_values(oracle/'java-values.tsv', expected)
    sys.path.insert(0, str(ROOT/'tools/formal-source-junit'))
    from validate import validate_events, load
    original = validate_events(load(oracle/'junit-events.jsonl'), 'seata-sqlparser-core')
    need({row['declaring_class']+'#'+row['method'] for row in original['cases']} ==
         {row['id'] for row in inventory['cases']}, 'Inventory does not equal original concrete case set')
    source_oracle = oracle
    oracle = source_oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f'))
    oracle.mkdir()
    inputs = []
    for family in ['crates', 'tools/formal-sqlparser-core', 'docs/phase-2/sqlparser_core']:
        for path in sorted((ROOT/family).rglob('*')):
            if path.is_file():
                inputs.append(dict(path=path.relative_to(ROOT).as_posix(), sha256=sha(path)))
    for name in ['Cargo.toml', 'Cargo.lock']:
        inputs.append(dict(path=name, sha256=sha(ROOT/name)))
    commands = []
    base_env = os.environ | dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',
        CARGO_INCREMENTAL='0', TEMP='D:/Temp', TMP='D:/Temp', PYTHONUTF8='1', PYTHONDONTWRITEBYTECODE='1')
    base_env.pop('SEATA_CORE_OBSERVATIONS', None)

    def invoke(args, name, env, cwd=ROOT, failure=False):
        start = datetime.datetime.now().isoformat()
        with (oracle/name).open('wb') as log:
            code = subprocess.run(list(map(str, args)), cwd=cwd, env=env, stdout=log,
                stderr=subprocess.STDOUT, timeout=600, creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str, args)), cwd=str(cwd), exit_code=code,
            log=name, started=start, ended=datetime.datetime.now().isoformat(),
            CARGO_HOME=env['CARGO_HOME'], CARGO_TARGET_DIR=env['CARGO_TARGET_DIR'],
            RUSTC=env.get('RUSTC'), RUSTUP_TOOLCHAIN=env.get('RUSTUP_TOOLCHAIN'),
            observations=env.get('SEATA_CORE_OBSERVATIONS')))
        (oracle/'rust-commands.json').write_text(json.dumps(commands, indent=2)+'\n', 'utf-8')
        need(code != 0 if failure else code == 0, 'Unexpected command result: '+name)
        return (oracle/name).read_text('utf-8')

    results = {}
    for label in ['1.99', '1.85']:
        env = base_env.copy()
        env['CARGO_TARGET_DIR'] = 'D:/cargo-target/seata-rust-production'+('-msrv' if label == '1.85' else '')
        if label == '1.85':
            bin_dir = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
            env.update(RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc', RUSTC=str(bin_dir/'rustc.exe'),
                       RUSTDOC=str(bin_dir/'rustdoc.exe'), PATH=str(bin_dir)+os.pathsep+env['PATH'])
            cargo = bin_dir/'cargo.exe'
        else:
            cargo = 'cargo'
        version = invoke([cargo, '--version'], label+'-cargo-version.log', env)
        need(('cargo '+label+'.') in version, 'Unexpected actual Cargo version')
        invoke([env.get('RUSTC', 'rustc'), '--version'], label+'-rustc-version.log', env)
        if label == '1.99':
            invoke([cargo, 'fmt', '--all', '--', '--check'], label+'-fmt.log', env)
        for kind, feature in [('check-default', []), ('check-all-features', ['--all-features'])]:
            invoke([cargo, 'check', '--offline', '--locked', '--workspace', '--all-targets']+feature,
                   label+'-'+kind+'.log', env)
        invoke([cargo, 'clippy', '--offline', '--locked', '--workspace', '--all-features', '--all-targets', '--', '-D', 'warnings'], label+'-clippy.log', env)
        values = oracle/(label+'-rust-values.tsv')
        need(not values.exists(), 'Observation file already exists')
        env['SEATA_CORE_OBSERVATIONS'] = str(values)
        log = invoke([cargo, 'test', '--offline', '--locked', '--workspace', '--all-features', '--all-targets'], label+'-workspace-tests.log', env)
        env.pop('SEATA_CORE_OBSERVATIONS')
        totals = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', log)]
        need(sum(row[0] for row in totals) == 369 and all(row[1] == 0 for row in totals), 'Full ordinary regression floor differs')
        doc = invoke([cargo, 'test', '--offline', '--locked', '--workspace', '--all-features', '--doc'], label+'-doc-tests.log', env)
        doc_totals = [int(value) for value in re.findall(r'test result: ok\. (\d+) passed;', doc)]
        need(sum(doc_totals) == 4, 'Original doctest floor differs')
        rust = parse_values(values, expected)
        differential = compare(java, rust)
        (oracle/(label+'-differential.json')).write_text(json.dumps(differential, indent=2)+'\n', 'utf-8')
        results[label] = dict(ordinary_passed=sum(row[0] for row in totals), harness_results=len(totals),
                              ignored=sum(row[2] for row in totals), doctests_passed=sum(doc_totals),
                              source_cases_MATCH=len(differential))
        print(json.dumps(dict(toolchain=label, **results[label])), flush=True)

    # Mutants are independent copies below our owned D root; live source is never edited.
    work = Path(receipt['work'])
    need(work.resolve().is_relative_to(Path('D:/codex-builds').resolve()), 'Mutation workspace escaped D root')
    variants = [
        ('wrong-select-mapping', 'sql_type.rs', '0 => Ok(Self::SELECT)', '0 => Ok(Self::INSERT)', 'core_values', 'sql_type_value_of'),
        ('accept-enum-hole', 'sql_type.rs', '_ => Err(InvalidEnumValue::new("SQLType", i)),', '100 => Ok(Self::SELECT),\n            _ => Err(InvalidEnumValue::new("SQLType", i)),', 'core_values', 'sql_type_invalid'),
        ('clamp-negative-count', 'struct/sql_method_expr.rs', 'Self { placeholder_count }', 'Self { placeholder_count: if placeholder_count < 0 { 0 } else { placeholder_count } }', 'core_value_obligations', 'method_count_is_signed_and_participates_in_equality'),
        ('lose-cause-message', 'sql_parsing_exception.rs', 'let message = cause.as_ref().map(ToString::to_string);', 'let message = Some(String::new());', 'core_values', 'exception_cause'),
        ('truncate-utf16-char', 'escape_symbol.rs', 'Self::paired(single_symbol, single_symbol)', 'Self::paired((single_symbol as u8) as u16, (single_symbol as u8) as u16)', 'core_value_obligations', 'escape_preserves_utf16_code_units'),
        ('wrong-jdbc-constant', 'util/jdbc_constants.rs', '"polardb-x"', '"polardbx"', 'core_constants_diff', 'complete_constant_inventory_matches_original_java'),
    ]
    mutants = []
    env = base_env | dict(CARGO_TARGET_DIR='D:/cargo-target/seata-core-value-mutants')
    for name, target, old, new, suite, test in variants:
        directory = work/'mutants'/oracle.name/name
        directory.mkdir(parents=True)
        for crate in ['seata_sqlparser', 'seata_test']:
            shutil.copytree(ROOT/'crates'/crate, directory/'crates'/crate)
        (directory/'Cargo.toml').write_text('[workspace]\nmembers=["crates/seata_sqlparser","crates/seata_test"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\nrust-version="1.85"\nlicense="Apache-2.0"\nrepository="https://github.com/easy-4-rust/seata-rust"\n[workspace.lints.rust]\nunsafe_code="forbid"\n[workspace.lints.clippy]\nunwrap_used="warn"\nexpect_used="warn"\n', 'utf-8')
        path = directory/'crates/seata_sqlparser/src'/target
        text = path.read_text('utf-8')
        need(text.count(old) == 1, 'Mutation anchor differs: '+name)
        before = sha(path)
        path.write_text(text.replace(old, new), 'utf-8')
        log = invoke(['cargo', 'test', '--offline', '-p', 'seata-test', '--test', suite, test, '--', '--exact'], 'mutant-'+name+'.log', env, directory, failure=True)
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log and 'panicked at' in log, 'Mutation did not fail through target behavior assertion')
        mutants.append(dict(name=name, source=str(path), before_sha256=before, mutant_sha256=sha(path),
                            suite=suite, test=test, result='RUNTIME_ASSERTION_REJECTED'))
    controls = []
    def rejected(name, action):
        try:
            action()
        except (ValueError, KeyError):
            controls.append(name)
            return
        raise ValueError('Differential counterexample accepted: '+name)
    wrong = copy.deepcopy(java)
    wrong.pop(next(iter(wrong)))
    rejected('missing-case', lambda: compare(java, wrong))
    wrong = copy.deepcopy(java)
    wrong['foreign#case'] = ['true']
    rejected('foreign-case', lambda: compare(java, wrong))
    wrong = copy.deepcopy(java)
    wrong[next(iter(wrong))] = ['wrong-observation']
    rejected('forged-green-over-value-mismatch', lambda: compare(java, wrong))
    duplicate = oracle/'duplicate-observation-control.tsv'
    duplicate.write_bytes((source_oracle/'java-values.tsv').read_bytes()*2)
    rejected('duplicate-case-observation', lambda: parse_values(duplicate, expected))
    for row in inputs:
        need(sha(ROOT/row['path']) == row['sha256'], 'Actual Rust/source input changed during verification')
    for row in inventory['objects']:
        need(sha(SOURCE/row['java']) == row['sha256'], 'Original source changed')
    (oracle/'verification-inputs.json').write_text(json.dumps(inputs, indent=2)+'\n', 'utf-8')
    result = dict(status='FOUNDATION_VALUES_VERIFIED_MODULE_MIGRATION_OPEN', java_pin=JAVA_PIN,
        source_oracle=str(source_oracle), verification=str(oracle),
        rust_pin=RUST_PIN, original_core_JUnit_passed=76, core_object_denominator=31,
        core_source_case_denominator=76, production_objects_added=11, core_source_cases_MATCH=23,
        core_source_cases_MISSING=53, original_constant_records_MATCH=88, Rust=results,
        runtime_mutants=mutants, differential_counterexamples=controls,
        full_core_migration_complete=False, WP05_complete=False, WP06_complete=False,
        full_V1_complete=False, external_TC_risk_accepted=False, shared_lock_issue='deferred_by_user',
        limitations=['Java String lone surrogate input domain is not representable by Rust UTF-8 String',
                     'No DB qualification rerun: existing database paths unchanged; host evidence remains inherited',
                     'Remaining 20 core objects and 53 source cases are not implemented',
                     'Initial RED is missing API E0432; runtime assertion RED comes from isolated mutants'])
    (oracle/'verification-result.json').write_text(json.dumps(result, indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(status=result['status'], MATCH=23, mutants=len(mutants), counterexamples=len(controls))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--oracle', type=Path, required=True)
    run(parser.parse_args().oracle)
