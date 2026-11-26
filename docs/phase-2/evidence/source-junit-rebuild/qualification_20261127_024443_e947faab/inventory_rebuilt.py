"""Bind independently parsed original source declarations to the rebuilt JUnit plan.

No Java execution or Rust parity is inferred from source declarations alone.
"""
import argparse
import copy
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid
import zipfile

from validate import bind_methods, load, validate_events

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / 'docs/phase-2/evidence/source-junit-rebuild'
BASELINE_CATALOG_SHA256 = 'daf41a3fb5094f4679a273c89a42e05b939edaa3db316b059ec477abe21ae0ab'
BUILD = 'run_20261127_021515_800aeb39'
RUNTIME = 'runtime_20261127_022128_dd3614d0'
JDK = Path('D:/Java/jdk-17.0.12')
TEST_ROOTS = ['rm-datasource/src/test', 'sqlparser/seata-sqlparser-core/src/test']


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def safe_path(root, member):
    """Only canonical relative ZIP/catalog members may enter the new D workspace."""
    need(isinstance(member, str) and member and '\\' not in member and ':' not in member,
         'Nonportable member path')
    need(all(part not in ('', '.', '..') for part in member.split('/')), 'Unsafe member path')
    target = root / member
    need(target.resolve().is_relative_to(root.resolve()), 'Member escaped root')
    return target


def verify_baseline(baseline):
    catalog = baseline / 'evidence-files.json'
    need(sha(catalog) == BASELINE_CATALOG_SHA256, 'Rebuilt baseline authority differs')
    rows = json.loads(catalog.read_bytes())
    need(len({row['path'] for row in rows}) == len(rows), 'Duplicate catalog member')
    for row in rows:
        path = safe_path(baseline, row['path'])
        need(path.is_file() and path.stat().st_size == row['bytes'] and sha(path) == row['sha256'],
             'Baseline member changed: ' + row['path'])


def declaration_controls(declarations, groups):
    rejected = []
    for label, changed in [('missing-declaration', declarations[:-1]),
                           ('duplicate-declaration', declarations + [declarations[0]])]:
        try:
            bind_methods(changed, groups)
        except ValueError:
            rejected.append(label)
        else:
            raise ValueError('Weakened original declaration set accepted: ' + label)
    wrong = copy.deepcopy(groups)
    wrong[0]['cases'][0]['declaring_class'] = 'unknown.Source'
    try:
        bind_methods(declarations, wrong)
    except ValueError:
        rejected.append('foreign-original-method')
    else:
        raise ValueError('Unbound original runtime method accepted')
    return rejected


