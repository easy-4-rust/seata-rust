"""Verify sealed managed XA observations against original wire frames and independent SQL."""
import argparse
import json
from pathlib import Path
from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog, sources


def load(path):
    return json.loads(path.read_bytes())


def raw_frames(path):
    result = []
    for row in map(json.loads, path.read_bytes().splitlines()):
        if row['event'] != 'frame':
            continue
        raw = bytes.fromhex(row['frame_hex'])
        need(raw[:3] == b'\xda\xda\x01' and int.from_bytes(raw[3:7], 'big') == len(raw), 'Invalid original V1 frame')
        header = int.from_bytes(raw[7:9], 'big')
        body = raw[header:]
        kind = int.from_bytes(body[:2], 'big')
        need(kind == row['kind'] and row['action'] == 'forward' and row['delivery_count'] == 1, 'Observer metadata/delivery differs from wire')
        result.append((kind, body, row['direction']))
    return result


def string(body, pos, width=2):
    length = int.from_bytes(body[pos:pos+width], 'big', signed=True)
    pos += width
    if length in [-1, 0]:
        return None, pos
    need(length >= 0 and pos + length <= len(body), 'Malformed original string')
    return body[pos:pos+length].decode(), pos+length


def only(frames, kind, direction):
    rows = [body for code, body, route in frames if code == kind and route == direction]
    need(len(rows) == 1, 'Original wire message missing or repeated: '+str(kind))
    return rows[0]


def java(folder):
    catalog(folder, sha(folder/'catalog.json'))
    sources(folder)
    result = load(folder/'result.json')
    need(result['Rust_versions'] == ['1.99', '1.85'] and result['whole_plan_complete'] is False and result['XA_full_fault_gate_complete'] is False, 'Java scope changed')
    need(result['Java_pin'] == 'cba73e180fd8896a99df6415b859d275122feaa7', 'Original Java pin changed')
    for rust in result['Rust_versions']:
        for direction in ['commit', 'rollback']:
            prefix = rust+'-'+direction
            case = folder/prefix
            proof = load(case/'observations/proof/original-java-xa.json')
            need(load(case/'client-command.json')['exit_code'] == 0 and load(case/'process-cleanup.json')['stopped'], 'Actual Java client/cleanup failed')
            need(proof['journal'] == 'Completed' and proof['prepared_remaining'] == 0 and proof['phase_one_reports'] == 0, 'Java original branch not completed')
            expected = [[1, 42]] if direction == 'commit' else []
            need(proof['business'] == expected and proof['global_status'] == ('Committed' if direction == 'commit' else 'Rollbacked'), 'Original global result changed')
            frames = raw_frames(case/'RM-frames.jsonl')
            need(not any(code == 13 for code, _, _ in frames), 'XA emitted AT phase-one report')
            register = only(frames, 11, 'c2s')
            xid, pos = string(register, 2)
            need(xid == proof['xid'] and register[pos] == 3, 'Original XA registration identity/type changed')
            resource, pos = string(register, pos+1)
            lock, pos = string(register, pos, 4)
            app, pos = string(register, pos, 4)
            need(resource == proof['resource_id'] and lock is None and app is None and pos == len(register), 'XA registration optional fields changed')
            response = only(frames, 12, 's2c')
            branch = int.from_bytes(response[4:12], 'big', signed=True)
            need(response[2:4] == b'\x01\x00' and str(branch) == proof['branch_id'], 'Original registered branch differs')
            callback = only(frames, 3 if direction == 'commit' else 5, 's2c')
            callback_xid, pos = string(callback, 2)
            need(callback_xid == xid and int.from_bytes(callback[pos:pos+8], 'big', signed=True) == branch and callback[pos+8] == 3, 'Callback not bound to original XA branch')
            callback_resource, pos = string(callback, pos+9)
            callback_app, pos = string(callback, pos, 4)
            need(callback_resource == resource and callback_app is None and pos == len(callback), 'Callback resource differs')
            reply = only(frames, 4 if direction == 'commit' else 6, 'c2s')
            reply_xid, pos = string(reply, 4)
            need(reply[2:4] == b'\x01\x00' and reply_xid == xid and int.from_bytes(reply[pos:pos+8], 'big', signed=True) == branch and reply[pos+8] == (5 if direction == 'commit' else 8), 'Callback original completion differs')
            tm = raw_frames(case/'TM-frames.jsonl')
            end = only(tm, 7 if direction == 'commit' else 9, 'c2s')
            need(string(end, 2)[0] == xid, 'Global decision refers to different transaction')
            end_reply = only(tm, 8 if direction == 'commit' else 10, 's2c')
            need(end_reply[2:4] == b'\x01\x00' and end_reply[-1] == (9 if direction == 'commit' else 11), 'Original TC global status differs')
            need((folder/(prefix+'-independent-business.stdout')).read_bytes().strip().replace(b'\r', b'') == (b'1\t42' if expected else b''), 'Independent business differs')
            need((folder/(prefix+'-independent-tc.stdout')).read_bytes().strip() == b'0\t0\t0', 'Original TC rows remain')
            need(not (folder/(prefix+'-independent-prepared.stdout')).read_bytes().strip(), 'Original Prepared remains')
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned Java MySQL remains')


