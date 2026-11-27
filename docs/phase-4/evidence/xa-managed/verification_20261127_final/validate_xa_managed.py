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


def mysql(folder):
    catalog(folder, sha(folder/'catalog.json'))
    sources(folder)
    result = load(folder/'result.json')
    need(result['Rust_versions'] == ['1.99', '1.85'] and result['XA_RM_gate_complete'] is False and result['whole_plan_complete'] is False, 'Managed scope changed')
    for rust in result['Rust_versions']:
        for index in range(8):
            prefix = rust+'-case-'+str(index)
            need(b'test result: ok. 1 passed;' in (folder/(prefix+'.stdout')).read_bytes(), 'Managed case did not execute')
            need(not (folder/(prefix+'-independent-prepared.stdout')).read_bytes().strip(), 'Managed Prepared remains')
            commands = (folder/(prefix+'-independent-native-commands.stdout')).read_bytes().splitlines()
            writes = [line for line in commands if b'XA RECOVER' not in line]
            if index in [1, 2, 5]:
                need(not writes, 'Rejected registration/schema/durability started native XA')
            if index == 6:
                need(sum(b'XA COMMIT ' in line for line in writes) == 1, 'Unknown native commit replayed')
                proof = load(folder/'observations'/rust/'case-6/proof/native-commit-loss.json')
                need(proof['journal'] == 'Pending' and proof['control_state'] == [1, 0] and proof['actual_commands'] == 1 and proof['callback_statuses'] == [6, 6], 'Lost original commit OK falsely completed')
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned managed MySQL remains')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--mysql', required=True)
    parser.add_argument('--java', required=True)
    args = parser.parse_args()
    mysql(Path(args.mysql))
    java(Path(args.java))
    print('MANAGED_XA_SEALED_SQL_AND_ORIGINAL_WIRE_VERIFIED')
