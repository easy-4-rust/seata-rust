"""Independently check actual original temporal values, loss and class origins."""
import argparse
import copy
import datetime
import json
from pathlib import Path
import subprocess
import zipfile

from inventory_rebuilt import ROOT, need, sha
import temporal_java_oracle
from validate_structured_logs import catalog


PROFILES = [('UTC-utc', 'UTC', '+00:00'), ('UTC-east8', 'UTC', '+08:00'),
            ('Asia-Shanghai-utc', 'Asia/Shanghai', '+00:00'), ('Asia-Shanghai-east8', 'Asia/Shanghai', '+08:00')]
DATES = ['1000-01-01', '1500-02-28', '1582-10-04', '1582-10-05', '1582-10-10',
         '1582-10-14', '1582-10-15', '1582-10-29', '1969-12-31', '1970-01-01', '2000-02-29', '9999-12-31', None]
TIMES = ['-838:59:59.000000', '-25:01:02.000000', '-00:00:00.000001', '00:00:00.000000',
         '00:00:00.000001', '12:34:56.123456', '23:59:59.999999', '24:00:00.000000', '838:59:59.000000', None]
TIMESTAMPS = {1: '1970-01-01 00:00:01.000000', 3: '1970-01-01 00:00:01.000001',
              4: '2000-02-29 12:34:56.123456', 5: '2038-01-19 03:14:07.000000',
              6: '2038-01-19 03:14:07.999999', 7: None}


def load(path):
    return json.loads(path.read_bytes())