def witness_proof(proof):
    need(proof['intermediate_journal'] == 'Pending' and proof['journal'] == 'Completed' and proof['control_state'] == [1, 1] and proof['actual_commands'] == 1 and proof['callback_statuses'] == [6, 5], 'Lost original commit OK lifecycle changed')
    need(proof['witness_corruptions_rejected'] == 6 and proof['missing_witness'] == 'Unknown' and proof['fresh_manager_recovered'] is True and proof['witness_invisible_before_commit'] is True and proof['witness_extra_index_rejected'] is True, 'Witness safety checks missing')
    request = bytes(proof['request'])
    response = bytes(proof['held_original_ok'])
    expected = ("XA COMMIT X'"+proof['xid'].encode().hex().upper()+"',X'"+('-'+proof['branch_id']).encode().hex().upper()+"',9752").encode()
    need(int.from_bytes(request[:3], 'little') == len(request)-4 and request[3:] == b'\x00\x03'+expected, 'Captured command not original COM_QUERY XA identity')
    need(int.from_bytes(response[:3], 'little') == len(response)-4 and response[3:5] == b'\x01\x00', 'Captured original native COMMIT OK missing')


def mysql(folder):
    catalog(folder, sha(folder/'catalog.json'))
    sources(folder)
    result = load(folder/'result.json')
    need(result['Rust_versions'] == ['1.99', '1.85'] and result['XA_RM_gate_complete'] is False and result['whole_plan_complete'] is False, 'Managed scope changed')
    for rust in result['Rust_versions']:
        for index in range(13):
            prefix = rust+'-case-'+str(index)
            need(b'test result: ok. 1 passed;' in (folder/(prefix+'.stdout')).read_bytes(), 'Managed case did not execute')
            need(not (folder/(prefix+'-independent-prepared.stdout')).read_bytes().strip(), 'Managed Prepared remains')
            need(not (folder/(prefix+'-independent-kill.stdout')).read_bytes().strip(), 'Managed recovery issued remote KILL')
            commands = (folder/(prefix+'-independent-native-commands.stdout')).read_bytes().splitlines()
            writes = [line for line in commands if b'XA RECOVER' not in line]
            if index in [1, 2, 5]:
                need(not writes, 'Rejected registration/schema/durability started native XA')
            if index == 6:
                need(sum(b'XA COMMIT ' in line for line in writes) == 1, 'Unknown native commit replayed')
                proof = load(folder/'observations'/rust/'case-6/proof/native-commit-loss.json')
                witness_proof(proof)
                state = (proof['xid'].encode().hex().upper()+'\t'+proof['branch_id']+'\t1\t1').encode()
                need((folder/(prefix+'-independent-state.stdout')).read_bytes().strip() == state, 'Independent original terminal differs')
                need((folder/(prefix+'-independent-witness.stdout')).read_bytes().strip() == state, 'Independent original committed witness differs')
            if index == 8:
                need(sum(b'XA ROLLBACK ' in line for line in writes) == 1, 'Original native rollback replayed')
                need(sum(b'XA PREPARE ' in line for line in writes) == 1, 'Original Prepare replayed')
                proof = load(folder/'observations'/rust/'case-8/proof/native-rollback-loss.json')
                rollback_proof(proof)
                key = (proof['xid'].encode().hex().upper()+'\t'+proof['branch_id']).encode()
                need((folder/(prefix+'-independent-state.stdout')).read_bytes().strip() == key+b'\t2\t1', 'Original rollback terminal differs')
                need((folder/(prefix+'-independent-intent.stdout')).read_bytes().strip() == key+b'\t2\t0', 'Prepared rollback intent differs')
                need(not (folder/(prefix+'-independent-witness.stdout')).read_bytes().strip() and not (folder/(prefix+'-independent-business.stdout')).read_bytes().strip(), 'Rollback retained witness/business')
            if index == 3:
                proof = load(folder/'observations'/rust/'case-3/proof/bound-and-unknown.json')
                need(proof['missing_guard_enrollment'] == 'Unknown' and proof['enrollment_corruptions_rejected'] == 6 and proof['guard_corruptions_rejected'] == 6 and proof['guard_index_rejected'] is True and proof['restored_guard_fence'] == 'Applied', 'Guard negative controls missing')
            if index == 9:
                need(len(writes) == 1 and b'XA START ' in writes[0], 'Active fence sent native END/PREPARE/end command')
                proof = load(folder/'observations'/rust/'case-9/proof/active-scope-fence.json')
                active_proof(proof)
                key = (proof['xid'].encode().hex().upper()+'\t'+proof['branch_id']).encode()
                need((folder/(prefix+'-independent-state.stdout')).read_bytes().strip() == key+b'\t2\t1', 'Active rollback terminal differs')
                need((folder/(prefix+'-independent-guard.stdout')).read_bytes().strip() == key+b'\t2\t1', 'Active guard tombstone differs')
                need((folder/(prefix+'-independent-guard-enrollment.stdout')).read_bytes().strip() == key+b'\t0\t0', 'Original guard enrollment differs')
                need(not (folder/(prefix+'-independent-business.stdout')).read_bytes().strip(), 'Old active business escaped fence')
            if index == 7:
                proof = load(folder/'observations'/rust/'case-7/proof/early-callback.json')
                early_proof(proof)
                need(not writes, 'Early rollback sent native XA control')
                key = (proof['xid'].encode().hex().upper()+'\t'+proof['branch_id']).encode()
                need((folder/(prefix+'-independent-state.stdout')).read_bytes().strip() == key+b'\t2\t1', 'Early denial terminal differs')
                need((folder/(prefix+'-independent-guard.stdout')).read_bytes().strip() == key+b'\t2\t1', 'Early denial guard differs')
                need(not (folder/(prefix+'-independent-guard-enrollment.stdout')).read_bytes().strip(), 'Early callback minted producer enrollment')
            if index == 10:
                proof = load(folder/'observations'/rust/'case-10/proof/registration-callback-race.json')
                race_proof(proof)
                allowed = {("Query\tXA START X'"+row['xid'].encode().hex().upper()+"',X'"+('-'+row['branch_id']).encode().hex().upper()+"',9752").encode() for row in proof['cases']}
                need(len(writes) == len(set(writes)) and set(writes) <= allowed, 'Registration race repeated or changed native START')
                need(not (folder/(prefix+'-independent-business.stdout')).read_bytes().strip(), 'Registration race leaked business')
            if index == 11:
                proof = load(folder/'observations'/rust/'case-11/proof/early-orphan-evidence.json')
                orphan_proof(proof)
                row = proof['cases'][-1]
                suffix = ("X'"+row['xid'].encode().hex().upper()+"',X'"+('-'+row['branch_id']).encode().hex().upper()+"',9752").encode()
                need(writes == [b'Query\tXA '+verb+b' '+suffix for verb in [b'START',b'END',b'PREPARE',b'ROLLBACK']], 'Orphan Prepared cleanup command sequence changed')
                need(not (folder/(prefix+'-independent-business.stdout')).read_bytes().strip(), 'Orphan cleanup leaked business')
            if index == 12:
                proof = load(folder/'observations'/rust/'case-12/proof/process-crash-matrix.json')
                process_proof(proof)
                expected = []
                for row in proof['cases']:
                    suffix = ("X'"+row['xid'].encode().hex().upper()+"',X'"+('-'+row['branch_id']).encode().hex().upper()+"',9752").encode()
                    verbs = [b'START'] if row['stage'] in ['enlisted','active','unresponsive'] else [b'START',b'END',b'PREPARE',b'COMMIT' if row['callback_status']==5 else b'ROLLBACK']
                    expected.extend(b'Query\tXA '+verb+b' '+suffix for verb in verbs)
                    checkpoint = load(folder/'observations'/rust/'case-12/processes'/row['stage']/'child/proof/process-checkpoint.json')
                    need(checkpoint['rust_process_id'] == row['writer_pid'] and checkpoint['xid'] == row['xid'] and checkpoint['branch_id'] == row['branch_id'], 'Actual writer checkpoint identity differs')
                need(writes == expected, 'Process recovery repeated or changed native command sequence')
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned managed MySQL remains')


