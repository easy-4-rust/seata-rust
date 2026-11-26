"""Retain a failing class-origin regression and restored tool tests in isolation."""
import datetime
import json
from pathlib import Path
import shutil
import subprocess
import sys
import uuid

from inventory_rebuilt import BASELINE, need, save, sha


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    work = Path('D:/codex-builds') / ('seata_rebuilt_tool_mutation_' + stamp)
    work.mkdir()
    out = BASELINE / ('tool_validation_' + stamp)
    out.mkdir()
    here = Path(__file__).resolve().parent
    names = ['test_rebuilt_inputs.py', 'replay_rebuilt.py', 'portable_rebuilt.py',
             'inventory_rebuilt.py', 'audit_rebuilt.py', 'validate.py', 'process_proof.py']
    for name in names:
        shutil.copyfile(here / name, work / name)
    target = work / 'replay_rebuilt.py'
    original = target.read_text(encoding='utf-8')
    before = "owners = {k for k, v in history if k.startswith('org.apache.seata.') and"
    after = "owners = {k for k, v in seata.items() if k.startswith('org.apache.seata.') and"
    need(original.count(before) == 1, 'Class origin history mutation site differs')
    target.write_text(original.replace(before, after), encoding='utf-8')
    shutil.copyfile(target, out / 'mutated-replay-owner-history.py')
    with (out / 'mutation-red.log').open('wb') as log:
        red = subprocess.run([sys.executable, '-B', '-m', 'unittest', 'discover',
                              '-s', str(work), '-p', 'test_rebuilt_inputs.py'],
                             stdout=log, stderr=subprocess.STDOUT, timeout=60,
                             creationflags=subprocess.CREATE_NO_WINDOW)
    need(red.returncode != 0, 'Lost class origin history was not detected')
    failed = (out / 'mutation-red.log').read_text(encoding='utf-8')
    need('test_later_defined_copy_does_not_erase_original_file_owner' in failed and
         'Original Seata fallback or unknown generated class outside sealed reactor' in failed,
         'Mutation did not fail at the intended class origin assertion')
    with (out / 'restored-green.log').open('wb') as log:
        green = subprocess.run([sys.executable, '-B', '-m', 'unittest', 'discover',
                                '-s', str(here), '-p', 'test_rebuilt_inputs.py'],
                               stdout=log, stderr=subprocess.STDOUT, timeout=60,
                               creationflags=subprocess.CREATE_NO_WINDOW)
    need(green.returncode == 0, 'Restored current tool checks failed')
    save(out / 'result.json', dict(status='REBUILT_TOOLS_REGRESSION_VERIFIED',
         tests=6, mutation_failed_at_target=True, restored_passed=True,
         repository_source_edited_for_mutation=False, mutation_workspace=str(work),
         original_sources=[dict(path=str(here / name), sha256=sha(here / name)) for name in names]))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    run()
