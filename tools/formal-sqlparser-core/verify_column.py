"""Execute dual-toolchain full regressions and the cumulative41/edge21 differential gate."""
import argparse
import datetime
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need, parse_values, compare


def edges(path):
    rows = {}
    for line in path.read_text('utf-8').splitlines():
        key, *values = line.split('\t')
        need(key not in rows, 'Duplicate edge identity')
        rows[key] = values
    need(len(rows) == 21, 'Original edge observation floor changed')
    return rows


def run(oracle):
    receipt = read(oracle/'oracle-result.json')
    need(receipt['original_JUnit_validation']['passed'] == 76, 'Original core JUnit is not qualified')
    out = oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f'))
    out.mkdir()
    inventory = read(DOC/'inventory.json')
    foundation = (oracle/'foundation-java.tsv').read_text('utf-8')
    column = (oracle/'column-java.tsv').read_text('utf-8')
    (out/'java-source-values.tsv').write_text(foundation+column, 'utf-8')
    expected = [line.split('\t')[0] for line in (foundation+column).splitlines()]
    need(len(expected) == len(set(expected)) == 41 and
         set(expected).issubset({row['id'] for row in inventory['cases']}), 'Wrong complete source case identities')
    java = parse_values(out/'java-source-values.tsv', expected)
    edge_java = edges(oracle/'edges-java.tsv')
    need((ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/column-edges-java-v2.tsv').read_bytes() == (oracle/'edges-java.tsv').read_bytes(), 'Derived Java oracle fixture drift')
    inputs = []
    for directory in ['crates', 'tools/formal-sqlparser-core', 'docs/phase-2/sqlparser_core']:
        for path in sorted((ROOT/directory).rglob('*')):
            if path.is_file():
                inputs.append(dict(path=path.relative_to(ROOT).as_posix(), sha256=sha(path)))
    for name in ['Cargo.toml', 'Cargo.lock']:
        inputs.append(dict(path=name, sha256=sha(ROOT/name)))
    commands = []
    base = os.environ | dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home', CARGO_INCREMENTAL='0',
                             TEMP='D:/Temp', TMP='D:/Temp')
    for name in ['SEATA_CORE_OBSERVATIONS', 'SEATA_COLUMN_EDGE_OBSERVATIONS']:
        base.pop(name, None)

    def invoke(argv, name, env, cwd=ROOT, expected_failure=False):
        with (out/name).open('wb') as log:
            code = subprocess.run(list(map(str, argv)), cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT,
                timeout=600, creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str, argv)), cwd=str(cwd), exit_code=code, log=name,
            CARGO_HOME=env['CARGO_HOME'], CARGO_TARGET_DIR=env['CARGO_TARGET_DIR'],
            RUSTC=env.get('RUSTC'), RUSTUP_TOOLCHAIN=env.get('RUSTUP_TOOLCHAIN'),
            source_observations=env.get('SEATA_CORE_OBSERVATIONS'), edge_observations=env.get('SEATA_COLUMN_EDGE_OBSERVATIONS')))
        (out/'commands.json').write_text(json.dumps(commands, indent=2)+'\n', 'utf-8')
        need(code == (101 if expected_failure else 0), 'Unexpected command exit: '+name)
        return (out/name).read_text('utf-8')

    results = {}
    for version in ['1.99', '1.85']:
        env = base | dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production'+('-msrv' if version == '1.85' else ''))
        cargo = 'cargo'
        if version == '1.85':
            binaries = Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin')
            env.update(RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc', RUSTC=str(binaries/'rustc.exe'),
                       RUSTDOC=str(binaries/'rustdoc.exe'), PATH=str(binaries)+os.pathsep+env['PATH'])
            cargo = binaries/'cargo.exe'
        need('cargo '+version+'.' in invoke([cargo, '--version'], version+'-cargo.log', env), 'Actual toolchain differs')
        invoke([env.get('RUSTC', 'rustc'), '--version'], version+'-rustc.log', env)
        if version == '1.99':
            invoke([cargo, 'fmt', '--all', '--', '--check'], version+'-fmt.log', env)
        for kind, feature in [('default', []), ('all', ['--all-features'])]:
            invoke([cargo, 'check', '--offline', '--locked', '--workspace', '--all-targets']+feature, version+'-check-'+kind+'.log', env)
        invoke([cargo, 'clippy', '--offline', '--locked', '--workspace', '--all-features', '--all-targets', '--', '-D', 'warnings'], version+'-clippy.log', env)
        source = out/(version+'-source.tsv'); edge = out/(version+'-edges.tsv')
        env.update(SEATA_CORE_OBSERVATIONS=str(source), SEATA_COLUMN_EDGE_OBSERVATIONS=str(edge))
        log = invoke([cargo, 'test', '--offline', '--locked', '--workspace', '--all-features', '--all-targets'], version+'-tests.log', env)
        totals = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', log)]
        need(sum(row[0] for row in totals) == 406 and len(totals) == 85 and sum(row[2] for row in totals) == 143,
             'Unexpected full ordinary regression/ignored/harness floor')
        env.pop('SEATA_CORE_OBSERVATIONS'); env.pop('SEATA_COLUMN_EDGE_OBSERVATIONS')
        doc = invoke([cargo, 'test', '--offline', '--locked', '--workspace', '--all-features', '--doc'], version+'-doc.log', env)
        need(sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', doc))) == 4, 'Doctest floor differs')
        differential = compare(java, parse_values(source, expected))
        edge_diff = compare(edge_java, edges(edge))
        for name, value in [('source-diff', differential), ('edge-diff', edge_diff)]:
            (out/(version+'-'+name+'.json')).write_text(json.dumps(value, indent=2)+'\n', 'utf-8')
        results[version] = dict(ordinary=406, harness=85, ignored=143, doc=4, source_MATCH=41, edge_MATCH=21)
        print(json.dumps(dict(version=version, **results[version])), flush=True)

    # Real mutant copies; never edit the shared live worktree or delete user data.
    work = Path(receipt['work'])/'mutants'/out.name
    need(work.resolve().is_relative_to(Path('D:/codex-builds').resolve()), 'Mutant workspace escaped D root')
    variants = [
        ('stale-snapshot-and-debug-lock', 'struct/column_meta.rs', None, None, 'column_meta_obligations', None),
        ('ascii-only-autoincrement', 'struct/column_meta.rs',
         'matches!(\n                text.as_utf16(),\n                [0x59 | 0x79, 0x45 | 0x65, 0x53 | 0x73 | 0x17f]\n            )',
         'text.as_str().is_some_and(|value| value.eq_ignore_ascii_case("YES"))',
         'column_meta_obligations', 'autoincrement_uses_java_unicode_ignore_case_not_ascii_only'),
        ('lossy-surrogate-storage', 'java_string.rs', 'let utf8 = String::from_utf16(&units).ok().map(Arc::from);',
         'let units: Vec<u16> = String::from_utf16_lossy(&units).encode_utf16().collect();\n        let utf8 = String::from_utf16(&units).ok().map(Arc::from);',
         'core_utf16', 'sequence_raw_getters_and_mutations_retain_unpaired_surrogates'),
        ('ignore-case-sensitive-hash', 'struct/column_meta.rs',
         'hash = hash.wrapping_add(if state.is_case_sensitive { 1231 } else { 1237 });',
         'hash = hash.wrapping_add(0);', 'column_meta_obligations', 'java_hash_wraps_and_text_retains_raw_utf16'),
        ('erase-boxed-number-type', 'java_object.rs', 'other.as_any().downcast_ref::<Self>() == Some(self)',
         'self.java_to_string() == other.java_to_string()', 'column_meta_obligations', 'object_field_preserves_boxed_types_and_null'),
    ]
    mutants = []
    for name, target, old, new, suite, test in variants:
        directory = work/name
        directory.mkdir(parents=True)
        for crate in ['seata_sqlparser', 'seata_test']:
            shutil.copytree(ROOT/'crates'/crate, directory/'crates'/crate)
        (directory/'Cargo.toml').write_text('[workspace]\nmembers=["crates/seata_sqlparser","crates/seata_test"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\nrust-version="1.85"\nlicense="Apache-2.0"\nrepository="https://github.com/easy-4-rust/seata-rust"\n[workspace.lints.rust]\nunsafe_code="forbid"\n[workspace.lints.clippy]\nunwrap_used="warn"\nexpect_used="warn"\n', 'utf-8')
        path = directory/'crates/seata_sqlparser/src'/target
        before = sha(path)
        if old is None:
            path.write_bytes((ROOT/'docs/phase-2/evidence/sqlparser-column/run_20261118_024251_e95f0720/column-meta-before-reentrant-fix.rs').read_bytes())
        else:
            text = path.read_text('utf-8')
            need(text.count(old) == (2 if name == 'erase-boxed-number-type' else 1), 'Mutant anchor changed: '+name)
            path.write_text(text.replace(old, new), 'utf-8')
        env = base | dict(CARGO_TARGET_DIR='D:/cargo-target/seata-column-mutants')
        args = ['cargo', 'test', '--offline', '-p', 'seata-test', '--test', suite]
        if test:
            args += [test, '--', '--exact']
        log = invoke(args, 'mutant-'+name+'.log', env, directory, expected_failure=True)
        failed = 4 if old is None else 1
        need(f'; {failed} failed;' in log and 'test result: FAILED.' in log and 'error[E' not in log and 'panicked at' in log,
             'Mutant must fail through actual target behavior assertions')
        mutants.append(dict(name=name, path=str(path), before_sha256=before, mutant_sha256=sha(path),
                            actual_failed_tests=failed, result='RUNTIME_REJECTED'))
    for row in inputs:
        need(sha(ROOT/row['path']) == row['sha256'], 'Input changed during actual verification')
    for path, digest in receipt['original_runtime_inputs'].items():
        need(sha(Path(path)) == digest, 'Original source runtime drift')
    (out/'inputs.json').write_text(json.dumps(inputs, indent=2)+'\n', 'utf-8')
    result = dict(status='COLUMN_METADATA_AND_LOSSLESS_UTF16_VERIFIED_CORE_OPEN', oracle=str(oracle), verification=str(out),
        original_JUnit_passed=76, source_cases_MATCH=41, source_cases_MISSING=35, source_objects=31,
        source_objects_present=12, source_objects_MISSING=19, original_edge_observations_MATCH=21,
        Rust=results, mutants=mutants,
        source_fixture_assets=0, original_test_sources=12, original_source_changes=False,
        full_core_complete=False, WP05_complete=False, WP06_complete=False, full_V1_complete=False,
        TC_safety_gate_passed=False, external_TC_risk_accepted=False, shared_lock_issue='deferred_by_user',
        limitations=['IndexMeta/TableMeta/SqlServer metadata and remaining SPI/recognizer objects not yet migrated',
                     'No new DB qualification; old seata_at SQL support matrix unchanged',
                     'UTF8 convenience views are optional; Java counterparts use explicit lossless *_java accessors'])
    (out/'result.json').write_text(json.dumps(result, indent=2)+'\n', 'utf-8')
    print(json.dumps(dict(verification=str(out), source_MATCH=41, edge_MATCH=21, runtime_mutants=len(mutants), full_core_complete=False)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--oracle', type=Path, required=True)
    run(parser.parse_args().oracle.resolve())
