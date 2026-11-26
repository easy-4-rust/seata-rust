"""Replay archived captures and separately validate an offline schema-adapted trace."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path

root = Path.cwd()
tools = root / 'tools'
evidence = root / 'docs/phase-2/evidence'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, tools / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read(path):
    return json.loads(path.read_bytes())


def frame(payload, sequence):
    return (len(payload).to_bytes(3, 'little') + bytes([sequence]) + payload).hex()


def upgraded(trace):
    """Change only legacy DELETE commands in a test copy; this is not server evidence."""
    new = copy.deepcopy(trace)
    new['normal_delete_identity'] = 'byte-exact'
    changed = {}
    for event in new['events']:
        if event.get('sql') != direct.REMOVE:
            continue
        event['sql'] = direct.EXACT_REMOVE
        raw = bytes.fromhex(event['wire_hex'])
        payload = raw[4:]
        if event['kind'] == 'prepare':
            packet = frame(b'\x16' + direct.EXACT_REMOVE.encode(), 0)
            changed[event['connection'], event['wire_hex']] = packet
            event['wire_hex'] = packet
        elif event['kind'] == 'prepare_ok':
            assert payload[7:9] == b'\x02\x00'
            event['wire_hex'] = frame(payload[:7] + b'\x03\x00' + payload[9:], 1)
        elif event['kind'] == 'execute':
            statement = int.from_bytes(payload[1:5], 'little')
            xid, branch = direct.parameters(event, statement, 2)
            assert 0 < len(xid) < 251 and branch == 42
            packet = frame(payload[:16] + b'\xfd\x00' + payload[16:] + bytes([len(xid)]) + xid, 0)
            changed[event['connection'], event['wire_hex']] = packet
            event['wire_hex'] = packet
    assert changed
    for event in new['events']:
        packet = changed.get((event['connection'], event.get('wire_hex')))
        if event['kind'] == 'client_command' and packet:
            relink(event, packet)
    return new


def relink(event, packet):
    event.update(wire_hex=packet, wire_prefix_hex=packet[:10],
                 wire_sha256=hashlib.sha256(bytes.fromhex(packet)).hexdigest())


def wrong_byte_exact_xid(trace):
    wrong = copy.deepcopy(trace)
    event = next(e for e in wrong['events'] if e['kind'] == 'execute' and e['sql'] == direct.EXACT_REMOVE)
    old = event['wire_hex']
    raw = bytearray.fromhex(old)
    xid = direct.parameters(event, int.from_bytes(raw[5:9], 'little'), 3)[-1]
    assert bytes(raw[-len(xid):]) == xid
    raw[-1] ^= 1
    event['wire_hex'] = raw.hex()
    linked = next(e for e in wrong['events'] if e['kind'] == 'client_command'
                  and e['wire_hex'] == old and e['connection'] == event['connection']
                  and e['qpc_ns'] < event['qpc_ns'])
    relink(linked, raw.hex())
    return wrong


def reject(verify, trace):
    try:
        verify(trace)
    except ValueError as error:
        return str(error)
    raise AssertionError('wrong proof accepted')


def exercise(path, verify, expected):
    trace = read(path / 'mysql-wire.json')
    assert verify(trace) == expected
    checked = upgraded(trace)
    assert verify(checked) == expected
    legacy_as_new = {**trace, 'normal_delete_identity': 'byte-exact'}
    upgraded_unmarked = {key: value for key, value in checked.items()
                         if key != 'normal_delete_identity'}
    return {
        'history': path.relative_to(root).as_posix(),
        'archived_replay': 'MATCH',
        'synthetic_exact_format': 'MATCH',
        'rejections': {
            'wrong_exact_bind': reject(verify, wrong_byte_exact_xid(checked)),
            'marked_legacy': reject(verify, legacy_as_new),
            'unmarked_exact': reject(verify, upgraded_unmarked),
            'null_marker': reject(verify, {**trace, 'normal_delete_identity': None}),
        },
    }


direct = load('direct', 'formal-undo-rollback-loss/validate.py')
rm = load('managed', 'formal-rm-mysql/validate.py')
preconditions = load('preconditions', 'formal-commit-preconditions/managed_rollback_wire.py')
rows = []
for name in ('5016ef18', 'a2db7894', 'd4cf70d7'):
    path = evidence / 'mysql-undo-rollback' / f'run_20261117_111149_{name}'
    observation = read(path / 'observations/rollback-wire-observation.json')
    rows.append(exercise(path, lambda trace: direct.validate(trace, observation), read(path/'wire-validation.json')))
    assert direct.negative_controls(read(path/'mysql-wire.json'), observation) == read(path/'wire-negative-controls.json')

for name, validator, observation_leaf in (
    ('124937_1876c94c', rm.validate, 'rm-wire-observation'),
    ('124941_5185439e', rm.validate, 'rm-wire-observation'),
    ('124949_97899cbe', rm.validate, 'rm-wire-observation'),
    ('125017_2f1979db', rm.validate_denial, 'rm-denial-observation'),
    ('125022_695641d3', rm.validate_concurrency, 'rm-concurrent-observation'),
):
    path = evidence / 'mysql-rm-recovery' / f'run_20261117_{name}'
    observation = read(path / 'observations' / f'{observation_leaf}.json')
    rows.append(exercise(path, lambda trace: validator(trace, observation), read(path/'wire-validation.json')))

path = evidence / 'mysql-managed-rollback-fault/seata_managed_rollback_rr6do0ye'
checkpoint = read(path/'observer-checkpoint.json')
rows.append(exercise(path, lambda trace: preconditions.verify(trace, checkpoint['observed'], checkpoint['qpc_ns']),
                     read(path/'wire-verification.json')))
print(json.dumps(rows, indent=2))