def rollback_proof(proof):
    need(proof['intermediate_journal'] == 'Pending' and proof['journal'] == 'Completed' and proof['callback_statuses'] == [9, 8] and proof['actual_commands'] == 1, 'Original rollback loss lifecycle differs')
    need(proof['commit_witness_rows'] == 0 and proof['business'] == [] and proof['intent_corruptions_rejected'] == 6 and proof['intent_extra_index_rejected'] is True and proof['staged_only'] == 'Unknown' and proof['missing_intent'] == 'Unknown' and proof['commit_witness_conflict_rejected'] is True and proof['fresh_manager_recovered'] is True, 'Prepared rollback safety proof missing')
    request = bytes(proof['request'])
    response = bytes(proof['held_original_ok'])
    expected = ("XA ROLLBACK X'"+proof['xid'].encode().hex().upper()+"',X'"+('-'+proof['branch_id']).encode().hex().upper()+"',9752").encode()
    need(int.from_bytes(request[:3], 'little') == len(request)-4 and request[3:] == b'\x00\x03'+expected, 'Captured original rollback identity differs')
    need(int.from_bytes(response[:3], 'little') == len(response)-4 and response[3:5] == b'\x01\x00', 'Original rollback OK missing')
    need(proof['prepare_ack_lost'] is True and proof['actual_prepare_commands'] == 1, 'Original Prepare uncertainty not exercised')
    need(proof['guard_protocol_removed_for_legacy_checks'] is True, 'Legacy Prepared safety checks masked by guard proof')
    prepare = bytes(proof['prepare_request'])
    prepare_ok = bytes(proof['held_prepare_ok'])
    expected_prepare = expected.replace(b'XA ROLLBACK ', b'XA PREPARE ', 1)
    need(int.from_bytes(prepare[:3], 'little') == len(prepare)-4 and prepare[3:] == b'\x00\x03'+expected_prepare, 'Original Prepare identity differs')
    need(int.from_bytes(prepare_ok[:3], 'little') == len(prepare_ok)-4 and prepare_ok[3:5] == b'\x01\x00', 'Original Prepare OK missing')


