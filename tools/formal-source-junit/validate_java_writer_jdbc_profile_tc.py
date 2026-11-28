"""Independently check original TC/RM integration bytes, database results and runtime origins."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/formal-tc-lock-coverage'))
from live import module, need, reg, sha


def verify(out, full):
    names = set()
    for row in json.loads((out / 'catalog.json').read_bytes())['members']:
        name = row['path']
        need(not Path(name).is_absolute() and '..' not in Path(name).parts and name not in names, 'Invalid member')
        names.add(name); need(sha(out / name) == row['sha256'], 'Catalog member differs: ' + name)
    need(names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'},
         'Catalog coverage differs')
    result = json.loads((out / 'result.json').read_bytes())
    profiles, directions = result['profiles'], result['directions']
    need(result['status'] == 'passed' and result['mocks'] is False and result['tc_jar_sha256'] == reg.JAR_SHA,
         'Original integration status differs')
    need(len(profiles) == len(set(profiles)) and set(profiles) <= set(range(1, 7))
         and len(directions) == len(set(directions)) and set(directions) <= {'commit', 'rollback'}, 'Invalid coverage')
    if full:
        need(set(profiles) == set(range(1, 7)) and set(directions) == {'commit', 'rollback'}, 'Full matrix missing')
    need(result['cases'] == len(profiles) * len(directions) and result['rejected'] == result['cases'] * 5,
         'Integration counts differ')
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Executed source drift')
    for row in json.loads((out / 'dependencies.json').read_bytes()):
        need(sha(Path(row['path'])) == row['sha256'], 'Runtime dependency drift')
    tc = json.loads((out / 'tc-command.json').read_bytes())
    need(tc['jar_sha256'] == reg.JAR_SHA and 'org.springframework.boot.loader.launch.PropertiesLauncher' in tc['argv'],
         'Original TC command differs')
    decoder = module('full_jdbc_profile_evidence_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    observations = json.loads((out / 'observations.json').read_bytes())
    need({(o['profile'], o['direction']) for o in observations} == {(p, d) for p in profiles for d in directions},
         'Observed matrix differs')
    need(len({o['xid'] for o in observations}) == len(observations)
         and len({o['branch_id'] for o in observations}) == len(observations), 'Branch identities reused')
    branch_enum = (out / 'branch-status-source.stdout').read_text(encoding='utf-8')
    need('PhaseTwo_Committed(5)' in branch_enum and 'PhaseTwo_Rollbacked(8)' in branch_enum, 'Original branch enum differs')
    commands = json.loads((out / 'commands.json').read_bytes())
    for command in commands:
        held = command['label'].endswith('-binding-held')
        need(command['exit_code'] != 0 if held else command['exit_code'] == 0, 'Command failed: ' + command['label'])
        if held: need(b'1205' in (out / (command['label'] + '.stderr')).read_bytes(), 'No native lock timeout')
    for observation in observations:
        profile, direction = observation['profile'], observation['direction']
        label = str(profile) + '-' + direction; case = out / label
        phase = json.loads((case / 'phase-one.json').read_bytes())
        need((observation['xid'], observation['branch_id']) == (phase['xid'], phase['branch_id'])
             and observation['terminal_counts'] == [0, 0, 0, 0], 'Observed branch differs')
        rejects = json.loads((case / 'rejections.json').read_bytes())
        need(len(rejects) == 5 and {r['bound'] for r in rejects} == set(range(1, 7)) - {profile}
             and all(r['actual'] == profile and r['reason'] == 'jdbc-profile' for r in rejects), 'Rejected profiles differ')
        early = json.loads((case / 'pre-begin-frames.json').read_bytes())
        need(not any(decoder.decode(r)['code'] in (1, 7, 9, 11, 13) for r in early if r['event'] == 'wire'),
             'Rejected writer sent a transaction request')
        need((out / (label + '-rejected-counts.stdout')).read_bytes().strip() == b'0\t0\t0\t0', 'Rejected state persisted')
        terminal = [c for c in commands if c['label'].startswith(label + '-terminal-poll')]
        need(terminal and (out / (terminal[-1]['label'] + '.stdout')).read_bytes().strip() == b'0\t0\t0\t0',
             'Native terminal state missing')
        before = (out / (label + '-before.stdout')).read_bytes()
        need((out / (label + '-rejected-business.stdout')).read_bytes() == before, 'Rejected writer changed business')
        after = (out / (label + '-phase-business.stdout')).read_bytes()
        need(after != before and (out / (label + '-final-business.stdout')).read_bytes()
             == (before if direction == 'rollback' else after), 'Business completion differs')
        graph = json.loads((case / 'original-undo.json').read_bytes())
        need((case / 'original-undo.json').read_bytes() == bytes.fromhex(phase['undo_hex'])
             and graph['xid'] == phase['xid'] and graph['branchId'] == phase['branch_id'], 'Original Undo differs')
        need(graph['sqlUndoLogs'][0] == 'java.util.ArrayList' and len(graph['sqlUndoLogs'][1])
             == observation['sql_undo_logs'] == (7 if profile >= 5 else 1), 'Undo coverage differs')
        expected_locks = {('admission_business', '1')}
        if profile >= 5:
            expected_after = ('1\t10\n1\t1582-10-10\t24:00:00.000001\n'
                '3\t1000-01-01\t-838:59:59.000000\n1582-10-05\t20\n1582-10-10\t30\n'
                '-25:01:02.123456\t30\n').encode()
            need(after == expected_after, 'Exact temporal phase one differs')
            fields = []
            def walk(node):
                if isinstance(node, dict):
                    if node.get('type') in (91, 92): fields.append(node)
                    for value in node.values(): walk(value)
                elif isinstance(node, list):
                    for value in node: walk(value)
            walk(graph)
            need({f['type'] for f in fields} == {91, 92} and all(isinstance(f['value'], str) for f in fields)
                 and fields == json.loads((case / 'temporal-undo-fields.json').read_bytes()), 'Exact Undo field classes differ')
            expected_locks |= {('temporal_values', str(i)) for i in (1, 2, 3)}
            expected_locks |= {('temporal_dates', k) for k in ('1582-10-05', '1582-10-10')}
            expected_locks.add(('temporal_times', '-25:01:02.123456'))
        tc_rows = [line.split('\t') for line in (out / (label + '-phase-tc.stdout')).read_text().splitlines()]
        need(tc_rows[0] == [phase['xid'], str(phase['branch_id']), phase['resource'], '1']
             and len(tc_rows[1:]) == len(expected_locks)
             and all(r[:2] == [phase['xid'], str(phase['branch_id'])] for r in tc_rows[1:])
             and {tuple(r[2:]) for r in tc_rows[1:]} == expected_locks, 'Native TC row identity differs')
        frames = json.loads((case / 'proxy-events.json').read_bytes())
        proxy = json.loads((case / 'proxy-result.json').read_bytes())
        need(not proxy['errors'] and not proxy['field_translation'] and not proxy['fault_injection'], 'Proxy altered/faulted')
        decoded = [(r, decoder.decode(r)) for r in frames if r['event'] == 'wire']
        registers = [v for r, v in decoded if r['direction'] == 'sdk-to-tc' and v['code'] == 11 and v['xid'] == phase['xid']]
        need(len(registers) == 1 and registers[0]['xid'] == phase['xid'] and registers[0]['resource'] == phase['resource'],
             'Original registration differs')
        registration = registers[0]
        registered = [v for r, v in decoded if r['direction'] == 'tc-to-sdk' and v['code'] == 12
                      and v['connection'] == registration['connection'] and v['id'] == registration['id']]
        need(len(registered) == 1 and registered[0]['result'] == 1 and registered[0]['branch_id'] == phase['branch_id'],
             'Original registration response differs')
        tm_code, tm_reply, tm_status = (9, 10, 11) if direction == 'rollback' else (7, 8, 9)
        decisions = [v for r, v in decoded if r['direction'] == 'sdk-to-tc' and v['code'] == tm_code and v['xid'] == phase['xid']]
        need(len(decisions) == 1 and decisions[0]['xid'] == phase['xid'], 'Original TM decision differs')
        decided = [v for r, v in decoded if r['direction'] == 'tc-to-sdk' and v['code'] == tm_reply
                   and v['connection'] == decisions[0]['connection'] and v['id'] == decisions[0]['id']]
        # AT 提交可以先返回异步提交中（8），最终仍须由原回调及数据库空终态证明完成。
        need(len(decided) == 1 and decided[0]['result'] == 1
             and decided[0]['status'] in ({8, tm_status} if direction == 'commit' else {tm_status}),
             'Original TM response differs')
        callback, reply, status = (5, 6, 8) if direction == 'rollback' else (3, 4, 5)
        callbacks = [(r, v) for r, v in decoded if r['direction'] == 'tc-to-sdk' and v['code'] == callback and v['xid'] == phase['xid']]
        replies = [(r, v) for r, v in decoded if r['direction'] == 'sdk-to-tc' and v['code'] == reply and v['xid'] == phase['xid']]
        need(callbacks and replies, 'Callback missing')
        for request, value in callbacks:
            matches = [v for r, v in replies if r['connection'] == request['connection'] and r['id'] == request['id']]
            need(len(matches) == 1 and value['xid'] == phase['xid'] and value['branch_id'] == phase['branch_id']
                 and matches[0]['xid'] == phase['xid'] and matches[0]['branch_id'] == phase['branch_id']
                 and matches[0]['result'] == 1 and matches[0]['status'] == status, 'Original callback pair differs')
        args = json.loads((case / 'client-command.json').read_bytes())['argv']
        classes = Path(args[args.index('-cp') + 1].split(';')[0])
        for compiled in (out / 'classes').rglob('*.class'):
            need(sha(compiled) == sha(classes / compiled.relative_to(out / 'classes')), 'Executed class drift')
        lines = (case / 'class-load.log').read_text(encoding='utf-8').splitlines()
        need(not any(' org.mockito.' in line for line in lines), 'Mock runtime loaded')
        expected = [
            ('io.seatarust.compat.mysql.JavaAtWriterAdmission', classes.as_posix()),
            ('org.apache.seata.rm.datasource.DataSourceProxy', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.rm.datasource.ConnectionProxy', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.tm.api.DefaultGlobalTransaction', 'seata-tm-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('com.mysql.cj.jdbc.ConnectionImpl', 'mysql-connector-java-8.0.27.jar'),
            ('com.fasterxml.jackson.databind.ObjectMapper', 'jackson-databind-2.18.3.jar')]
        if profile >= 5: expected.append(('io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource', classes.as_posix()))
        for symbol, origin in expected:
            need(any(' ' + symbol + ' source: ' in line and origin in line.replace('file:/D:', 'D:') for line in lines),
                 'Unexpected class origin: ' + symbol)
    print('JAVA_WRITER_PROFILE_TC_VERIFIED ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); parser.add_argument('--full', action='store_true')
    args = parser.parse_args(); verify(args.packet, args.full)