def run(baseline=BASELINE):
    verify_baseline(baseline)
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_rebuilt_inventory_' + stamp)
    work.mkdir()
    source = work / 'source'
    source.mkdir()
    classes = work / 'classes'
    classes.mkdir()
    out = baseline / ('inventory_' + stamp)
    out.mkdir()
    print('REBUILT_SOURCE_INVENTORY ' + str(out), flush=True)
    build = baseline / BUILD
    literal = json.loads((build / 'literal-git-inputs.json').read_bytes())
    expected = {row['path']: row for row in literal}
    need(len(literal) == len(expected) == 4269, 'Original Git file denominator differs')
    with zipfile.ZipFile(build / 'literal-source.zip') as archive:
        need(len(archive.namelist()) == len(expected) and set(archive.namelist()) == set(expected),
             'Original source ZIP file set differs')
        for name, row in expected.items():
            data = archive.read(name)
            need(hashlib.sha256(data).hexdigest() == row['sha256'] and
                 hashlib.sha1(('blob ' + str(len(data)) + '\0').encode() + data).hexdigest() == row['git_blob'],
                 'Original Git source bytes differ')
            path = safe_path(source, name)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    helper = ROOT / 'tools/formal-undo-format/SourceCaseInventory.java'
    helper_copy = work / helper.name
    helper_copy.write_bytes(helper.read_bytes())
    (out / helper.name).write_bytes(helper.read_bytes())
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp',
                            JAVA_TOOL_OPTIONS='-Duser.home=' + str(work) + ' -Djava.io.tmpdir=D:/Temp')
    for key in ('JDK_JAVA_OPTIONS', '_JAVA_OPTIONS'):
        env.pop(key, None)
    commands = [[str(JDK / 'bin/javac.exe'), '-encoding', 'UTF-8', '-d', str(classes), str(helper_copy)],
                [str(JDK / 'bin/java.exe'), '-cp', str(classes), 'SourceCaseInventory',
                 str(work / 'canonical-methods.jsonl'), *[str(source / p) for p in TEST_ROOTS]]]
    for index, command in enumerate(commands):
        with (out / ('command-' + str(index) + '.log')).open('wb') as log:
            completed = subprocess.run(command, cwd=work, env=env, stdout=log,
                                       stderr=subprocess.STDOUT, timeout=120,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
        need(completed.returncode == 0, 'Original source AST helper failed')
    declarations = load(work / 'canonical-methods.jsonl')
    # The shared historical parser emits placeholder Rust fields; those are not a new parity verdict.
    (out / 'parser-raw-methods.jsonl').write_bytes((work / 'canonical-methods.jsonl').read_bytes())
    java_files = [p for folder in TEST_ROOTS for p in (source / folder).rglob('*.java')]
    need(len(java_files) == 189, 'Original Java test-file denominator differs')
    assets = []
    for folder in TEST_ROOTS:
        for p in (source / folder).rglob('*'):
            if p.is_file() and p.suffix != '.java':
                relative = p.relative_to(source).as_posix()
                copy_path = ROOT / 'tests/fixtures/mysql/java_git_assets' / relative
                need(copy_path.read_bytes() == p.read_bytes(), 'Original test fixture copy changed')
                assets.append(dict(source_git_relative_path=relative, sha256=sha(p)))
    need(len(assets) == 7, 'Original source asset denominator differs')
    for declaration in declarations:
        p = Path(declaration['source'])
        relative = p.relative_to(source).as_posix()
        need(relative in expected and sha(p) == expected[relative]['sha256'], 'AST source not a pinned Git blob')
        declaration['source_git_relative_path'] = relative
        declaration['source_sha256'] = expected[relative]['sha256']
    groups = [validate_events(load(baseline / RUNTIME / (name + '-events.jsonl')), name)
              for name in ['seata-sqlparser-core', 'rm-datasource']]
    methods = bind_methods(declarations, groups)
    controls = declaration_controls(declarations, groups)
    for row in methods:
        row.pop('Rust_disposition')
        row.pop('differential')
    for row in literal:
        need(sha(source / row['path']) == row['sha256'], 'AST extraction mutated original source')
    save(out / 'source-method-bindings.json', methods)
    save(out / 'source-assets.json', assets)
    save(out / 'commands.json', dict(argv=commands, cwd=str(work), JAVA_TOOL_OPTIONS=env['JAVA_TOOL_OPTIONS']))
    save(out / 'helper-artifacts.json', [dict(path=str(p), sha256=sha(p)) for p in sorted(classes.rglob('*.class'))])
    save(out / 'result.json', dict(status='ORIGINAL_AST_AND_JUNIT_METHODS_BOUND',
         baseline_catalog_sha256=BASELINE_CATALOG_SHA256, java_pin='cba73e180fd8896a99df6415b859d275122feaa7',
         helper_sha256=sha(helper), original_source_files=4269, original_test_java_files=len(java_files),
         original_source_assets=len(assets), declared_methods=len(methods),
         methods_executed=sum(m['Java_status'] == 'PASSED' for m in methods),
         methods_disabled_JDK17=sum(m['Java_status'] == 'SOURCE_DISABLED_JDK17' for m in methods),
         concrete_case_identities=sum(len(m['concrete_case_ids']) for m in methods),
         negative_controls_rejected=controls, source_files_unchanged=True,
         Rust_parity_assessed=False, independent_human_review='OPEN', full_V1_complete=False))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print((out / 'result.json').read_text(encoding='utf-8'), flush=True)
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, default=BASELINE)
    run(parser.parse_args().baseline)