def active_proof(proof):
    need(proof['journal'] == 'Completed' and proof['callback_status'] == 8 and proof['business'] == [] and proof['guard'] == [2, 1], 'Active scope not actually rolled back')
    need(proof['caller_did_not_drop_before_callback'] is True and proof['independent_manager'] is True and proof['late_business_rejected'] is True and proof['late_prepare_rejected'] is True, 'Original active scope lifecycle not verified')
    need(type(proof['phase_zero_monitor_observations']) is int and proof['phase_zero_monitor_observations'] >= 2, 'Healthy original phase-zero scope not retained across actual monitor observations')


def early_proof(proof):
    need(proof['callback_statuses'] == [8,8] and proof['early_result'] == 'Applied' and proof['journal'] == 'Completed' and proof['business_rows'] == 0, 'Early callback did not durably finish rollback')
    need(proof['manual_entry_result'] == 'Unknown' and proof['manual_dispatch_no_control_rows'] is True and proof['producer_enrollment_rows'] == 0, 'Manual identity minted callback authority')
    need(proof['late_start_rejected'] is True and proof['late_receipt_preserved'] is True and proof['duplicate_receipt_rejected'] is True and proof['fresh_manager_terminal'] == 'Applied' and proof['opposite_phase_rejected'] is True, 'Late original registration escaped denial')


def race_proof(proof):
    need(len(proof['cases']) == 3 and proof['original_registration_each_case'] == 1 and proof['whole_plan_complete'] is False, 'Registration race scope changed')
    need(len({row['branch_id'] for row in proof['cases']}) == 3, 'Registration race reused identity')
    for row in proof['cases']:
        need(row['callback_statuses'] and row['callback_statuses'][-1] == 8 and set(row['callback_statuses']) <= {8,9}, 'Registration race not completed or incorrectly rejected')
        need(row['application_data'] == 'original-race-application' and row['late_business_rejected'] is True and row['journal'] == 'Completed' and row['business'] == [], 'Registration race changed original data or leaked business')


