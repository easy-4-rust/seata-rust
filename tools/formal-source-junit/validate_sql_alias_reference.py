"""Verify the full native matrix plus original Java alias recognizer and affected regressions."""
import argparse
import json
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, need, sha
from validate_native_sql_matrix import verify


def audit(packet):
    verify(packet, True)
    out = packet / 'alias-reference'
    for row in json.loads((out / 'inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Reference input changed')
    commands = json.loads((out / 'commands.json').read_bytes()); need(all(c['exit_code'] == 0 for c in commands), 'Reference command failed')
    original = json.loads((out / 'result.json').read_bytes())
    expected = {
        'alias_update_explicit': ('UPDATE', ['v', 'note']),
        'alias_update_implicit_quoted': ('UPDATE', ['v']),
        'alias_update_all': ('UPDATE', ['note']),
        'alias_delete_explicit': ('DELETE', []),
        'alias_delete_implicit': ('DELETE', []),
        'qualified_without_alias': ('UPDATE', ['v']),
    }
    need(len(original) == len(expected) and {r['id'] for r in original} == set(expected), 'Original SQL alias coverage differs')
    for row in original:
        kind, columns = expected[row['id']]
        need(row['type'] == kind and row['table'].replace('`', '') == 'sql_matrix_records'
             and (row['alias'] or '').replace('`', '') == ('' if row['id'] == 'qualified_without_alias' else 'r')
             and [c.replace('`', '').split('.')[-1] for c in row.get('columns', [])] == columns, 'Original qualifier/table/columns differ')
    oracle, = [c for c in commands if c['label'] == 'oracle']
    classes = Path(oracle['argv'][oracle['argv'].index('-cp') + 1].split(';')[0])
    for compiled in (out / 'classes').rglob('*.class'):
        need(sha(compiled) == sha(classes / compiled.relative_to(out / 'classes')), 'Executed reference class differs')
    loaded = (out / 'class-load.log').read_text().splitlines()
    for symbol, origin in [('SqlAliasRecognizerOracle', classes.as_posix()),
                           ('org.apache.seata.sqlparser.druid.DruidDelegatingSQLRecognizerFactory', 'seata-sqlparser-druid-2.8.0-SNAPSHOT.jar'),
                           ('org.apache.seata.sqlparser.druid.mysql.MySQLUpdateRecognizer', 'seata-sqlparser-druid-2.8.0-SNAPSHOT.jar'),
                           ('org.apache.seata.sqlparser.druid.mysql.MySQLDeleteRecognizer', 'seata-sqlparser-druid-2.8.0-SNAPSHOT.jar')]:
        need(any(' ' + symbol + ' source: ' in line and origin in line.replace('file:/D:', 'D:') for line in loaded), 'Original class origin differs: ' + symbol)
    regressions = json.loads((packet / 'alias-component-results.json').read_bytes())
    need(len(regressions) == 2 and {r['version'] for r in regressions} == {'1.99', '1.85'}, 'Component toolchains missing')
    for run in regressions:
        need(run['exit_code'] == run['failed'] == 0 and run['passed'] == 128 and run['ignored'] == 12, 'Component regression failed')
        for row in run['logs']: need(sha(packet / row['path']) == row['sha256'], 'Component log changed')
        need(run['clippy_argv'][-2:] == ['-D', 'warnings'], 'Strict Clippy missing')
    print('SQL_ALIAS_REFERENCE_AUDIT_PASS ' + sha(packet / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); audit(parser.parse_args().packet.resolve())
