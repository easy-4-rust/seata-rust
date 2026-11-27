"""Independently bind original TC retry evidence to wire identity and physical XA outcome."""
import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog, sources
from validate_xa_managed import load, string, only
from mysql_undo_executor_live import IMAGE


def frames(path):
    events = [json.loads(line) for line in path.read_bytes().splitlines()]
    result = []
    for row in events:
        if row['event'] != 'frame':
            continue
        raw = bytes.fromhex(row['frame_hex'])
        header = int.from_bytes(raw[7:9], 'big')
        need(raw[:3] == b'\xda\xda\x01' and int.from_bytes(raw[3:7], 'big') == len(raw) and 16 <= header < len(raw), 'Original V1 envelope changed')
        body = raw[header:]
        need(int.from_bytes(body[:2], 'big') == row['kind'] and int.from_bytes(raw[12:16], 'big', signed=True) == row['request_id'], 'Original message correlation changed')
        row['body'] = body
        result.append(row)
    summary = [row for row in events if row['event'] == 'summary']
    need(len(summary) == 1 and summary[0]['succeeded'] and not summary[0]['errors'], 'Observer not complete')
    return result, summary[0], events


def check_case(folder, prefix, mode, direction):
    case = folder/prefix
    proof = load(case/'observations/proof/original-java-xa-fault.json')
    commit = direction == 'commit'
    database = mode in ['restart', 'crash']
    expected = 'Committed' if commit else 'Rollbacked'
    need(proof['mode'] == mode and proof['expected_tc_terminal'] == expected and proof['initial_global_status'] in [None, expected], 'Original decision differs')
    need(proof['journal'] == 'Completed' and proof['prepared_remaining'] == proof['phase_one_reports'] == 0 and proof['client_end_requests'] == 1 and proof['finished_inferred_as_success'] is False, 'Uncertain reply incorrectly treated as success')
    need(proof['business'] == ([[1, 42]] if commit else []), 'Business proof differs')
    need(load(case/'client-command.json')['exit_code'] == 0 and load(case/'process-cleanup.json')['stopped'], 'Actual client/cleanup failed')
    rm, summary, events = frames(case/'RM-frames.jsonl')
    tm, tm_summary, _ = frames(case/'TM-frames.jsonl')
    wire = [(r['kind'], r['body'], r['direction']) for r in rm]
    need(not any(r['kind'] == 13 for r in rm), 'XA sent AT phase-one report')
    register = only(wire, 11, 'c2s')
    xid, pos = string(register, 2)
    need(xid == proof['xid'] and register[pos] == 3, 'Original XA registration identity changed')
    resource, pos = string(register, pos+1)
    lock, pos = string(register, pos, 4)
    app, pos = string(register, pos, 4)
    need(resource == proof['resource_id'] and lock is app is None and pos == len(register), 'Registration optional fields changed')
    response = only(wire, 12, 's2c')
    branch = int.from_bytes(response[4:12], 'big', signed=True)
    need(response[2:4] == b'\x01\x00' and str(branch) == proof['branch_id'], 'Original TC branch differs')
    callback_kind, reply_kind = (3, 4) if commit else (5, 6)
    callbacks = [r for r in rm if r['kind'] == callback_kind]
    replies = [r for r in rm if r['kind'] == reply_kind]
    need(callbacks and (len(replies) == len(callbacks) and 1 <= len(callbacks) <= 16 if database else len(replies) == 2) and all(r['direction'] == 's2c' for r in callbacks), 'Fault callback/reply count differs')
    for row in callbacks:
        body = row['body']
        value, pos = string(body, 2)
        need(value == xid and int.from_bytes(body[pos:pos+8], 'big', signed=True) == branch and body[pos+8] == 3, 'Retry changed original XA identity')
        value, pos = string(body, pos+9)
        application, pos = string(body, pos, 4)
        need(value == resource and application is None and pos == len(body), 'Retry changed original XA resource')
    for row in replies:
        body = row['body']
        value, pos = string(body, 4)
        need(row['direction'] == 'c2s' and body[2:4] == b'\x01\x00' and value == xid and int.from_bytes(body[pos:pos+8], 'big', signed=True) == branch and body[pos+8:] == bytes([5 if commit else 8]), 'Original terminal reply changed')
        need(any((r['connection'], r['request_id']) == (row['connection'], row['request_id']) for r in callbacks), 'Reply not correlated with callback')
    need(summary['mode'] == ('basic' if database else mode) and summary['faults'] == (0 if database else 1), 'Fault not injected exactly once')
    if database:
        callback_keys = {(r['connection'], r['request_id']) for r in callbacks}
        reply_keys = {(r['connection'], r['request_id']) for r in replies}
        need(len(callback_keys) == len(callbacks) and callback_keys == reply_keys and len(reply_keys) == len(replies) and all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in rm), 'Database recovery changed callback delivery/correlation')
        checkpoint = load(case/'observations/proof/before-database-restart.json')
        need(checkpoint == dict(mode=mode, xid=xid, branch_id=str(branch), resource_id=resource, rust_process_id=proof['rust_process_id']), 'Database checkpoint changed original identity')
        process_ids = load(case/'process-cleanup.json')['pids']
        need(len(process_ids) == len(set(process_ids)) == 4 and process_ids[-1] == checkpoint['rust_process_id'], 'Checkpoint not from actual owned client')
        need((case/'observations/proof/database-resumed').read_bytes() == b'resume-original-branch\n', 'Original client not resumed')
        def snapshot(name):
            return (folder/(prefix+'-'+name+'.stdout')).read_bytes().strip()
        before = snapshot('prepared-before-restart')
        suffix = (xid+'-'+str(branch)).encode()
        need(before == snapshot('prepared-after-restart') and before.split(b'\t') == [b'9752', str(len(xid.encode())).encode(), str(len(('-'+str(branch)).encode())).encode(), b'0x'+suffix.hex().upper().encode()], 'Prepared identity did not survive database recovery')
        need(snapshot('uuid-before-restart') == snapshot('uuid-after-restart') and snapshot('uuid-before-restart'), 'Original database instance changed')
        need(snapshot('tc-before-restart') == snapshot('tc-after-restart') == (xid+'\t'+str(branch)+'\n'+xid).encode(), 'Original TC registration changed over restart')
        before_owner = json.loads(snapshot('owned-before-restart'))[0]
        stopped_owner = json.loads(snapshot('owned-stopped'))[0]
        need(before_owner['Id'] == stopped_owner['Id'] and before_owner['Image'] == stopped_owner['Image'] == IMAGE and before_owner['State']['Running'] and not stopped_owner['State']['Running'] and before_owner['Config']['Labels']['seata.xa.owner'] == stopped_owner['Config']['Labels']['seata.xa.owner'] and before_owner['Config']['Labels']['seata.xa.owner'], 'Owned database did not exit')
        if mode == 'crash': need(stopped_owner['State']['ExitCode'] == 137, 'Database crash not forced termination')
        commands = load(folder/'commands.json')
        stop = [r for r in commands if r['label'] == prefix+'-stop-for-restart']
        need(len(stop) == 1 and stop[0]['exit_code'] == 0 and stop[0]['argv'] == (['docker','kill','--signal','KILL',before_owner['Id']] if mode == 'crash' else ['docker','stop','--time','20',before_owner['Id']]), 'Database lifecycle command changed')
    elif mode == 'duplicate':
        need(len(callbacks) == 1 and callbacks[0]['action'] == 'duplicate_original' and callbacks[0]['delivery_count'] == 2, 'Original callback not duplicated exactly')
        need([r['action'] for r in replies] == ['hold', 'release_original'] and [r['delivery_count'] for r in replies] == [0, 1], 'Duplicate responses not held/released exactly')
        need(replies[0]['frame_hex'] == replies[1]['frame_hex'] == replies[1]['released_frame_hex'] and summary['released'] == 1 and summary['closed'] == 0, 'Released reply not original bytes')
    else:
        need(len(callbacks) == 2 and callbacks[0]['body'] == callbacks[1]['body'] and callbacks[0]['connection'] < callbacks[1]['connection'], 'Original callback not retried after reconnect')
        need([r['action'] for r in replies] == ['drop_and_close', 'forward'] and [r['delivery_count'] for r in replies] == [0, 1], 'Original terminal loss not exercised')
        need(summary['closed'] == 1 and summary['connections'] >= 2 and len([r for r in events if r['event'] == 'fault_close']) == 1, 'Owned RM connection not closed')
        need(any(r['kind'] == 103 and r['connection'] == callbacks[1]['connection'] for r in rm) and any(r['kind'] == 104 and r['connection'] == callbacks[1]['connection'] and r['body'][2:4] == b'\x01\x00' for r in rm), 'RM did not register again')
    selected = callbacks + replies
    need(all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in rm if r not in selected), 'Unrelated RM message altered')
    need(tm_summary['mode'] == 'basic' and all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in tm), 'TM observer altered end decision')
    tm_wire = [(r['kind'], r['body'], r['direction']) for r in tm]
    only(tm_wire, 1, 'c2s')
    need(string(only(tm_wire, 7 if commit else 9, 'c2s'), 2)[0] == xid, 'End request changed identity or repeated')
    need(not any(r['kind'] in [15, 16] for r in tm), 'Status reconciliation used as outcome proof')
    terminal = load(case/'original-tc-terminal.json')
    branch_log = ('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)
    global_log = ('Committing global transaction is successfully done, xid = ' if commit else 'Rollback global transaction successfully, xid = ')+xid+'.'
    need(terminal == dict(xid=xid, branch_id=str(branch), direction=direction, original_branch_success_log=branch_log, original_global_success_log=global_log, Finished_used_as_success=False), 'Original TC outcome declaration changed')
    logs = (case/'tc-stdout.log').read_bytes()
    need(branch_log.encode() in logs and global_log.encode() in logs, 'Original TC did not log branch and global completion')
    def independent(suffix):
        return (folder/(prefix+'-'+suffix+'.stdout')).read_bytes().strip().replace(b'\r', b'')
    need(independent('independent-state') == (xid.encode().hex().upper()+'\t'+str(branch)+'\t'+('1' if commit else '2')+'\t1').encode(), 'Physical original terminal differs')
    need(independent('independent-business') == (b'1\t42' if commit else b'') and not independent('independent-prepared') and independent('independent-tc') == b'0\t0\t0', 'Physical business/prepared/TC cleanup differs')
    writes = [r for r in independent('native-commands').splitlines() if b'XA RECOVER' not in r]
    if database:
        writes = [r for r in independent('native-before-restart').splitlines() if b'XA RECOVER' not in r]+writes
    suffix = ("X'"+xid.encode().hex().upper()+"',X'"+('-'+str(branch)).encode().hex().upper()+"',9752").encode()
    need(writes == [b'Query\tXA '+verb+b' '+suffix for verb in [b'START', b'END', b'PREPARE', b'COMMIT' if commit else b'ROLLBACK']], 'Native XA body/end replayed or changed')


def verify(folder, digest, database=False):
    catalog(folder, digest)
    sources(folder)
    result = load(folder/'result.json')
    from xa_managed_java_faults import guard
    need(result['Java_jar_sha256'] == guard.reg.JAR_SHA, 'Original TC artifact changed')
    original = subprocess.check_output(['git', '-C', str(guard.reg.ORACLE), 'show', guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java'])
    need((folder/'original-DefaultCore.java').read_bytes() == original, 'Terminal log source not original pinned TC')
    need(result['Rust_versions'] == ['1.99', '1.85'] and result['modes'] == (['restart', 'crash'] if database else ['duplicate', 'reconnect']) and result['Java_pin'] == 'cba73e180fd8896a99df6415b859d275122feaa7' and result['whole_plan_complete'] is result['XA_full_fault_gate_complete'] is False, 'Fault scope or Java pin changed')
    for version in result['Rust_versions']:
        for mode in result['modes']:
            for direction in ['commit', 'rollback']:
                check_case(folder, version+'-'+mode+'-'+direction, mode, direction)
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned fixture container remains')


def negative_controls(folder, database=False):
    """Mutate semantic inputs without catalog checks, so hashes cannot mask weak assertions."""
    rejected = 0
    for version in ['1.99', '1.85']:
        for mode in (['restart', 'crash'] if database else ['duplicate', 'reconnect']):
            for direction in ['commit', 'rollback']:
                prefix = version+'-'+mode+'-'+direction
                with tempfile.TemporaryDirectory(prefix='seata-xa-proof-', dir='D:/Temp') as directory:
                    root = Path(directory)
                    case = root/prefix
                    shutil.copytree(folder/prefix, case)
                    for path in folder.glob(prefix+'-*.stdout'):
                        shutil.copyfile(path, root/path.name)
                    shutil.copyfile(folder/'commands.json', root/'commands.json')
                    proof_path = case/'observations/proof/original-java-xa-fault.json'
                    rm_path = case/'RM-frames.jsonl'
                    log_path = case/'tc-stdout.log'
                    sql_path = root/(prefix+'-native-commands.stdout')
                    tc_path = root/(prefix+'-independent-tc.stdout')
                    mutations = []
                    for key, value in [('finished_inferred_as_success', True), ('client_end_requests', 2), ('journal', 'Pending')]:
                        proof = load(proof_path)
                        proof[key] = value
                        mutations.append((proof_path, json.dumps(proof).encode()))
                    events = [json.loads(line) for line in rm_path.read_bytes().splitlines()]
                    for change in ['no_fault', 'wrong_reply', 'no_register', 'no_delivery']:
                        altered = json.loads(json.dumps(events))
                        if change == 'no_register':
                            altered = [r for r in altered if r.get('kind') != 11]
                        else:
                            row = next(r for r in altered if r['event'] == 'summary') if change == 'no_fault' else next(r for r in altered if r.get('kind') == (4 if direction == 'commit' else 6))
                            if change == 'no_fault': row['faults'] = 1 if database else 0
                            elif change == 'wrong_reply': row['frame_hex'] = row['frame_hex'][:-2]+'0a'
                            else: row['delivery_count'] = 0 if database else 1
                        mutations.append((rm_path, b''.join(json.dumps(r).encode()+b'\n' for r in altered)))
                    terminal = load(case/'original-tc-terminal.json')
                    mutations.extend([(log_path, log_path.read_bytes().replace(terminal['original_global_success_log'].encode(), b'Finished')),
                                      (sql_path, sql_path.read_bytes()+next(line for line in sql_path.read_bytes().splitlines(keepends=True) if b'XA RECOVER' not in line)),
                                      (tc_path, b'1\t0\t0\n')])
                    if database:
                        for name in ['prepared-after-restart', 'uuid-after-restart', 'tc-after-restart']:
                            mutations.append((root/(prefix+'-'+name+'.stdout'), b'changed-original\n'))
                        owner_path = root/(prefix+'-owned-stopped.stdout')
                        owner = load(owner_path)
                        owner[0]['State']['Running'] = True
                        mutations.append((owner_path, json.dumps(owner).encode()))
                    check_case(root, prefix, mode, direction)
                    for path, altered in mutations:
                        before = path.read_bytes()
                        try:
                            path.write_bytes(altered)
                            try: check_case(root, prefix, mode, direction)
                            except ValueError: rejected += 1
                            else: raise RuntimeError('Semantic weakening accepted: '+str(path))
                        finally: path.write_bytes(before)
    need(rejected == (112 if database else 80), 'Semantic weakening matrix incomplete')
    return rejected


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('folder', type=Path)
    parser.add_argument('catalog_sha256')
    parser.add_argument('--negative-controls', action='store_true')
    parser.add_argument('--database', action='store_true')
    args = parser.parse_args()
    verify(args.folder, args.catalog_sha256, args.database)
    if args.negative_controls:
        print('Rejected semantic weakenings:', negative_controls(args.folder, args.database))
    print('Verified eight original Java TC XA callback fault cases, wire identities and physical outcomes')