def report_checks(report, jvm, session):
    need(report['jvm_zone'] == jvm and report['session_zone'] == session and report['undo_zone'] == 'UTC', 'Temporal time zone differs')
    need(report['driver_zone'] == ('UTC' if session == '+00:00' else 'Asia/Shanghai'), 'Driver/session zone mismatch')
    need(report['driver'].startswith('mysql-connector-java-8.0.27 ') and report['server'] == '8.4.13', 'Pinned driver/server differs')
    expected = {('probe_date', i): v for i, v in enumerate(DATES, 1)}
    expected.update({('probe_time', i): v for i, v in enumerate(TIMES, 1)})
    for i, v in TIMESTAMPS.items():
        if v is not None and session == '+08:00':
            v = (datetime.datetime.fromisoformat(v) + datetime.timedelta(hours=8)).strftime('%Y-%m-%d %H:%M:%S.%f')
        expected['probe_timestamp', i] = v
    rows = report['cases']
    need(len(rows) == 29 and len({(r['table'], r['id']) for r in rows}) == 29, 'Temporal cases absent or duplicated')
    failures = lost = unequal = 0
    for row in rows:
        table, identity = row['table'], row['id']
        need((table, identity) in expected and row['physical_before'] == expected[table, identity], 'Physical temporal baseline differs')
        jdbc, klass = {'probe_date': (91, 'java.sql.Date'), 'probe_time': (92, 'java.sql.Time'), 'probe_timestamp': (93, 'java.sql.Timestamp')}[table]
        need(row['jdbc_type'] == jdbc, 'Original JDBC temporal identity differs')
        fails = (table == 'probe_date' and identity in [4, 5, 6]) or (table == 'probe_time' and identity in [1, 2, 8, 9])
        if fails:
            need(row['status'] == 'ORIGINAL_EXCEPTION' and row['failure_stage'] == 'read_original', 'Actual driver exception lost or relabeled')
            need('SQLException' in row['exception'] and ('specified date' if table == 'probe_date' else 'invalid TIME') in row['message'], 'Wrong failure caused original case')
            need('original' not in row and 'undo_utf8' not in row, 'Failed read falsely encoded')
            failures += 1
            continue
        need(row['status'] == 'OBSERVED', 'Original encode/decode/rebind failed unexpectedly')
        original, decoded = row['original'], row['decoded']
        is_null = row['physical_before'] is None
        need(original['class'] == decoded['class'] == (None if is_null else klass) and row['same_class'], 'Original Java value class differs')
        branch = json.loads(row['undo_utf8'])
        need(branch['xid'] == '127.0.0.1:8091:42' and branch['branchId'] == 9223372036854775807, 'Original branch identity lost')
        logs = branch['sqlUndoLogs']
        need(logs[0] == 'java.util.ArrayList' and len(logs[1]) == 1, 'Original concrete log list differs')
        log = logs[1][0]
        need(log['sqlType'] == 'UPDATE' and log['tableName'] == table and log['beforeImage'] == log['afterImage'], 'Original log shape differs')
        fields = log['beforeImage']['rows'][1][0]['fields'][1]
        need(len(fields) == 2 and fields[0]['value'] == identity and fields[0]['type'] == 4, 'Original row identity differs')
        field = fields[1]
        need(field['name'] == 'v' and field['keyType'] == 'NULL' and field['type'] == jdbc, 'Original field identity differs')
        if is_null:
            need(field['value'] is None and original['text'] is None and decoded['text'] is None and row['physical_rebound'] is None, 'SQL NULL changed')
        else:
            need(field['value'][0] == klass, 'Wrong original temporal wrapper')
            payload = field['value'][1]
            if table == 'probe_date':
                need(isinstance(payload, int) and payload == original['epoch_millis'] == decoded['epoch_millis'], 'DATE epoch or original calendar changed')
                expected_millis = {1: -30609792000000, 2: -14825980800000, 3: -12219379200000,
                                   7: -12219292800000, 8: -12218083200000, 9: -86400000,
                                   10: 0, 11: 951782400000, 12: 253402214400000}[identity]
                if jvm == 'Asia/Shanghai':
                    expected_millis -= 28800000
                need(payload == expected_millis, 'DATE JDK hybrid calendar or default-zone epoch changed')
                need(original['text'] == decoded['text'] == expected[table, identity] == row['physical_rebound'], 'DATE physical roundtrip differs')
            elif table == 'probe_time':
                clock = {3: '00:00:00', 4: '00:00:00', 5: '00:00:00', 6: '12:34:56', 7: '23:59:59'}[identity]
                need(payload == clock == original['text'] == decoded['text'] and row['physical_rebound'] == clock + '.000000', 'TIME actual precision loss differs')
                millis = ((int(clock[:2]) * 60 + int(clock[3:5])) * 60 + int(clock[6:])) * 1000
                if jvm == 'Asia/Shanghai':
                    millis -= 28800000
                need(decoded['epoch_millis'] == millis and original['epoch_millis'] == millis + {3: 0, 4: 0, 5: 0, 6: 123, 7: 999}[identity], 'TIME driver/Jackson loss stages differ')
            else:
                instant = datetime.datetime.fromisoformat(TIMESTAMPS[identity])
                delta = instant - datetime.datetime(1970, 1, 1)
                millis = delta.days * 86400000 + delta.seconds * 1000 + delta.microseconds // 1000
                nanos = instant.microsecond * 1000
                need(original['epoch_millis'] == decoded['epoch_millis'] == millis and original['nanoseconds'] == decoded['nanoseconds'] == nanos, 'TIMESTAMP instant or nanos changed')
                need(payload == ([millis, nanos] if nanos % 1000000 else millis), 'Original TIMESTAMP payload differs')
                need(row['physical_rebound'] == expected[table, identity], 'TIMESTAMP changed physical instant')
        same_value = original == decoded
        same_physical = row['physical_before'] == row['physical_rebound']
        need(row['same_value'] == same_value and row['same_physical'] == same_physical, 'Reported fidelity flag contradicts actual values')
        unequal += not same_value
        lost += not same_physical
    need((failures, lost, unequal) == (7, 4, 2), 'Temporal incompatibilities hidden or added')


