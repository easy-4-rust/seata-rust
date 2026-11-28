"""Re-read original bidirectional frames, exact SQL state, Undo SHA and Java compensation."""
import argparse
import hashlib
import json
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, module, need, reg, sha


def verify(out, full):
    names = set()
    for row in json.loads((out / 'catalog.json').read_bytes())['members']:
        name = row['path']; need(not Path(name).is_absolute() and '..' not in Path(name).parts and name not in names, 'Invalid member')
        names.add(name); need(sha(out / name) == row['sha256'], 'Catalog member drift')
    need(names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'}, 'Catalog coverage differs')
    result = json.loads((out / 'result.json').read_bytes())
    need(result['status'] == 'passed' and result['mocks'] is False and result['tc_sha256'] == reg.JAR_SHA, 'Original TC status differs')
    versions, profiles, directions = result['versions'], result['profiles'], result['directions']
    if full: need(versions == ['1.99', '1.85'] and profiles == [5, 6] and directions == ['commit', 'rollback'], 'Full matrix missing')
    expected_cases = len(versions) * len(profiles) * len(directions)
    need(result['cases'] == expected_cases and result['java_rejected'] == result['java_admitted'] == expected_cases * 2, 'Coverage counts differ')
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Executed source drift')
    for row in json.loads((out / 'dependencies.json').read_bytes()):
        need(sha(Path(row['path'])) == row['sha256'], 'Executed Java dependency drift')
    binaries = json.loads((out / 'rust-binaries.json').read_bytes())
    commands = json.loads((out / 'commands.json').read_bytes())
    need(all(c['exit_code'] == 0 for c in commands), 'A native command failed')
    for version in versions:
        need(sha(Path(binaries[version]['path'])) == binaries[version]['sha256'], 'Native Rust executable differs')
        need(('rustc ' + version + '.0').encode() in (out / ('rustc-' + version + '.stdout')).read_bytes(), 'Toolchain differs')
        need(any(c['label'] == 'clippy-' + version and c['argv'][-2:] == ['-D', 'warnings'] for c in commands), 'Strict Clippy missing')
    decoder = module('reverse_temporal_native_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    observations = json.loads((out / 'observations.json').read_bytes())
    need(len(observations) == expected_cases and {(o['version'], o['profile'], o['direction']) for o in observations}
         == {(v, p, d) for v in versions for p in profiles for d in directions}, 'Observed matrix differs')
    all_xids = set(); prior_fences = set()
    def fence_rows(label, allowed):
        rows = [r.split('\t') for r in (out / (label + '.stdout')).read_text().splitlines()]
        actual = {(r[0], int(r[1])) for r in rows}
        need(len(rows) == len(actual) and actual <= allowed and all(r[2] == '1' for r in rows), 'Unexpected fence/Undo rows')
        return actual
    for observation in observations:
        version, profile, direction = observation['version'], observation['profile'], observation['direction']
        label = version + '-' + str(profile) + '-' + direction; case = out / label
        if prior_fences:
            need(fence_rows(label + '-prior-fences', prior_fences) == prior_fences
                 and (out / (label + '-reset-fences.stdout')).read_bytes().strip() == str(len(prior_fences)).encode(),
                 'Prior fixture fence isolation differs')
        owner = json.loads((case / 'rust/owner-held.json').read_bytes())
        need(owner == observation['owner'] and owner['profile'] == profile and owner['checksum_persisted'] is True, 'Owner identity differs')
        owner_xid, owner_branch, resource = owner['xid'], owner['branch_id'], owner['resource']
        completed_branches = {(owner_xid, owner_branch)}
        need(owner_xid not in all_xids, 'Owner XID reused'); all_xids.add(owner_xid)
        expected_keys = {('temporal_dates', '1582-10-05'), ('temporal_times', '-25:01:02.123456')}
        need({tuple(k.split(':', 1)) for k in owner['lock'].split(';')} == expected_keys, 'Rust exact lock spelling differs')
        before = (out / (label + '-before.stdout')).read_bytes(); held = (out / (label + '-held-business.stdout')).read_bytes()
        need(before == b'1582-10-05\t10\n-25:01:02.123456\t20\n'
             and held == b'1582-10-05\t20\n-25:01:02.123456\t30\n', 'Rust actual phase one differs')
        lock_rows = [r.split('\t') for r in (out / (label + '-held-tc.stdout')).read_text().splitlines()]
        need(lock_rows[0] == [owner_xid, str(owner_branch), resource, '2'] and len(lock_rows) == 3
             and all(r[:2] == [owner_xid, str(owner_branch)] for r in lock_rows[1:])
             and {tuple(r[2:]) for r in lock_rows[1:]} == expected_keys, 'Original TC owner row differs')
        raw = (out / (label + '-held-undo.stdout')).read_text().strip().split('\t')
        undo = (case / 'rust-original-undo.json').read_bytes()
        need(undo == bytes.fromhex(raw[0]) and hashlib.sha256(undo).hexdigest() == raw[1].lower()
             and raw[2:] == [str(profile), '1', '0'], 'Undo SHA/profile was not persisted with phase one')
        graph = json.loads(undo)
        need(graph['xid'] == owner_xid and graph['branchId'] == owner_branch and len(graph['sqlUndoLogs'][1]) == 2, 'Rust original Undo identity differs')
        for suffix in ['business', 'tc', 'undo']:
            need((out / (label + '-rejected-' + suffix + '.stdout')).read_bytes()
                 == (out / (label + '-held-' + suffix + '.stdout')).read_bytes(), 'Rejected Java changed owner ' + suffix)
        terminal = [c for c in commands if c['label'].startswith(label + '-owner-terminal')]
        need(terminal and (out / (terminal[-1]['label'] + '.stdout')).read_bytes().strip() == b'0\t0\t0\t0', 'Rust owner did not clear TC/Undo')
        fence_rows(label + '-owner-fences', completed_branches)
        outcome = (out / (label + '-owner-business.stdout')).read_bytes()
        need(outcome == (held if direction == 'commit' else before), 'Rust actual completion differs')
        completed = json.loads((case / 'rust/owner-finished.json').read_bytes())
        need(completed == dict(xid=owner_xid, branch_id=owner_branch, state=[1, 0, 1, 1] if direction == 'commit' else [2, 1, 0, 1]), 'Rust recovery terminal state differs')
        rows = json.loads((case / 'proxy-events.json').read_bytes())
        proxy = json.loads((case / 'proxy-result.json').read_bytes())
        need(not proxy['errors'] and not proxy['field_translation'] and not proxy['fault_injection'], 'Proxy changed native execution')
        frames = [(r, decoder.decode(r)) for r in rows if r['event'] == 'wire']

        def requests(code, xid, direction='sdk-to-tc'):
            return [v for r, v in frames if r['direction'] == direction and v['code'] == code and v.get('xid') == xid]

        def response(request, code, direction='tc-to-sdk'):
            matches = [v for r, v in frames if r['direction'] == direction and v['code'] == code
                       and v['connection'] == request['connection'] and v['id'] == request['id']]
            need(len(matches) == 1, 'Native request/reply pairing differs'); return matches[0]

        registered, = requests(11, owner_xid); registered_reply = response(registered, 12)
        need(registered['version'] == 1 and registered['resource'] == resource and registered['lock_key'] == owner['lock']
             and registered_reply['result'] == 1 and registered_reply['branch_id'] == owner_branch, 'Rust owner original registration differs')
        report, = requests(13, owner_xid)
        need(report['branch_id'] == owner_branch and report['status'] == 2 and response(report, 14)['result'] == 1, 'Original phase-one report missing')
        callback_code, reply_code, status = (3, 4, 5) if direction == 'commit' else (5, 6, 8)
        callbacks = requests(callback_code, owner_xid, 'tc-to-sdk'); need(callbacks, 'Owner callback missing')
        for callback in callbacks:
            reply = response(callback, reply_code, 'sdk-to-tc')
            need(callback['branch_id'] == owner_branch and callback['resource'] == resource
                 and reply['xid'] == owner_xid and reply['branch_id'] == owner_branch and reply['result'] == 1 and reply['status'] == status,
                 'Original Rust phase-two callback differs')
        rejected = json.loads((case / 'java/rejected.json').read_bytes())
        need(len(rejected) == 2 and {r['case'] for r in rejected} == {'date', 'time'}, 'Java conflict coverage differs')
        for name, table, key in [('date', 'temporal_dates', '1582-10-05'), ('time', 'temporal_times', '-25:01:02.123456')]:
            conflict, = [r for r in rejected if r['case'] == name]
            xid = conflict['xid']; need(xid not in all_xids, 'Java conflict XID reused'); all_xids.add(xid)
            request, = requests(11, xid); rejected_reply = response(request, 12)
            need(request['version'] == 2 and request['resource'] == resource and request['lock_key'] == table + ':' + key
                 and conflict['lock'] == request['lock_key'] and rejected_reply['result'] == 0
                 and rejected_reply['error'] == {'LockKeyConflict': 2, 'LockKeyConflictFailFast': 20}[conflict['exception']],
                 'Java did not receive an original TC lock rejection')
            admitted = json.loads((case / ('java/admitted-' + name + '.json')).read_bytes())
            xid, branch = admitted['xid'], admitted['branch_id']; need(xid not in all_xids, 'Java admitted XID reused'); all_xids.add(xid)
            completed_branches.add((xid, branch))
            request, = requests(11, xid); accepted = response(request, 12)
            need(request['version'] == 2 and request['resource'] == resource and request['lock_key'] == table + ':' + key
                 and accepted['result'] == 1 and accepted['branch_id'] == branch, 'Java admission differs')
            tc_rows = (out / (label + '-admitted-' + name + '-tc.stdout')).read_text().splitlines()
            need(tc_rows == ['\t'.join([xid, str(branch), resource, '1']), '\t'.join([xid, str(branch), table, key])], 'Independent Java lock rows differ')
            data = (out / (label + '-admitted-' + name + '-business.stdout')).read_text().splitlines()
            expected = [r.split('\t') for r in outcome.decode().splitlines()]
            index = 0 if name == 'date' else 1; expected[index][1] = str(int(expected[index][1]) + 1)
            need(data == ['\t'.join(r) for r in expected], 'Java phase one business differs')
            java_undo = (case / ('java/undo-' + name + '.json')).read_bytes(); graph = json.loads(java_undo)
            need(java_undo == bytes.fromhex(admitted['undo_hex']) and graph['xid'] == xid and graph['branchId'] == branch
                 and len(graph['sqlUndoLogs'][1]) == 1, 'Java original Undo differs')
            log = graph['sqlUndoLogs'][1][0]
            for image in ['beforeImage', 'afterImage']:
                fields = log[image]['rows'][1][0]['fields'][1]
                pk, = [f for f in fields if f['name'] == 'k']
                need(pk['type'] == (91 if name == 'date' else 92) and pk['value'] == key, 'Java exact Undo PK type/value differs')
            rollbacks = requests(5, xid, 'tc-to-sdk'); need(rollbacks, 'Java rollback missing')
            for rollback in rollbacks:
                rolled = response(rollback, 6, 'sdk-to-tc')
                need(rollback['branch_id'] == branch and rolled['xid'] == xid and rolled['branch_id'] == branch
                     and rolled['result'] == 1 and rolled['status'] == 8, 'Original Java compensation callback differs')
            terminal = [c for c in commands if c['label'].startswith(label + '-' + name + '-terminal')]
            need(terminal and (out / (terminal[-1]['label'] + '.stdout')).read_bytes().strip() == b'0\t0\t0\t0'
                 and (out / (label + '-' + name + '-final.stdout')).read_bytes() == outcome, 'Java compensation/terminal differs')
            fence_rows(label + '-' + name + '-fences', completed_branches)
        prior_fences = fence_rows(label + '-final-fences', completed_branches)
        need(prior_fences == {(r['xid'], r['branch_id']) for r in observation['terminal_fences']}, 'Final fence identities differ')
        need(b'test result: ok. 1 passed;' in (case / 'rust/client-stdout.log').read_bytes(), 'Rust live test missing')
        command = json.loads((case / 'rust/command.json').read_bytes())
        need(command['sha256'] == binaries[version]['sha256'] and command['argv'][0] == binaries[version]['path'], 'Executed Rust binary differs')
        args = json.loads((case / 'java/command.json').read_bytes())['argv']; classes = Path(args[args.index('-cp') + 1].split(';')[0])
        for compiled in (out / 'classes').rglob('*.class'): need(sha(compiled) == sha(classes / compiled.relative_to(out / 'classes')), 'Executed Java class drift')
        lines = (case / 'java/class-load.log').read_text().splitlines()
        need(not any(' org.mockito.' in line for line in lines), 'Mock runtime loaded')
        for symbol, origin in [
            ('io.seatarust.compat.mysql.JavaAtWriterAdmission', classes.as_posix()),
            ('io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource', classes.as_posix()),
            ('org.apache.seata.rm.datasource.DataSourceProxy', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.rm.datasource.ConnectionProxy', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.tm.api.DefaultGlobalTransaction', 'seata-tm-2.8.0-SNAPSHOT.jar'),
            ('org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager', 'seata-rm-datasource-2.8.0-SNAPSHOT.jar'),
            ('com.mysql.cj.jdbc.ConnectionImpl', 'mysql-connector-java-8.0.27.jar'),
            ('com.fasterxml.jackson.databind.ObjectMapper', 'jackson-databind-2.18.3.jar')]:
            need(any(' ' + symbol + ' source: ' in line and origin in line.replace('file:/D:', 'D:') for line in lines), 'Unexpected Java class origin')
    need(len(all_xids) == expected_cases * 5, 'Native global transaction count differs')
    print('REVERSE_TEMPORAL_VERIFIED cases=' + str(expected_cases) + ' globals=' + str(len(all_xids)) + ' ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); parser.add_argument('--full', action='store_true')
    args = parser.parse_args(); verify(args.packet, args.full)
