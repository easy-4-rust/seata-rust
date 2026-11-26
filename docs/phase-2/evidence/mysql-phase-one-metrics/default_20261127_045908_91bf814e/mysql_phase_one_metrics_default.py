"""Verify the default exporter remains independent of MySQL runtime dependencies."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-phase-one-metrics' / ('default_' + stamp)
    out.mkdir(parents=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'),
        CARGO_TARGET_DIR=str(ROOT.parent / '.seata-target-nodebug'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0')
    commands = []
    for label, args in [('runtime-tree', ['tree', '-p', 'seata-observability', '--edges', 'normal', '--no-default-features', '--offline', '--locked']),
                        ('clippy', ['clippy', '-p', 'seata-observability', '--all-targets', '--no-default-features', '--offline', '--locked', '--', '-D', 'warnings'])]:
        argv = ['cargo', '+1.99.0', *args]
        result = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=300,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(argv=argv, exit_code=result.returncode))
        need(result.returncode == 0, 'Default exporter check failed: ' + label)
        if label == 'runtime-tree':
            need(b'seata-db-mysql' not in result.stdout and b'sqlx' not in result.stdout, 'Default exporter gained MySQL runtime dependency')
    save(out / 'commands.json', commands)
    save(out / 'result.json', dict(status='DEFAULT_EXPORTER_DEPENDENCIES_AND_STRICT_CLIPPY_PASSED',
         mysql_runtime_dependency=False, sqlx_runtime_dependency=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(default_checks=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
