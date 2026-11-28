"""Independent initial SQL matrix audit: data, Undo, exact locks, original frames and Java parser."""
import argparse
import hashlib
import json
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, module, need, reg, sha


def normalize(value):
    if isinstance(value, dict): return {k: normalize(v) for k, v in value.items() if k != '@class'}
    if isinstance(value, list):
        if len(value) == 2 and isinstance(value[0], str) and value[0].startswith('java.util.') and isinstance(value[1], list): return normalize(value[1])
        return [normalize(v) for v in value]
    return value


def verify(out, full):
    names = set()
    for row in json.loads((out / 'catalog.json').read_bytes())['members']:
        name = row['path']; need(not Path(name).is_absolute() and '..' not in Path(name).parts and name not in names, 'Invalid member')
        names.add(name); need(sha(out / name) == row['sha256'], 'Catalog member differs')
    need(names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'}, 'Catalog coverage differs')
    result = json.loads((out / 'result.json').read_bytes()); versions, directions = result['versions'], result['directions']
    if full: need(versions == ['1.99', '1.85'] and directions == ['commit', 'rollback'], 'Full matrix missing')
    need(result['status'] == 'passed' and result['accepted'] == 13 and result['rejected'] == 0
         and result['execution_rejected'] == 3 and result['globals'] == 13 * len(versions) * len(directions)
         and result['mocks'] is False and result['tc_sha256'] == reg.JAR_SHA, 'Native scope/count differs')
    create, = [c for c in json.loads((out / 'commands.json').read_bytes()) if c['label'] == 'create']
    need(create['argv'][-2:] == ['--auto-increment-increment=7','--auto-increment-offset=3'], 'Non-unit allocation fixture differs')
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Executed source differs')
    for row in json.loads((out / 'dependencies.json').read_bytes()): need(sha(Path(row['path'])) == row['sha256'], 'Java dependency differs')
    matrix = json.loads((ROOT / 'crates/seata_db_mysql/tests/data/mysql_auto_increment.json').read_bytes())
    cases = {c['id']: c for c in matrix['accepted']}; need(len(cases) == 13 and len(matrix['rejected']) == 3, 'Matrix asset differs')
    binaries = json.loads((out / 'rust-binaries.json').read_bytes()); commands = json.loads((out / 'commands.json').read_bytes())
    need(all(c['exit_code'] == 0 for c in commands), 'Native command failed')
    for version in versions:
        need(sha(Path(binaries[version]['path'])) == binaries[version]['sha256'], 'Rust binary differs')
        need(('rustc ' + version + '.0').encode() in (out / ('rustc-' + version + '.stdout')).read_bytes(), 'Rust version differs')
        need(b'test result: ok. 1 passed;' in (out / ('classify-' + version + '.stdout')).read_bytes(), 'Classifier did not execute')
        need(any(c['label'] == 'clippy-' + version and c['argv'][-2:] == ['-D', 'warnings'] for c in commands), 'Strict Clippy missing')
    metadata = json.loads((out / 'metadata-binaries.json').read_bytes())
    for version in versions:
        need(sha(Path(metadata[version]['path'])) == metadata[version]['sha256'], 'Metadata binary differs')
        need(b'test result: ok. 1 passed;' in (out / ('metadata-' + version + '.stdout')).read_bytes(), 'Bidirectional physical attribute race did not execute')
    observations = json.loads((out / 'observations.json').read_bytes())
    need(len(observations) == result['globals'] and {(o['version'], o['direction'], o['id']) for o in observations}
         == {(v, d, id) for v in versions for d in directions for id in cases}, 'Observed coverage differs')
    oracle = json.loads((out / 'java-oracle.json').read_bytes())
    need(oracle['jackson'] == '2.18.3' and len(oracle['cases']) == len(observations), 'Original Java parser coverage differs')
    decoder = module('initial_sql_matrix_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    def decode(row):
        if row['payload_type'] != 111: return decoder.decode(row)
        # 原 TC 的定期单向清理指令；独立读原帧，不把它误算成分支业务或改写报文。
        frame = bytes.fromhex(row['frame_hex']); head = int.from_bytes(frame[7:9], 'big')
        need(frame[:3] == b'\xda\xda\x01' and int.from_bytes(frame[3:7], 'big') == len(frame)
             and frame[10:12] == b'\x01\x00' and head == 16 and frame[9] == row['kind'] == 2
             and row['direction'] == 'tc-to-sdk' and hashlib.sha256(frame).hexdigest() == row['sha256']
             and int.from_bytes(frame[12:16], 'big', signed=True) == row['id']
             and int.from_bytes(frame[head:head+2], 'big') == 111, 'Invalid original cleanup envelope')
        reader = decoder.Reader(frame[head+2:]); branch_type = reader.integer(1); resource = reader.string(); days = reader.integer(2, True); reader.end()
        need(branch_type == 0 and days == 7 and resource.startswith('jdbc:mysql://127.0.0.1:'), 'Original cleanup fields differ')
        return dict(code=111, resource=resource, days=days)
    xids = set(); completed = set()
    for observation in observations:
        version, direction, id = observation['version'], observation['direction'], observation['id']
        case = cases[id]; label = version + '-' + direction; prefix = label + '-' + id; folder = out / label
        phase = json.loads((folder / (id + '.held.json')).read_bytes()); xid, branch = phase['xid'], phase['branch_id']
        need(phase['last_insert_id'] == case['last_insert_id'] and phase['affected_rows'] == case['affected'], 'Original generated receipt differs')
        need(phase == observation['phase'] and xid not in xids and phase['checksum_persisted'] is True, 'Phase identity differs'); xids.add(xid)
        command = json.loads((folder / 'command.json').read_bytes())
        need(command['argv'][0] == binaries[version]['path'] and command['sha256'] == binaries[version]['sha256'], 'Executed binary differs')
        need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Native test missing')
        rejected = json.loads((folder / 'execution-rejected.json').read_bytes())
        need(len(rejected) == 3 and {r['id'] for r in rejected} == {r['id'] for r in matrix['rejected']} and all(r['inserts_before'] == r['inserts_after'] and r['inserts_before'] > 0 and r['rows'] == [['1',10]] for r in rejected), 'Generated batch/composite rejection differs')
        rows = [json.loads(line) for line in (out / (prefix + '-business.stdout')).read_text(encoding='utf-8').splitlines()]
        need(rows == case['expected'] == phase['phase_one'], 'Independent phase one differs')
        before = {r[0]: r for r in case['seed']}; after = {r[0]: r for r in case['expected']}
        changed = {k for k in before.keys() | after.keys() if before.get(k) != after.get(k)}
        lock = phase['lock']; need(lock.startswith('auto_items:') and set(lock.split(':', 1)[1].split(',')) == changed, 'Exact lock keys differ')
        tc_rows = [line.split('\t') for line in (out / (prefix + '-tc.stdout')).read_text(encoding='utf-8').splitlines()]
        need(tc_rows[0] == [xid, str(branch), phase['resource'], '2'] and len(tc_rows) == len(changed) + 1
             and all(r[:3] == [xid, str(branch), 'auto_items'] for r in tc_rows[1:])
             and {r[3] for r in tc_rows[1:]} == changed, 'Independent TC rows differ')
        undo = (folder / (id + '.undo.json')).read_bytes(); digest = hashlib.sha256(undo).hexdigest()
        raw = (out / (prefix + '-undo.stdout')).read_text(encoding='utf-8').strip().split('\t')
        need(bytes.fromhex(raw[0]) == undo and raw[1].lower() == digest and raw[2:] == ['1', '1'], 'Atomic Undo SHA differs')
        graph = json.loads(undo); logs = graph['sqlUndoLogs'][1]
        need(graph['xid'] == xid and graph['branchId'] == branch and len(logs) == 1, 'Original Undo identity differs')
        log = logs[0]; kind = case['sql'].split()[0].upper()
        need(log['sqlType'] == kind and log['tableName'] == 'auto_items', 'Original SQL Undo kind differs')
        for image, expected in [('beforeImage', [] if kind == 'INSERT' else [before[k] for k in sorted(changed)]),
                                ('afterImage', [] if kind == 'DELETE' else [after[k] for k in sorted(changed)])]:
            decoded = []
            for row in normalize(log[image])['rows']:
                fields = {f['name']: f for f in row['fields']}
                need(set(fields) == {'id','v'} and fields['id']['type'] == -5 and fields['v']['type'] == 4
                     and fields['id']['keyType'] == 'PRIMARY_KEY', 'Original native JDBC primary differs')
                def integer(value):
                    if isinstance(value,list) and len(value)==2 and value[0] in {'java.math.BigInteger','java.lang.Long','java.lang.Integer'}: value=value[1]
                    need(isinstance(value,(int,str)) and not isinstance(value,bool), 'Integer field type differs')
                    return int(value)
                decoded.append([str(integer(fields['id']['value'])),integer(fields['v']['value'])])
            need(sorted(decoded, key=lambda r: r[0]) == expected, 'Original before/after values differ')
        java_path = folder / (id + '.undo.json.java.json')
        need(normalize(graph) == normalize(json.loads(java_path.read_bytes())), 'Java parser changed semantic fields')
        java_observation, = [r for r in oracle['cases'] if r['path'] == label + '/' + id + '.undo.json']
        need(java_observation['xid'] == xid and java_observation['branch_id'] == branch and java_observation['logs'] == 1, 'Original Java read differs')
        terminal = [c for c in commands if c['label'].startswith(prefix + '-terminal')]
        need(terminal and (out / (terminal[-1]['label'] + '.stdout')).read_bytes().strip() == b'0\t0\t0\t0', 'TC/normal Undo remains')
        final = [json.loads(line) for line in (out / (prefix + '-final.stdout')).read_text(encoding='utf-8').splitlines()]
        need(final == observation['final'] == (case['expected'] if direction == 'commit' else case['seed']), 'Final business differs')
        recovery = (out / (prefix + '-recovery.stdout')).read_text(encoding='utf-8').strip().split('\t')
        need(recovery[:4] == (['1','0','1','1'] if direction == 'commit' else ['2','1','0','1']) and recovery[4].lower() == digest, 'Recovery history/checksum differs')
        completed.add((xid, branch))
        fences = [r.split('\t') for r in (out / (prefix + '-fences.stdout')).read_text(encoding='utf-8').splitlines()]
        need(['\t'.join(r) for r in fences] == observation['fences'] and all(r[2] == '1' and (r[0], int(r[1])) in completed for r in fences), 'Unexpected Undo/fence')
        frames = [(r, decode(r)) for r in json.loads((folder / 'proxy-events.json').read_bytes()) if r['event'] == 'wire']
        proxy = json.loads((folder / 'proxy-result.json').read_bytes()); need(not proxy['errors'] and not proxy['field_translation'] and not proxy['fault_injection'], 'Proxy changed execution')
        registrations = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == 11 and v['xid'] == xid]
        need(len(registrations) == 1 and registrations[0]['lock_key'] == lock and registrations[0]['resource'] == phase['resource'], 'Original registration differs')
        def reply(request, code):
            matches = [v for r, v in frames if r['direction'] == 'tc-to-sdk' and v['code'] == code
                       and v['connection'] == request['connection'] and v['id'] == request['id']]
            need(len(matches) == 1, 'Native response pairing differs'); return matches[0]
        registered = reply(registrations[0], 12)
        need(registered['result'] == 1 and registered['branch_id'] == branch, 'Original register response differs')
        reports = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == 13 and v['xid'] == xid]
        need(len(reports) == 1 and reports[0]['branch_id'] == branch and reports[0]['status'] == 2
             and reply(reports[0], 14)['result'] == 1, 'Original phase-one report differs')
        decisions = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == (7 if direction == 'commit' else 9) and v['xid'] == xid]
        need(len(decisions) == 1, 'Original TM decision missing')
        decided = reply(decisions[0], 8 if direction == 'commit' else 10)
        need(decided['result'] == 1 and decided['status'] in ({8, 9} if direction == 'commit' else {11}), 'Original TM decision response differs')
        code, reply_code, status = (3, 4, 5) if direction == 'commit' else (5, 6, 8)
        callbacks = [v for r, v in frames if r['direction'] == 'tc-to-sdk' and v['code'] == code and v['xid'] == xid]
        need(callbacks, 'Original callback missing')
        for request in callbacks:
            replies = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == reply_code and v['connection'] == request['connection'] and v['id'] == request['id']]
            need(len(replies) == 1 and request['branch_id'] == branch and replies[0]['xid'] == xid and replies[0]['branch_id'] == branch
                 and replies[0]['result'] == 1 and replies[0]['status'] == status, 'Original callback pairing/status differs')
    lines = (out / 'oracle-class-load.log').read_text(encoding='utf-8').splitlines()
    oracle_command, = [c for c in commands if c['label'] == 'java-oracle']
    classes = Path(oracle_command['argv'][oracle_command['argv'].index('-cp') + 1].split(';')[0])
    for compiled in (out / 'classes').rglob('*.class'):
        need(sha(compiled) == sha(classes / compiled.relative_to(out / 'classes')), 'Executed oracle class differs')
    need(any(' SqlMatrixUndoOracle source: ' in line and classes.as_posix() in line.replace('file:/D:', 'D:') for line in lines), 'Fresh oracle origin differs')
    for symbol, origin in [('org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
                            ('com.fasterxml.jackson.databind.ObjectMapper', 'jackson-databind-2.18.3.jar')]:
        need(any(' ' + symbol + ' source: ' in line and origin in line for line in lines), 'Original Java class origin differs')
    print('AUTO_INCREMENT_VERIFIED globals=' + str(len(xids)) + ' ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); parser.add_argument('--full', action='store_true')
    args = parser.parse_args(); verify(args.packet, args.full)
