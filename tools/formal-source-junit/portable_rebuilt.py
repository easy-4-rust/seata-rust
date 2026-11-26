"""Seal and verify the complete input closure of the rebuilt original JUnit run."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import zipfile

from inventory_rebuilt import BASELINE, BUILD, RUNTIME, need, safe_path, save, sha, verify_baseline

AUTH = 'dependency_auth_20261127_022357_e4773a97'
PACKAGE_AUTHORITY_SHA256 = '1ac8fcff14d29fd25bf42d61b4db07b421d1a84cc97c87e35fa7f0a237ebefd7'
MAX_MEMBER_BYTES = 16 * 1024 * 1024
MAX_TOTAL_BYTES = 160 * 1024 * 1024


def classify_input(path, stage, repository):
    path = path.resolve()
    for prefix, root in [('stage', stage), ('m2', repository)]:
        if path.is_relative_to(root.resolve()):
            return prefix + '/' + path.relative_to(root.resolve()).as_posix()
    raise ValueError('Executed input outside original isolated stage/repository: ' + str(path))


def verify_package_authority(data):
    need(hashlib.sha256(data).hexdigest() == PACKAGE_AUTHORITY_SHA256,
         'Portable package authority differs; a self-rehashed manifest is insufficient')
    return json.loads(data)


def validate_members(package, manifest):
    rows = manifest['files']
    expected = {r['member']: r for r in rows}
    need(len(expected) == len(rows) and len(rows) <= 7000, 'Duplicate/excessive package members')
    total = 0
    with zipfile.ZipFile(package / 'runtime-inputs.zip') as archive:
        need(len(archive.infolist()) == len(rows) and set(archive.namelist()) == set(expected),
             'Runtime ZIP member set differs')
        for info in archive.infolist():
            safe_path(Path('D:/codex-builds/member-validation'), info.filename)
            row = expected[info.filename]
            need(info.file_size == row['bytes'] and 0 <= info.file_size <= MAX_MEMBER_BYTES,
                 'Runtime ZIP member size differs')
            need((info.external_attr >> 16) & 0o170000 == 0o100000, 'Runtime ZIP member is not a regular file')
            total += info.file_size
            need(total <= MAX_TOTAL_BYTES, 'Runtime package exceeds extraction budget')
            need(hashlib.sha256(archive.read(info)).hexdigest() == row['sha256'],
                 'Runtime ZIP member checksum differs')
    need(total == manifest['total_bytes'], 'Runtime package total differs')


def seal(baseline, out):
    verify_baseline(baseline)
    out.mkdir()
    build = baseline / BUILD
    runtime = baseline / RUNTIME
    state = json.loads((build / 'compile-result.json').read_bytes())
    actual = json.loads((runtime / 'result.json').read_bytes())
    authentication = json.loads((baseline / AUTH / 'dependency-authentication.json').read_bytes())
    stage = Path(state['work'])
    work = Path(actual['work'])
    repository = Path(authentication['repository'])
    need(authentication['all_authenticated'], 'Maven authentication incomplete')
    official = {r['sha256']: r for r in authentication['files'] if r['path'].endswith('.jar')}
    data_by_member = {}
    bindings = []

    def add(member, data, origin):
        safe_path(Path('D:/codex-builds/package-members'), member)
        need(len(data) <= MAX_MEMBER_BYTES, 'Input exceeds member size budget')
        if member in data_by_member:
            need(data_by_member[member] == data, 'Conflicting package input')
        else:
            data_by_member[member] = data
        bindings.append(dict(original=origin, member=member, sha256=hashlib.sha256(data).hexdigest()))

    literal = json.loads((build / 'literal-git-inputs.json').read_bytes())
    with zipfile.ZipFile(build / 'literal-source.zip') as source_zip:
        for row in literal:
            data = source_zip.read(row['path'])
            need(hashlib.sha256(data).hexdigest() == row['sha256'], 'Literal Git input drift')
            add('stage/source/' + row['path'], data, str(stage / 'source' / row['path']))
    records = json.loads((runtime / 'preexisting-runtime-inputs.json').read_bytes())
    records += json.loads((runtime / 'helper-artifacts.json').read_bytes())
    agent = json.loads((runtime / 'agent-binding.json').read_bytes())
    records += [dict(path=agent['path'], sha256=agent['sha256']),
                dict(path=str(work / 'hosts'), sha256=sha(runtime / 'hosts'))]
    for row in records:
        path = Path(row['path'])
        need(sha(path) == row['sha256'], 'Executed runtime input drift: ' + str(path))
        if path.suffix == '.jar' and path != Path(agent['path']):
            need(row['sha256'] in official, 'Runtime JAR lacks exact official checksum')
        add(classify_input(path, stage, repository), path.read_bytes(), str(path))
    modules = {}
    for name in ['seata-sqlparser-core', 'rm-datasource']:
        command = json.loads((runtime / (name + '-command.json')).read_bytes())
        module = 'sqlparser/seata-sqlparser-core' if name == 'seata-sqlparser-core' else name
        declared = stage / 'source' / module / 'target/declared-test-classpath.txt'
        cp = [work / 'classes', stage / 'source' / module / 'target/test-classes',
              stage / 'source' / module / 'target/classes']
        cp += [Path(p) for p in declared.read_text(encoding='utf-8').strip().split(os.pathsep)]
        argv = command['argv']
        need(argv[argv.index('-cp') + 1] == os.pathsep.join(map(str, cp)), 'Original classpath differs')
        expected_files = {str(p) for root in cp for p in (root.rglob('*') if root.is_dir() else [root]) if p.is_file()}
        need(expected_files.issubset({r['path'] for r in records}), 'Executed classpath footprint incomplete')
        modules[name] = dict(cwd='stage/source/' + module,
                             test_classes='stage/source/' + module + '/target/test-classes',
                             classpath=[classify_input(p, stage, repository) for p in cp],
                             original_POM_JDK17_arguments=command['original_POM_JDK17_arguments'])
    files = []
    need(sum(len(d) for d in data_by_member.values()) <= MAX_TOTAL_BYTES, 'Runtime closure exceeds budget')
    with zipfile.ZipFile(out / 'runtime-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for member, data in sorted(data_by_member.items()):
            info = zipfile.ZipInfo(member, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
            files.append(dict(member=member, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    manifest = dict(format=1, original_stage=str(stage), original_repository=str(repository),
                    baseline_catalog_sha256=sha(baseline / 'evidence-files.json'),
                    Java_pin=state['java_pin'], original_source_files=4269,
                    work_member=classify_input(work, stage, repository),
                    agent_member=classify_input(Path(agent['path']), stage, repository),
                    modules=modules, files=files, original_bindings=bindings,
                    total_bytes=sum(r['bytes'] for r in files),
                    archive_sha256=sha(out / 'runtime-inputs.zip'),
                    official_runtime_jar_digests=sorted({r['sha256'] for r in records
                        if r['path'].endswith('.jar') and r['sha256'] in official}))
    save(out / 'manifest.json', manifest)
    validate_members(out, manifest)
    print(json.dumps(dict(package=str(out), members=len(files), bytes=manifest['total_bytes'],
                          zip_bytes=(out / 'runtime-inputs.zip').stat().st_size,
                          manifest_sha256=sha(out / 'manifest.json'))), flush=True)
    return out


def verify(package):
    manifest = verify_package_authority((package / 'manifest.json').read_bytes())
    need(sha(package / 'runtime-inputs.zip') == manifest['archive_sha256'], 'Portable archive changed')
    validate_members(package, manifest)
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, default=BASELINE)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    seal(args.baseline, args.output)
