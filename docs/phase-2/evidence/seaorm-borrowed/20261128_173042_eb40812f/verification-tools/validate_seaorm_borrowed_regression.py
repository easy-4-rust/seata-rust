"""Require both full original-TC execution and actual dual-toolchain regressions."""
import argparse
import json
import re
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, need, sha
from validate_seaorm_borrowed import verify


def verify_regression(out):
    verify(out, True)
    need(sha(Path(__file__)) == sha(out / 'verification-tools/validate_seaorm_borrowed_regression.py'), 'Regression verifier snapshot differs')
    runs = json.loads((out / 'component-results.json').read_bytes())
    need(len(runs) == 2 and {r['version'] for r in runs} == {'1.99','1.85'}, 'Regression toolchains missing')
    for run in runs:
        need(run['exit_code'] == run['failed'] == 0 and run['passed'] == 850 and run['ignored'] == 418, 'Workspace checks failed')
        need(run['test_argv'] == ['cargo','+'+run['version']+'.0','test','--offline','--locked','--workspace','--all-features'], 'Workspace test scope changed')
        need(run['clippy_argv'] == ['cargo','+'+run['version']+'.0','clippy','--offline','--locked','-p','seata-at','-p','seata-db-mysql','-p','seata-orm-seaorm','--all-targets','--all-features','--','-D','warnings'], 'Strict changed-package lint scope differs')
        for log in run['logs']:
            raw = (out / log['path']).read_bytes(); need(sha(out / log['path']) == log['sha256'], 'Regression log changed')
            if log['path'].endswith('-workspace.log'):
                summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',raw)
                need([sum(int(row[i]) for row in summaries) for i in range(3)] == [850,0,418], 'Actual workspace test counts differ')
    need(b'sea-orm v' not in (out / 'core-default-tree.stdout').read_bytes(), 'Core acquired an ORM dependency')
    need(b'sqlx v' not in (out / 'seaorm-default-tree.stdout').read_bytes(), 'Default ORM adapter enabled an independent driver')
    normal=(out / 'workspace-normal-tree.stdout').read_bytes()
    need(not re.search(rb'\b(rsa|time|libsqlite3-sys|sqlx-sqlite) v\d',normal) and b'sqlx-mysql v0.8.6' in normal and b'sea-orm v1.1.19' in normal, 'Optional native-driver activation differs')
    audit=json.loads((out / 'cargo-audit.stdout').read_bytes())
    need(audit['settings']['ignore']==[] and {row['advisory']['id'] for row in audit['vulnerabilities']['list']}=={'RUSTSEC-2026-0459','RUSTSEC-2026-0009'}, 'Audit findings were hidden or changed')
    need(all(row['exit_code']==(1 if row['label']=='cargo-audit' else 0) for row in json.loads((out / 'boundary-commands.json').read_bytes())), 'Boundary/audit command results differ')
    print('SEAORM_BORROWED_REGRESSIONS_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet',type=Path)
    verify_regression(parser.parse_args().packet)