def verify(folder, digest):
    catalog(folder, digest)
    temporal_java_oracle.verify(folder)
    with zipfile.ZipFile(folder / 'source-inputs.zip') as archive:
        for row in load(folder / 'source-inputs.json'):
            need(sha(ROOT / row['path']) == row['sha256'] and archive.read(row['path']) == (ROOT / row['path']).read_bytes(), 'Temporal executed source differs')
    binding = load(folder / 'java-build/binding.json')
    originals = [r for r in binding['files'] if r['role'] == 'original_source']
    need(len(originals) == 18 and len([r for r in binding['files'] if r['role'] == 'test_harness']) == 1, 'Original source compile closure differs')
    for row in originals:
        relative = row['relative'].removeprefix('src/')
        actual = subprocess.run(['git', '-C', str(ROOT.parent / 'seata-java-phase09-oracle'), 'show', temporal_java_oracle.PIN + ':' + relative],
                                check=True, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW).stdout
        need(actual == (folder / 'java-build/runtime' / row['relative']).read_bytes(), 'Original Java source changed')
    for name, jvm, session in PROFILES:
        report = load(folder / name / 'observations.json')
        report_checks(report, jvm, session)
        for row in report['cases']:
            if row['status'] == 'OBSERVED':
                need((folder / name / (row['table'] + '-' + str(row['id']) + '.json')).read_text(encoding='utf-8') == row['undo_utf8'], 'Actual raw Undo differs')
        classes = (folder / name / 'class-load.log').read_text(encoding='utf-8').splitlines()
        for symbol in ['org.apache.seata.rm.datasource.sql.struct.TableRecords', 'org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser',
                       'org.apache.seata.rm.datasource.sql.struct.cache.TemporalColumnOracle']:
            need(any(symbol + ' source: file:/' in line and '/temporal-oracle/classes/' in line for line in classes), 'Fresh original runtime class not used')
        need(any('com.mysql.cj.jdbc.Driver source: file:/' in line and '/temporal-oracle/lib/mysql-connector-java-8.0.27.jar' in line for line in classes), 'Actual JDBC driver differs')
    commands = load(folder / 'commands.json')
    need(all(r['exit_code'] == (1 if r['label'] in ['timestamp-below-range', 'confirm-removed'] else 0) for r in commands), 'Unexpected command failure')
    need(b'1292' in (folder / 'timestamp-below-range.stderr').read_bytes(), 'MySQL lower-range rejection absent')
    state = load(folder / 'inspect.stdout')[0]
    need(state['Id'] == (folder / 'create.stdout').read_text().strip() and state['Image'] == 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902' and state['Config']['Labels'].get('seata.temporal.owner'), 'Owned MySQL identity differs')
    need(load(folder / 'cleanup.json')['owned_container_removed'], 'Owned MySQL cleanup incomplete')
    result = load(folder / 'result.json')
    need(result['status'] == 'ORIGINAL_TEMPORAL_FIDELITY_OBSERVED' and result['actual_cases'] == 116 and result['rust_implementation_certified'] is False and result['full_temporal_support'] is False and result['whole_plan_complete'] is False, 'Reference falsely certifies support')


def weakenings(folder):
    count = 0
    for name, jvm, session in PROFILES:
        original = load(folder / name / 'observations.json')
        variants = []
        for field, value in [('jvm_zone', 'unknown'), ('undo_zone', 'Asia/Shanghai'), ('driver_zone', 'unknown')]:
            altered = copy.deepcopy(original)
            altered[field] = value
            variants.append(altered)
        altered = copy.deepcopy(original)
        altered['cases'].pop()
        variants.append(altered)
        for table, identity, field, value in [('probe_date', 4, 'status', 'OBSERVED'), ('probe_time', 1, 'failure_stage', 'encode_original'),
                ('probe_time', 6, 'same_value', True), ('probe_time', 5, 'same_physical', True),
                ('probe_timestamp', 6, 'physical_rebound', None), ('probe_date', 1, 'jdbc_type', 93)]:
            altered = copy.deepcopy(original)
            next(r for r in altered['cases'] if r['table'] == table and r['id'] == identity)[field] = value
            variants.append(altered)
        for field, value in [('epoch_millis', 0), ('nanoseconds', 0), ('class', 'java.time.LocalDateTime')]:
            altered = copy.deepcopy(original)
            next(r for r in altered['cases'] if r['table'] == 'probe_timestamp' and r['id'] == 6)['decoded'][field] = value
            variants.append(altered)
        for altered in variants:
            try:
                report_checks(altered, jvm, session)
            except ValueError:
                count += 1
            else:
                raise RuntimeError('Temporal diagnostic weakening accepted')
    return count


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('folder', type=Path)
    parser.add_argument('sha256')
    parser.add_argument('--weakenings', action='store_true')
    args = parser.parse_args()
    verify(args.folder, args.sha256)
    print('Original temporal reference verified; rejected alterations:', weakenings(args.folder) if args.weakenings else 0)
