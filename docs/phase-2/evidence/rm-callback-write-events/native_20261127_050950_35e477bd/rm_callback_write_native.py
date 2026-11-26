"""Run native RM events and reject removal of the actual callback write settlement."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile

from inventory_rebuilt import ROOT, need, save, sha


def run():
    out = ROOT / 'docs/phase-2/evidence/rm-callback-write-events' / ('native_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir(parents=True)
    print('CALLBACK_WRITE_NATIVE ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0')
    files = [p for p in (ROOT / 'crates/seata_rm').rglob('*') if p.is_file()]
    sources = []
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted(files):
            path = p.relative_to(ROOT).as_posix()
            archive.writestr(path, p.read_bytes())
            sources.append(dict(path=path, sha256=sha(p)))
    save(out / 'source-inputs.json', sources)
    commands = []

    def execute(version, label, arguments, expected=0):
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        argv = ['cargo', '+' + version + '.0', *arguments]
        result = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (version + '-' + label + '.stdout')).write_bytes(result.stdout)
        (out / (version + '-' + label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(Rust=version, label=label, argv=argv, exit_code=result.returncode))
        save(out / 'commands.json', commands)
        need(result.returncode == expected, 'Native RM event result differs: ' + label + '\n' + (result.stdout + result.stderr)[-2000:].decode(errors='replace'))
        return result

    target = ROOT / 'crates/seata_rm/src/manager/callback_handler.rs'
    original = target.read_bytes()
    needle = b'        event.finish(&result);'
    need(original.count(needle) == 1, 'Actual callback write settlement target ambiguous')
    mutated = original.replace(needle, b'        drop((event, result));')
    for version in ['1.99', '1.85']:
        execute(version, 'rm-tests', ['test', '-p', 'seata-rm', '--all-features', '--offline', '--locked'])
        arguments = ['test', '-p', 'seata-rm', '--test', 'structured_events', 'callback_decisions_emit_original_branch_identity_without_replaying_effect', '--offline', '--locked', '--', '--exact']
        try:
            target.write_bytes(mutated)
            result = execute(version, 'mutation', arguments, expected=101)
            need(b'cancelled' in result.stdout and b'local_written' in result.stdout, 'Mutation did not fail at actual write classification')
            save(out / (version + '-mutation.json'), dict(path=target.relative_to(ROOT).as_posix(), original_sha256=__import__('hashlib').sha256(original).hexdigest(), mutation_sha256=sha(target), actual_settlement_removed=True))
        finally:
            target.write_bytes(original)
        execute(version, 'restored', arguments)
    for row in sources:
        need(sha(ROOT / row['path']) == row['sha256'], 'Final native RM source drift')
    save(out / 'result.json', dict(status='NATIVE_RM_WRITE_EVENTS_AND_SETTLEMENT_MUTATION_VERIFIED', Rust_versions=['1.99', '1.85'], mutation_rejected_per_version=True, restored_target_passed_per_version=True, WP07_complete=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    run()
