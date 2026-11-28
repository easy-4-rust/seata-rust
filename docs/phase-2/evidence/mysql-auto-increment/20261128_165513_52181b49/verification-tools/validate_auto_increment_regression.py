"""Require both full original-TC execution and actual dual-toolchain regressions."""
import argparse
import json
import re
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, need, sha
from validate_auto_increment import verify


def verify_regression(out):
    verify(out, True)
    need(sha(Path(__file__)) == sha(out / 'verification-tools/validate_auto_increment_regression.py'), 'Regression verifier snapshot differs')
    runs = json.loads((out / 'component-results.json').read_bytes())
    need(len(runs) == 2 and {r['version'] for r in runs} == {'1.99','1.85'}, 'Regression toolchains missing')
    for run in runs:
        need(run['exit_code'] == run['failed'] == 0 and run['passed'] == 845 and run['ignored'] == 417, 'Workspace checks failed')
        need(run['test_argv'] == ['cargo','+'+run['version']+'.0','test','--offline','--locked','--workspace','--all-features'], 'Workspace test scope changed')
        need(run['clippy_argv'] == ['cargo','+'+run['version']+'.0','clippy','--offline','--locked','-p','seata-datasource','-p','seata-db-mysql','-p','seata-orm-seaorm','--all-targets','--all-features','--','-D','warnings'], 'Strict changed-package lint scope differs')
        for log in run['logs']:
            raw = (out / log['path']).read_bytes(); need(sha(out / log['path']) == log['sha256'], 'Regression log changed')
            if log['path'].endswith('-workspace.log'):
                summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',raw)
                need([sum(int(row[i]) for row in summaries) for i in range(3)] == [845,0,417], 'Actual workspace test counts differ')
    print('AUTO_INCREMENT_REGRESSIONS_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet',type=Path)
    verify_regression(parser.parse_args().packet)
