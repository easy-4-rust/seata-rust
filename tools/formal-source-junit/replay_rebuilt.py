"""Execute all rebuilt original JUnit cases from only the sealed portable inputs.

Requires the fixed installed JDK, no old Maven cache, original workdir or oracle.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import urllib.parse
import uuid
import zipfile

from audit_rebuilt import run as audit_runtime
from inventory_rebuilt import BASELINE, JDK, need, safe_path, save, sha
from portable_rebuilt import verify
import process_proof


def validate_origins(out, module, cp, rows, official):
    history = [(m[1], m[2].replace('\\', '/')) for m in re.finditer(
        r'\[class,load\] (\S+) source: ([^\r\n]+)',
        (out / (module + '-class-load.log')).read_text(encoding='utf-8'))]
    loaded = dict(history)
    need(loaded.get('org.junit.jupiter.engine.JupiterTestEngine') == rows[0]['jupiter_origin'] and
         loaded.get('org.junit.platform.launcher.core.LauncherFactory') == rows[0]['launcher_origin'],
         'Real engine class-loading origin differs')

    def origin_path(value):
        path = urllib.parse.unquote(urllib.parse.urlparse(value).path)
        if re.match(r'^/[A-Za-z]:', path):
            path = path[1:]
        return Path(path).resolve()

    directories = {p.resolve() for p in cp if p.is_dir()}
    jars = {p.resolve() for p in cp if p.is_file() and p.suffix == '.jar'}
    seata = {k: v for k, v in loaded.items() if k.startswith('org.apache.seata.')}
    # A later mock-defined class with the same name must not erase its actual reactor origin.
    owners = {k for k, v in history if k.startswith('org.apache.seata.') and
              v.startswith('file:') and origin_path(v) in directories}
    generated = []
    need(seata, 'No original Seata classes loaded')
    for classname, value in history:
        if not classname.startswith('org.apache.seata.'):
            continue
        if value.startswith('file:'):
            location = origin_path(value)
            if location not in directories:
                owner = None
                if classname.endswith('ConstructorAccess') and location.name == 'reflectasm-1.11.9.jar':
                    owner = classname[:-len('ConstructorAccess')]
                match = re.fullmatch(r'(.+?)ForyRefCodec(?:_\d+|MetaShared\d+_\d+)', classname)
                if match and location.name == 'fory-core-0.12.3.jar':
                    owner = match.group(1)
                need(owner in owners and location in jars and sha(location) in official,
                     'Original Seata fallback or unknown generated class outside sealed reactor')
                generated.append(dict(classname=classname, source_owner=owner,
                                      generator=str(location), generator_sha256=sha(location)))
        else:
            need(value.startswith('__JVM') or value.startswith('__VM') or
                 classname.startswith(value + '$$Lambda$') and value in seata,
                 'Unknown non-file Seata class origin')
    return dict(seata_classes=seata, verified_generator_derivatives=generated,
                launcher=rows[0]['launcher_origin'], jupiter=rows[0]['jupiter_origin'])


def replay(package, output_root=BASELINE):
    manifest = verify(package)
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    root = Path('D:/codex-builds') / ('seata_junit_portable_' + stamp)
    root.mkdir()
    out = output_root / ('portable_replay_' + stamp)
    out.mkdir()
    print('PORTABLE_ORIGINAL_JUNIT_REPLAY ' + str(out), flush=True)
    helpers = out / 'executed-tools'
    helpers.mkdir()
    for name in ['replay_rebuilt.py', 'portable_rebuilt.py', 'inventory_rebuilt.py',
                 'audit_rebuilt.py', 'validate.py', 'process_proof.py']:
        path = Path(__file__).with_name(name)
        shutil.copyfile(path, helpers / name)
    with zipfile.ZipFile(package / 'runtime-inputs.zip') as archive:
        for info in archive.infolist():
            path = safe_path(root, info.filename)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(archive.read(info))
    stage = root / 'stage'
    work = safe_path(root, manifest['work_member'])
    work.mkdir(exist_ok=True)
    (work / 'home').mkdir()
    agent = safe_path(root, manifest['agent_member'])
    hosts = work / 'hosts'
    need(hosts.is_file(), 'Sealed resolver missing')
    for module in manifest['modules'].values():
        for member in module['classpath']:
            path = safe_path(root, member)
            if not path.suffix == '.jar':
                path.mkdir(parents=True, exist_ok=True)
    save(out / 'extraction.json', dict(package=str(package), manifest_sha256=sha(package / 'manifest.json'),
         root=str(root), members=len(manifest['files']), total_bytes=manifest['total_bytes'],
         old_workdir_or_cache_used=False, Maven_or_oracle_used=False))
    receipts = []
    origins = {}
    for name, module in manifest['modules'].items():
        cp = [safe_path(root, p) for p in module['classpath']]
        cp_files = {p for path in cp for p in (path.rglob('*') if path.is_dir() else [path]) if p.is_file()}
        sealed_files = {safe_path(root, r['member']) for r in manifest['files']}
        need(cp_files.issubset(sealed_files), 'Unsealed classpath file')
        probe = work / (name + '-environments')
        probe.mkdir()
        events = work / (name + '-events.jsonl')
        log = work / (name + '-class-load.log')
        env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp',
            JAVA_TOOL_OPTIONS='-Duser.home=' + str(work / 'home') + ' -Djava.io.tmpdir=D:/Temp' +
            ' -Djdk.net.hosts.file=' + str(hosts) + ' -javaagent:' + str(agent) + '=' + str(probe))
        for key in ('JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'MAVEN_ARGS'):
            env.pop(key, None)
        argv = [str(JDK / 'bin/java.exe'), '-Dseata.junit.scope=' + str(stage),
                '-Dseata.junit.work=' + str(work), '-Djava.security.manager=HarnessGuard',
                '-Xlog:class+load=info:file=' + log.as_posix(), *module['original_POM_JDK17_arguments'],
                '-cp', os.pathsep.join(map(str, cp)), 'SourceJUnitRunner',
                str(safe_path(root, module['test_classes'])), str(events), 'execute']
        cwd = safe_path(root, module['cwd'])
        save(out / (name + '-command.json'), dict(argv=argv, cwd=str(cwd),
             JAVA_TOOL_OPTIONS=env['JAVA_TOOL_OPTIONS'], original_pressure_overrides=False))
        with (out / (name + '-stdout.log')).open('wb') as output:
            completed = subprocess.run(argv, cwd=cwd, env=env, stdout=output,
                                       stderr=subprocess.STDOUT, timeout=600,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
        for path in [events, log, work / 'guard-denials.jsonl']:
            if path.is_file():
                shutil.copyfile(path, out / path.name)
        (out / name / 'environments').mkdir(parents=True)
        for path in probe.glob('*.xml'):
            shutil.copyfile(path, out / name / 'environments' / path.name)
        rows = [json.loads(line) for line in events.read_bytes().splitlines()]
        receipts.append(dict(module=name, exit_code=completed.returncode, summary=rows[-1]))
        save(out / 'result.json', dict(mode='execute', source_pin=manifest['Java_pin'], work=str(work),
             modules=receipts, Rust_parity_complete=False, full_V1_complete=False))
        print(json.dumps(receipts[-1]), flush=True)
        need(completed.returncode == 0, 'Original JUnit portable execution failed; diagnostics retained')
        origins[name] = validate_origins(out, name, cp, rows, set(manifest['official_runtime_jar_digests']))
    audit_runtime(out)
    need(not (stage / 'source/rm-datasource/db_store').exists(), 'Original H2 teardown incomplete')
    for row in manifest['files']:
        need(sha(safe_path(root, row['member'])) == row['sha256'], 'Sealed runtime input changed after execution')
    pids = sorted({int(process_proof.read(p)['pid']) for p in out.glob('*/environments/*.xml')})
    query = '$wanted=@(' + ','.join(map(str, pids)) + '); @((Get-CimInstance Win32_Process | Where-Object { $wanted -contains $_.ProcessId }) | ForEach-Object { [pscustomobject]@{pid=$_.ProcessId;command=$_.CommandLine} }) | ConvertTo-Json -Compress'
    stopped = subprocess.run(['powershell', '-NoProfile', '-Command', query],
                             capture_output=True, timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
    need(stopped.returncode == 0, 'Owned JVM process query failed')
    raw = stopped.stdout.decode('utf-8-sig').strip()
    active = json.loads(raw) if raw else []
    active = active if isinstance(active, list) else [active]
    need(not any(str(root).replace('\\', '/') in (p.get('command') or '').replace('\\', '/')
                 for p in active), 'Owned JVM remains active after original JUnit completion')
    save(out / 'owned-process-teardown.json', dict(observed_pids=pids,
         live_pid_snapshot=active, owned_JVMs_stopped=True))
    save(out / 'class-origin-validation.json', origins)
    save(out / 'replay-proof.json', dict(status='PORTABLE_ORIGINAL_JUNIT_EXECUTED',
         package_manifest_sha256=sha(package / 'manifest.json'),
         original_source_files=manifest['original_source_files'], all_input_bytes_unchanged=True,
         H2_teardown_verified=True, old_workdir_or_cache_used=False, Maven_or_oracle_used=False,
         JDK=str(JDK), Rust_parity_assessed=False, independent_human_review='OPEN'))
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--package', type=Path, default=BASELINE / 'portable_inputs_20261127')
    replay(parser.parse_args().package)