def orphan_proof(proof):
    need([row['kind'] for row in proof['cases']] == ['seata_rs_xa_commit_witness_v1','seata_rs_xa_prepare_evidence_v1','seata_rs_xa_guard_enrollment_v1','seata_rs_xa_session_guard_v1','native_prepared'], 'Orphan matrix incomplete')
    for row in proof['cases']:
        need(row['first_status'] == (9 if row['kind'] == 'native_prepared' else 10) and row['provisional_denial_rows_after_failure'] == 0 and row['first_journal'] == 'Pending', 'Orphan evidence falsely committed denial')
        need(row['repaired_status'] == 8 and row['late_start_rejected'] is True and row['journal'] == 'Completed' and row['business'] == [], 'Orphan repair failed original rollback')


def process_proof(proof):
    stages = ['enlisted','active','prepared_commit','prepared_rollback','commit_ok','rollback_ok','terminal_commit_ok','terminal_rollback_ok','unresponsive']
    need([row['stage'] for row in proof['cases']] == stages and proof['whole_plan_complete'] is False, 'Process crash matrix incomplete')
    for index,row in enumerate(proof['cases']):
        stage = row['stage']
        commit = stage in ['prepared_commit','commit_ok','terminal_commit_ok']
        intercepted = stage in ['commit_ok','rollback_ok','terminal_commit_ok','terminal_rollback_ok']
        active = stage in ['enlisted','active','unresponsive']
        phase = 1 if commit else 2
        status = 5 if commit else 8
        need(row['xid'] == 'xa:process:'+stage and row['branch_id'] == str(9007199254741100+index), 'Process original identity changed')
        need(type(row['writer_pid']) is int and row['writer_pid'] > 0 and row['writer_pid'] == row['checkpoint_pid'] and row['writer_pid'] != row['recovery_pid'] == proof['rust_process_id'], 'Writer and recovery are not distinct real processes')
        need(row['writer_exited'] is True and row['writer_exit_code'] != 0 and row['guard_locked_at_checkpoint'] is True, 'Owned writer did not crash while holding original guard')
        need(row['prepared_at_checkpoint'] is (not active) and row['prepared_at_cut'] is stage.startswith('prepared_'), 'Physical Prepared boundary differs')
        before = [phase,1 if stage.startswith('terminal_') else 0] if intercepted or stage=='unresponsive' else [0,0]
        need(row['control_before_crash'] == before and row['business_at_cut'] == ([[1,42]] if stage in ['commit_ok','terminal_commit_ok'] else []), 'Physical effect/control cut differs')
        need(row['old_journal'] == ('Pending' if intercepted else 'None') and row['journal'] == 'Completed', 'Old Pending journal was not recovered')
        need(row['control_after'] == [phase,1] and row['business'] == ([[1,42]] if commit else []) and row['prepared_remaining'] is False and row['witness_after'] == int(commit), 'Process recovery effect differs')
        need(row['guard_after'] == ([2,1] if active else [0,0] if commit else None), 'Process guard outcome differs')
        need(row['callback_status'] == status and row['recovery_statuses'] and row['recovery_statuses'][-1] == status and set(row['recovery_statuses']) <= {status,6 if commit else 9}, 'New RM callback did not finish original branch')
        need(row['recovery_source_created_after_exit'] is (stage!='unresponsive') and row['first_unresponsive_status'] == (9 if stage=='unresponsive' else None), 'Live unresponsive guard was prematurely completed')
        if intercepted:
            request,response = map(bytes,row['held_packet'])
            command = b'COMMIT' if stage.startswith('terminal_') else ("XA "+('COMMIT' if commit else 'ROLLBACK')+" X'"+row['xid'].encode().hex().upper()+"',X'"+('-'+row['branch_id']).encode().hex().upper()+"',9752").encode()
            need(int.from_bytes(request[:3],'little') == len(request)-4 and request[3:] == b'\x00\x03'+command and int.from_bytes(response[:3],'little') == len(response)-4 and response[3:5] == b'\x01\x00', 'Held native/control OK not original command')
            need(type(row['matched_commands']) is int and (row['matched_commands'] >= (4 if commit else 5) if stage.startswith('terminal_') else row['matched_commands']==1), 'Native command replay or wrong control boundary')
        else:
            need(row['held_packet'] is None and row['matched_commands'] is None, 'Unexpected ACK fixture in plain crash')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--mysql', required=True)
    parser.add_argument('--java', required=True)
    args = parser.parse_args()
    mysql(Path(args.mysql))
    java(Path(args.java))
    print('MANAGED_XA_SEALED_SQL_AND_ORIGINAL_WIRE_VERIFIED')
