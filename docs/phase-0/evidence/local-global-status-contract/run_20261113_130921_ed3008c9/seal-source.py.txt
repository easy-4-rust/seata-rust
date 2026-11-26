"""把本项实际运行和校验器 RED/GREEN 记录汇总为不可覆盖的目录清单。"""
import argparse
import hashlib
import json
from pathlib import Path
import re

from run import ROOT, HERE, put, save, sha
from verify import check
from validate_capture import validate


def seal(out, red_run, red_log, source_red_dir, capture_red_dir):
    expected = check(*[json.loads((out / name).read_bytes()) for name in ['java.json', 'go-native.json', 'go-profile.json']])
    result = json.loads((out / 'result.json').read_bytes())
    if any(result.get(key) != value for key, value in expected.items()):
        raise ValueError('Result summary differs from raw evidence')
    validation = validate(out)
    control_rows = json.loads((red_run / 'input-audit.json').read_bytes())
    old_test = next(row for row in control_rows if Path(row['path']) == HERE / 'test_verify.py')
    old_verifier = next(row for row in control_rows if Path(row['path']) == HERE / 'verify.py')
    old_test_bytes = (red_run / old_test['snapshot']).read_bytes()
    old_verifier_bytes = (red_run / old_verifier['snapshot']).read_bytes()
    if hashlib.sha256(old_test_bytes).hexdigest() != sha(HERE / 'test_verify.py'):
        raise ValueError('RED/GREEN test sources differ')
    if hashlib.sha256(old_verifier_bytes).hexdigest() != old_verifier['sha256']:
        raise ValueError('RED verifier snapshot differs')
    red = red_log.read_text(encoding='utf-8')
    green = (out / 'python-tests.log').read_text(encoding='utf-8')
    if not re.search(r'Ran 11 tests', red) or not re.search(r'FAILED \(failures=10\)', red):
        raise ValueError('Need actual ten failed guard checks and one control')
    if not re.search(r'Ran 22 tests', green) or not re.search(r'\nOK\s*$', green):
        raise ValueError('Need actual twenty-two GREEN checks')
    source_red = (source_red_dir / 'origins-red.log').read_text(encoding='utf-8')
    if not re.search(r'Ran 5 tests', source_red) or not re.search(r'FAILED \(failures=4\)', source_red):
        raise ValueError('Need actual four failed source guards and one control')
    capture_red = (capture_red_dir / 'capture-red.log').read_text(encoding='utf-8')
    if not re.search(r'Ran 6 tests', capture_red) or not re.search(r'FAILED \(failures=2\)', capture_red):
        raise ValueError('Need actual two failed seal guards and four controls')
    put(out / 'tdd/verifier-red.log', red_log.read_bytes())
    put(out / 'tdd/old-verifier.py.txt', old_verifier_bytes)
    put(out / 'tdd/tests.py.txt', old_test_bytes)
    put(out / 'tdd/red-java.json', (red_run / 'java.json').read_bytes())
    put(out / 'tdd/red-go-native.json', (red_run / 'go-native.json').read_bytes())
    put(out / 'tdd/red-go-profile.json', (red_run / 'go-profile.json').read_bytes())
    put(out / 'tdd/origins-red.log', (source_red_dir / 'origins-red.log').read_bytes())
    put(out / 'tdd/pre-origin-verifier.py.txt', (source_red_dir / 'pre-origin-verifier.py').read_bytes())
    put(out / 'tdd/origin-tests.py.txt', (HERE / 'test_origins.py').read_bytes())
    put(out / 'tdd/semantic-green.log', (red_log.parent / 'verifier-green.log').read_bytes())
    put(out / 'tdd/origins-green.log', (source_red_dir / 'origins-green.log').read_bytes())
    put(out / 'tdd/capture-red.log', (capture_red_dir / 'capture-red.log').read_bytes())
    put(out / 'tdd/pre-capture-validator.py.txt', (capture_red_dir / 'pre-capture-validator.py').read_bytes())
    put(out / 'tdd/pre-capture-tests.py.txt', (capture_red_dir / 'pre-capture-tests.py').read_bytes())
    save(out / 'tdd/red-green.json', dict(kind='disabled_verifier_guard_control_not_upstream_production_red',
         red_run=str(red_run), red_failed=10, red_passed_control=1, green_passed=22,
         source_guard_red_failed=4, source_guard_red_control=1,
         seal_guard_red_failed=2, seal_guard_red_controls=4,
         seal_test_fixture_adapter_change='Final tests read execution.json emitted before acceptance; RED clone read prior result.json. Same six guard identities, not byte-identical test sources.',
         identical_test_source_sha256=sha(HERE / 'test_verify.py'), old_verifier_sha256=old_verifier['sha256']))
    put(out / 'seal-source.py.txt', Path(__file__).read_bytes())
    save(out / 'seal-validation.json', validation)
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p), bytes=p.stat().st_size)
               for p in sorted(out.rglob('*')) if p.is_file()]
    save(out / 'catalog.json', dict(kind='bounded_global_status_contract', members=members))
    print(json.dumps(dict(run=str(out), files=len(members), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--red-run', type=Path, required=True)
    parser.add_argument('--red-log', type=Path, required=True)
    parser.add_argument('--source-red-dir', type=Path, required=True)
    parser.add_argument('--capture-red-dir', type=Path, required=True)
    options = parser.parse_args()
    evidence = (ROOT / 'docs/phase-0/evidence/local-global-status-contract').resolve()
    if options.run.resolve().parent != evidence or options.red_run.resolve().parent != evidence:
        raise ValueError('Run directories must stay in this evidence root')
    seal(options.run, options.red_run, options.red_log, options.source_red_dir, options.capture_red_dir)
