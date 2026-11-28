"""Bind the real rbatis/Java TC packet to dual-workspace, cache and boundary checks."""
import argparse
import json
import re
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, need, sha
from validate_rbatis_mysql import verify


def verify_regression(out):
    verify(out, True)
    need(sha(Path(__file__)) == sha(out / 'verification-tools/validate_rbatis_regression.py'), 'Regression verifier snapshot differs')
    commands = {r['label']: r for r in json.loads((out / 'regression/commands.json').read_bytes())}
    inputs = json.loads((out / 'regression/source-inputs.json').read_bytes())
    need(inputs == json.loads((out / 'regression/source-end.json').read_bytes()), 'Regression source drift')
    for name, digest in inputs.items():
        need(sha(ROOT / name) == digest == sha(out / 'source' / name), 'Regression runtime changed: ' + name)
    totals = []
    for version in ('1.99', '1.85'):
        for label in ('workspace', 'clippy', 'compatibility', 'compatibility-clippy'):
            row = commands[version + '-' + label]
            need(row['exit_code'] == 0, 'Component execution failed')
        row = commands[version + '-workspace']
        need(row['argv'] == ['cargo','+'+version+'.0','test','--offline','--locked','--workspace','--all-features'], 'Workspace scope differs')
        raw = (out / ('regression/' + version + '-workspace.stdout')).read_bytes()
        summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw)
        total = [sum(int(item[i]) for item in summaries) for i in range(3)]
        need(total == [861, 0, 420] and row['passed'] == 861 and row['ignored'] == 420, 'Actual regression count differs')
        totals.append(total)
        need(commands[version+'-clippy']['argv'] == ['cargo','+'+version+'.0','clippy','--offline','--locked','-p','seata-at','-p','seata-db-mysql','-p','seata-orm-seaorm','-p','seata-orm-rbatis','--all-targets','--all-features','--','-D','warnings'], 'Strict package scope differs')
        raw = (out / ('regression/' + version + '-compatibility.stdout')).read_bytes()
        need(b'9 passed; 0 failed; 0 ignored' in raw and b'cache_checks::statement_cache_preserves_mutation_replacement_and_lru_resources ... ok' in raw, 'Actual cache/API checks missing')
    core = (out / 'regression/core-default-tree.stdout').read_bytes()
    need(not re.search(rb'\b(rbatis|sea-orm|time) v\d', core), 'Core acquired an ORM/time dependency')
    need(b'sqlx v' not in (out / 'regression/seaorm-default-tree.stdout').read_bytes(), 'SeaORM default acquired a driver')
    normal = (out / 'regression/workspace-normal-tree.stdout').read_bytes()
    need(not re.search(rb'\b(rsa|libsqlite3-sys|sqlx-sqlite|rbdc-mysql) v\d', normal), 'Unexpected normal driver activation')
    need(b'rbatis v4.9.11' in normal and b'lru v0.18.2' in normal and b'time v0.3.45+seata.1' in normal, 'Actual maintained graph differs')
    need(b'lru v0.14' not in normal, 'Vulnerable cache remained active')
    publication = json.loads((out / 'regression/lru-publication.json').read_bytes())['version']
    need(publication['num'] == '0.18.2' and publication['rust_version'] == '1.85.0' and not publication['yanked'] and publication['checksum'] == '5d2f2f9b4ba7e6b24d95e7e899329d35be83bcded72c8540cdd5368932d1d90a', 'Official cache source differs')
    for label, expected in [('cargo-audit', {'RUSTSEC-2026-0459'}), ('compatibility-audit', set())]:
        audit = json.loads((out / ('regression/' + label + '.stdout')).read_bytes())
        need(audit['settings']['ignore'] == [] and {r['advisory']['id'] for r in audit['vulnerabilities']['list']} == expected, 'Audit findings changed or hidden')
        warnings = [r['advisory']['id'] for group in audit['warnings'].values() for r in group]
        need(not {'RUSTSEC-2026-0002','RUSTSEC-2026-0253'}.intersection(warnings), 'Old cache unsoundness warning remained')
    need(all(row['exit_code'] == (1 if name == 'cargo-audit' else 0) for name, row in commands.items()), 'Unexpected boundary exit')
    print('RBATIS_REGRESSIONS_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('packet', type=Path)
    verify_regression(parser.parse_args().packet)
