"""Verify sealed exact temporal adapter observations and deliberately lossy controls."""
import argparse
import copy
import datetime
import json
from pathlib import Path
import zipfile
from inventory_rebuilt import ROOT, need, sha, save
import exact_temporal_java_oracle
from validate_temporal_reference import DATES, TIMES, TIMESTAMPS

DIGEST = '859131ff80e96f0c7741b59272c76708794c39b618532a668924cbe81291c8a4'
PACKET = ROOT / 'docs/phase-2/evidence/mysql-exact-temporal-adapter/java_20261128_105936_612c9866'
FLAGS = ['adapter_lifecycle_verified', 'sqltype_binding_verified', 'null_semantics_verified', 'rollback_verified', 'exception_state_preserved', 'connection_chain_preserved']


def check(report):
    mode, binary, session = report['adapter_mode'], report['server_prepare'], report['session_zone']
    need(mode in ['read_text', 'read_bytes', 'read_bytes_write_text', 'datasource'] and isinstance(binary, bool), 'Protocol or adapter mode differs')
    need(report['jvm_zone'] in ['UTC', 'Asia/Shanghai'] and session in ['+00:00', '+08:00'] and report['undo_zone'] == 'UTC', 'Temporal zone differs')
    need(report['driver_zone'] == ('UTC' if session == '+00:00' else 'Asia/Shanghai'), 'Driver zone differs')
    need(report['server'] == '8.4.13' and report['driver'].startswith('mysql-connector-java-8.0.27 '), 'Driver/server pin differs')
    if mode == 'datasource':
        need(all(report.get(key) is True for key in FLAGS), 'DataSource lifecycle proof absent')
    expected = {('probe_date', i): v for i, v in enumerate(DATES, 1)}
    expected.update({('probe_time', i): v for i, v in enumerate(TIMES, 1)})
    for i, v in TIMESTAMPS.items():
        if v is not None and session == '+08:00':
            v = (datetime.datetime.fromisoformat(v) + datetime.timedelta(hours=8)).strftime('%Y-%m-%d %H:%M:%S.%f')
        expected['probe_timestamp', i] = v
    for fsp, text in enumerate(['12:34:56', '12:34:56.1', '12:34:56.12', '12:34:56.123', '12:34:56.1235', '12:34:56.12346']):
        expected['probe_time_' + str(fsp), 1] = text
        expected['probe_time_' + str(fsp), 2] = None
    rows = report['cases']
    need(len(rows) == len(expected) == len({(r['table'], r['id']) for r in rows}), 'Cases missing or repeated')
    failures = changed = 0
    for row in rows:
        key = (row['table'], row['id'])
        need(key in expected and row['physical_before'] == expected[key], 'Physical baseline differs')
        table, identity = key
        jdbc = 91 if table == 'probe_date' else 93 if table == 'probe_timestamp' else 92
        need(row['jdbc_type'] == jdbc and row['row_binary'] is binary, 'JDBC identity/protocol differs')
        bad_bind = mode == 'read_bytes' and not binary and key == ('probe_time', 3)
        if bad_bind:
            need(row['status'] == 'ORIGINAL_EXCEPTION' and row['failure_stage'] == 'rebind_original' and '-0.000001S' in row['message'], 'Typed client binding counterexample absent')
            failures += 1
            continue
        need(row['status'] == 'OBSERVED' and row['same_class'] is True and row['same_value'] is True and row['original'] == row['decoded'], 'Original parser changed value/class')
        branch = json.loads(row['undo_utf8'])
        need(branch['xid'] == '127.0.0.1:8091:42' and branch['branchId'] == 9223372036854775807, 'Branch identity differs')
        log = branch['sqlUndoLogs'][1][0]
        need(log['sqlType'] == 'UPDATE' and log['tableName'] == table and log['beforeImage'] == log['afterImage'], 'Original Undo graph differs')
        fields = log['beforeImage']['rows'][1][0]['fields'][1]
        need(len(fields) == 2 and fields[0]['value'] == identity and fields[1]['name'] == 'v' and fields[1]['type'] == jdbc, 'Original field identity differs')
        lossy = mode == 'read_text' and table == 'probe_time' and identity in ([1, 2, 3] if binary else [3])
        rebound = expected[key]
        if lossy:
            rebound = {1: '-794:59:59.000000', 2: '-23:01:02.000000', 3: '00:00:00.000001'}[identity]
            changed += 1
        need(row['physical_rebound'] == rebound and row['same_physical'] is (not lossy), 'Physical rebound/counterexample differs')
        if jdbc != 93:
            need(fields[1]['value'] == row['original']['text'], 'Exact String Undo payload differs')
            text = row['original']['text']
            canonical = rebound
            if mode == 'read_text' and jdbc == 92 and canonical is not None:
                canonical = canonical.rstrip('0').rstrip('.') if '.' in canonical else canonical
            need(text == canonical, 'String physical value differs')
            need(row['original']['class'] == (None if rebound is None else 'java.lang.String'), 'Exact value is not natural String/NULL')
        else:
            need(row['original']['class'] == (None if rebound is None else 'java.sql.Timestamp'), 'TIMESTAMP class changed')
    return dict(cases=len(rows), failures=failures, changed=changed)


