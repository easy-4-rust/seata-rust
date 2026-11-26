"""Bind negotiated mTLS sessions to unchanged original TC frames and callback results."""
import argparse
import copy
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import uuid

from inventory_rebuilt import ROOT, need, save, sha
from qualify_rebuilt import verify_catalog
from validate_rm_callback_write_events import verify as callback_events, wire


def verify(frontends, frames, client_digest, scenario):
    need(len(frontends) == 2 and {f['role'] for f in frontends} == {'TM', 'RM'}, 'Two secure role frontends required')
    summary = []
    for frontend in frontends:
        role = frontend['role']
        need(frontend['mutual_TLS'] is True and frontend['TLS'] == 'TLSv1.3' and frontend['backend'] == 'loopback-only', 'Mutual TLS profile differs')
        need(frontend['expected_client_sha256'] == client_digest, 'Expected client certificate differs')
        need(frontend['stopped'] is True and frontend['active_handlers'] == 0 and frontend['active_writers'] == 0 and not frontend['handshake_errors'], 'TLS runtime failed or leaked')
        native = [wire.envelope(row) for row in frames[role] if row.get('event') == 'frame']
        connections = sorted({r['connection'] for r in native})
        sessions = frontend['sessions']
        need(len(sessions) == len(connections) and sessions, 'Negotiated TLS/native connections differ')
        # 当前 profile 每角色仅一个池槽，完成旧连接后才建立新代际，按实际接受顺序对应。
        for session, connection in zip(sessions, connections, strict=True):
            need(session['TLS'] == 'TLSv1.3' and session['peer_sha256'] == client_digest and session['closed'] is True, 'Authenticated TLS session differs')
            need(type(session['opened_qpc_ns']) is int and session['opened_qpc_ns'] < session['closed_qpc_ns'], 'TLS lifetime ordering differs')
            for direction, counter in [('c2s', 'upstream_bytes'), ('s2c', 'downstream_bytes')]:
                total = sum(len(bytes.fromhex(r['frame_hex'])) for r in native if r['connection'] == connection and r['direction'] == direction)
                need(type(session[counter]) is int and session[counter] == total and total > 0, 'Decrypted forwarded bytes differ from actual original frames')
        if role == 'RM':
            expected = 2 if scenario.endswith('reply_loss') else 1
            need(len(sessions) == expected, 'TLS RM reconnect count differs from original TC scenario')
        else:
            need(1 <= len(sessions) <= 2, 'Unexpected TM TLS lifecycle')
            for connection in connections[1:]:
                requests = [r['kind'] for r in native if r['connection'] == connection and r['direction'] == 'c2s']
                need(requests and requests[0] == 101 and requests.count(101) == 1 and all(kind == 15 for kind in requests[1:]) and len(requests) > 1,
                     'Replacement TM must register then only observe status, never replay Begin/end')
        summary.append(dict(role=role, mutual_TLS_sessions=len(sessions), original_native_connections=len(connections)))
    return summary


def controls(data):
    rejected = []
    for label in ['no_mutual_auth', 'wrong_protocol', 'untrusted_client', 'plaintext_backend_policy',
                  'missing_reconnect', 'forged_forwarded_bytes', 'boolean_byte_count', 'active_socket_leak', 'handshake_error_hidden']:
        value = copy.deepcopy(data)
        frontend = next(f for f in value['frontends'] if f['role'] == 'RM')
        session = frontend['sessions'][0]
        if label == 'no_mutual_auth': frontend['mutual_TLS'] = False
        elif label == 'wrong_protocol': session['TLS'] = 'TLSv1.2'
        elif label == 'untrusted_client': session['peer_sha256'] = '0' * 64
        elif label == 'plaintext_backend_policy': frontend['backend'] = 'remote-cleartext'
        elif label == 'missing_reconnect': frontend['sessions'].pop()
        elif label == 'forged_forwarded_bytes': session['upstream_bytes'] += 1
        elif label == 'boolean_byte_count': session['upstream_bytes'] = True
        elif label == 'active_socket_leak': frontend['active_writers'] = 1
        else: frontend['handshake_errors'].append(dict(kind='SSLError'))
        try: verify(value['frontends'], value['frames'], value['client_digest'], 'commit_callback_reply_loss')
        except ValueError: rejected.append(label)
        else: raise ValueError('TLS evidence weakening accepted: ' + label)
    return rejected


def run(root, digest):
    verify_catalog(root, digest)
    records = json.loads((root / 'scenarios.json').read_bytes())
    expected = {(v,s) for v in ['1.99','1.85'] for s in ['commit','commit_callback_reply_loss','rollback_callback_reply_loss']}
    need({(r['Rust'],r['scenario']) for r in records} == expected and len(records) == 6, 'Original TLS scenario set incomplete')
    client_digest = sha(root / 'client.der')
    qualified, negatives, raw_inputs = [], {}, []
    out = root.parent / ('validation_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir()
    for row in records:
        folder = ROOT / row['evidence']
        need(sha(folder / 'verified.json') == row['verified_sha256'], 'Original TC qualification drift')
        observation, = row['stage_files']
        need(observation['path'] == 'transport-tls.json' and sha(folder / observation['path']) == observation['sha256'], 'TLS session bytes drift')
        data = dict(frontends=json.loads((folder / observation['path']).read_bytes()), client_digest=client_digest,
             frames={role:[json.loads(line) for line in (folder / (role+'-frames.jsonl')).read_text(encoding='utf-8').splitlines()] for role in ['TM','RM']})
        sessions = verify(data['frontends'], data['frames'], client_digest, row['scenario'])
        decisions = callback_events(dict(events=json.loads((folder / 'rm-callback-write-events.json').read_bytes()), frames=data['frames']['RM']), row['scenario'])
        qualified.append(dict(Rust=row['Rust'],scenario=row['scenario'],sessions=sessions,callback_write=decisions))
        if row['scenario'] == 'commit_callback_reply_loss': negatives[row['Rust']] = controls(data)
        need(json.loads((folder / 'process-cleanup.json').read_bytes())['stopped'], 'Original TC process remains')
        copied = out / (row['Rust']+'-'+row['scenario'])
        copied.mkdir()
        for name in ['TM-frames.jsonl','RM-frames.jsonl','rm-callback-write-events.json','transport-tls.json','verified.json','process-cleanup.json']:
            shutil.copyfile(folder / name, copied / name)
            raw_inputs.append(dict(path=(copied/name).relative_to(out).as_posix(),sha256=sha(copied/name)))
    for row in json.loads((root / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'], 'Consumed TLS source drift')
    need(json.loads((root / 'cleanup.json').read_bytes())['owned_container_removed'], 'Owned TLS database container remains')
    save(out / 'result.json',dict(status='MUTUAL_TLS_BOUND_TO_ORIGINAL_TC_FRAMES', evidence=str(root), catalog_sha256=digest,
         client_certificate_sha256=client_digest, qualified=qualified, negative_controls=negatives,
         copied_primary_observations=raw_inputs, original_Java_TC_native_TLS_claimed=False,
         independent_external_review='OPEN', WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out / 'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(validation=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--evidence',type=Path,required=True)
    parser.add_argument('--catalog-sha256',required=True)
    args=parser.parse_args()
    run(args.evidence,args.catalog_sha256)