def main():
    need(sha(PACKET / 'catalog.json') == DIGEST, 'Authority catalog changed')
    for item in json.loads((PACKET / 'catalog.json').read_bytes()):
        need(sha(PACKET / item['path']) == item['sha256'], 'Evidence member changed')
    with zipfile.ZipFile(PACKET / 'source-inputs.zip') as inputs:
        for item in json.loads((PACKET / 'source-inputs.json').read_bytes()):
            import hashlib
            need(sha(ROOT / item['path']) == item['sha256'] == hashlib.sha256(inputs.read(item['path'])).hexdigest(), 'Executed source binding changed')
    exact_temporal_java_oracle.verify(PACKET)
    result = json.loads((PACKET / 'result.json').read_bytes())
    totals = [0, 0, 0]
    controls = 0
    seen = set()
    for profile in result['profiles']:
        report = json.loads((PACKET / profile['path'] / 'observations.json').read_bytes())
        key = (report['jvm_zone'], report['session_zone'], report['adapter_mode'], report['server_prepare'])
        need(key not in seen, 'Repeated profile'); seen.add(key)
        actual = check(report)
        need(actual == dict(cases=profile['cases'], failures=profile['original_exceptions'], changed=profile['changed_physical']), 'Reported counts differ')
        for i, name in enumerate(['cases', 'failures', 'changed']): totals[i] += actual[name]
        if report['adapter_mode'] == 'datasource':
            for flag in FLAGS:
                bad = copy.deepcopy(report); bad[flag] = False
                try: check(bad)
                except (AssertionError, RuntimeError, ValueError): controls += 1
                else: raise AssertionError('Weakened lifecycle accepted')
            bad = copy.deepcopy(report); bad['cases'][0]['physical_rebound'] = '1000-01-02'
            try: check(bad)
            except (AssertionError, RuntimeError, ValueError): controls += 1
            else: raise AssertionError('Changed date accepted')
    need(len(seen) == 32 and totals == [1312, 4, 16] and controls == 56, 'Coverage differs')
    bounds = json.loads((PACKET / 'time-bounds.json').read_bytes())
    need(len(bounds) == 7 and bounds[0]['exit_code'] == 0 and all(r['exit_code'] != 0 for r in bounds[1:]), 'Server TIME limits differ')
    need(json.loads((PACKET / 'cleanup.json').read_bytes())['owned_container_removed'] is True, 'Owned server cleanup missing')
    out = ROOT / 'docs/phase-2/evidence/mysql-exact-temporal-adapter/independent_validation_20261128.json'
    save(out, dict(status='VERIFIED_EXACT_DATASOURCE_COMPONENT', authority_catalog_sha256=DIGEST, observed_rows=1312, exact_datasource_rows=328, deliberately_lossy_physical_rows=16, deliberately_failed_client_typed_bindings=4, rejected_weakening_controls=controls, full_java_rm_or_rust_sql_certified=False))
    print(out.read_text(encoding='utf-8'))


if __name__ == '__main__':
    main()
