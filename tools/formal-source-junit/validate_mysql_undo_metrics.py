"""Independently replay fixed Undo counters against raw MySQL fault and HTTP evidence."""
import argparse
import copy
import datetime
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import uuid

from inventory_rebuilt import ROOT, need, save, sha
from qualify_rebuilt import verify_catalog

FIELDS = ('active', 'completed', 'unknown_commit', 'before_commit_error', 'cancelled',
          'consumed', 'fenced_inserted', 'fenced_existing', 'undo_present', 'observed_restored_rows')


def expected(**fields):
    return dict.fromkeys(FIELDS, 0) | fields


def counts(value, **fields):
    need(set(value) == set(FIELDS) and all(type(n) is int and 0 <= n <= 2**64 - 1 for n in value.values()),
         'Metric shape contains labels, booleans or invalid counters')
    need(value == expected(**fields), 'Observed independent Undo counters differ')


def text_counts(text, zero=False):
    values = {}
    for line in text.splitlines():
        match = re.fullmatch(r'(seata_mysql_undo_[^ ]+) ([0-9]+)', line)
        need(match is not None and match[1] not in values, 'Malformed/duplicate HTTP metric')
        values[match[1]] = int(match[2])
    wanted = {}
    for operation in ('rollback', 'fence'):
        fields = expected() if zero else expected(**({'before_commit_error': 1} if operation == 'rollback' else
                                                   {'completed': 2, 'fenced_inserted': 1, 'fenced_existing': 1}))
        wanted[f'seata_mysql_undo_active{{operation="{operation}"}}'] = fields['active']
        for field in FIELDS[1:5]:
            wanted[f'seata_mysql_undo_total{{operation="{operation}",outcome="{field}"}}'] = fields[field]
        for field in FIELDS[5:9]:
            wanted[f'seata_mysql_undo_observations_total{{operation="{operation}",observation="{field}"}}'] = fields[field]
        wanted[f'seata_mysql_undo_restored_rows_observed_total{{operation="{operation}"}}'] = fields['observed_restored_rows']
    need(values == wanted and len(values) == 20, 'Fixed HTTP series/observations differ')


def wire_proof(wire):
    need(wire['mode'] == 'drop_ok' and wire['claimed'] is True, 'COMMIT fault not claimed')
    events = wire['events']
    requests = [e for e in events if e['kind'] == 'commit_received' and e.get('fault') is True]
    need(len(requests) == 1, 'Unique faulted original COMMIT missing')
    request = requests[0]
    need(bytes.fromhex(request['wire_hex']) == b'\x07\x00\x00\x00\x03COMMIT', 'Not an original COM_QUERY COMMIT')
    selected = {kind: [e for e in events if e['kind'] == kind and e['connection'] == request['connection']]
                for kind in ('commit_forwarded', 'commit_reply', 'commit_reply_dropped', 'commit_reply_forwarded')}
    need(all(len(selected[k]) == 1 for k in ('commit_forwarded', 'commit_reply', 'commit_reply_dropped')) and
         not selected['commit_reply_forwarded'], 'Actual original OK drop lifecycle differs')
    forwarded, reply, dropped = [selected[k][0] for k in ('commit_forwarded', 'commit_reply', 'commit_reply_dropped')]
    payload = bytes.fromhex(reply['payload_hex'])
    need(reply['ok'] is True and payload[0] == 0 and bytes.fromhex(reply['wire_hex'])[4:] == payload and
         hashlib.sha256(payload).hexdigest() == reply['sha256'], 'Actual MySQL OK bytes differ')
    need(request['qpc_ns'] < forwarded['qpc_ns'] < reply['qpc_ns'] < dropped['qpc_ns'], 'Fault causal ordering differs')


def verify(data):
    api = data['api']
    counts(api['fence'], completed=3, before_commit_error=1, undo_present=1, fenced_inserted=1, fenced_existing=1)
    counts(api['rollback'], completed=2, before_commit_error=1, consumed=1, observed_restored_rows=1, fenced_existing=1)
    need(api['business_rows'] == 0 and api['undo_rows'] == 1, 'Independent API physical result differs')
    for key, field in [('cancel', 'cancelled'), ('deadline', 'before_commit_error')]:
        counts(data[key]['interrupted'], **{field: 1})
        counts(data[key]['after_explicit_recovery'], **{field: 1}, completed=1, consumed=1, observed_restored_rows=1)
    for role in ('rollback', 'fence'):
        metric = data[role + '_metrics']
        counts(metric['unknown'], unknown_commit=1)
        counts(metric['after_explicit_observation'], unknown_commit=1, completed=1,
               **({'fenced_inserted': 1} if role == 'rollback' else {'fenced_existing': 1}))
        wire_proof(data[role + '_wire'])
    physical = data['rollback_physical']
    need(physical['unknown'] is True and physical['mode'] == 'drop_ok' and
         physical['before_explicit_recovery'] == {'business': 0, 'undo': 0} and
         physical['after_explicit_recovery'] == {'business': 0, 'undo': 1}, 'Unknown rollback physical effect differs')
    fence = data['fence_physical']
    need(fence['unknown'] is True and fence['mode'] == 'drop_ok' and fence['before_explicit_recovery'] == 1 and
         fence['after'] == 1 and fence['business_rows'] == 0 and fence['recovery_inserted'] is False, 'Unknown fence physical effect differs')
    text_counts(data['http_zero'], zero=True)
    text_counts(data['http_final'])


def controls(data):
    rejected = []
    for label in ('unknown_as_completed', 'unknown_as_restored_rows', 'double_existing_fence', 'active_leak',
                  'missing_counter', 'boolean_counter', 'identity_label', 'http_identity_label',
                  'no_original_OK_drop', 'forged_OK_checksum', 'response_before_request', 'cancellation_as_success'):
        value = copy.deepcopy(data)
        unknown = value['rollback_metrics']['unknown']
        if label == 'unknown_as_completed':
            unknown.update(unknown_commit=0, completed=1)
        elif label == 'unknown_as_restored_rows':
            unknown['observed_restored_rows'] = 1
        elif label == 'double_existing_fence':
            value['fence_metrics']['after_explicit_observation']['fenced_existing'] = 2
        elif label == 'active_leak':
            unknown['active'] = 1
        elif label == 'missing_counter':
            unknown.pop('cancelled')
        elif label == 'boolean_counter':
            unknown['unknown_commit'] = True
        elif label == 'identity_label':
            unknown['xid'] = 'private'
        elif label == 'http_identity_label':
            value['http_final'] = value['http_final'].replace('operation="fence"', 'operation="fence",xid="private"')
        elif label == 'no_original_OK_drop':
            for event in value['rollback_wire']['events']:
                if event['kind'] == 'commit_reply_dropped':
                    event['kind'] = 'commit_reply_forwarded'
        elif label == 'forged_OK_checksum':
            next(e for e in value['rollback_wire']['events'] if e['kind'] == 'commit_reply')['sha256'] = '0' * 64
        elif label == 'response_before_request':
            next(e for e in value['rollback_wire']['events'] if e['kind'] == 'commit_reply')['qpc_ns'] = 0
        else:
            value['cancel']['interrupted'].update(cancelled=0, completed=1)
        try:
            verify(value)
        except ValueError:
            rejected.append(label)
        else:
            raise ValueError('Weakened Undo evidence accepted: ' + label)
    return rejected


def run(root, digest):
    verify_catalog(root, digest)
    versions = {}
    for version in ('1.99', '1.85'):
        obs = root / 'observations' / version
        def read(path):
            return json.loads(path.read_bytes())
        data = dict(api=read(obs / 'api/independent-undo-observations.json'),
                    cancel=read(obs / 'wait-0/independent-undo-cancel-metrics.json'),
                    deadline=read(obs / 'wait-1/independent-undo-deadline-metrics.json'),
                    http_zero=(obs / 'http/independent-undo-http-zero.txt').read_text(encoding='utf-8'),
                    http_final=(obs / 'http/independent-undo-http-after-close.txt').read_text(encoding='utf-8'))
        for role in ('rollback', 'fence'):
            data[role + '_metrics'] = read(obs / role / (role + '-independent-metrics.json'))
            data[role + '_wire'] = read(root / (version + '-' + role + '-wire.json'))
        data['rollback_physical'] = read(obs / 'rollback/rollback-wire-observation.json')
        data['fence_physical'] = read(obs / 'fence/wire-observation.json')
        verify(data)
        versions[version] = dict(verified=True, negative_controls=controls(data), fixed_http_series=20)
    for row in json.loads((root / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'], 'Executed source input drift')
    need((root / 'server-after.stdout').read_text().splitlines() == ['0'] and
         json.loads((root / 'cleanup.json').read_bytes())['owned_container_removed'], 'Owned runtime cleanup incomplete')
    out = root.parent / ('validation_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir()
    for name in ('formal-undo-fence/proxy.py', 'formal-undo-rollback-loss/proxy.py'):
        path = ROOT / 'tools' / name
        original = subprocess.check_output(['git', 'show', '9180cddfbb52005a85b8250096812fdce6e7ba24:tools/' + name], cwd=ROOT)
        need(path.read_bytes() == original, 'Inherited raw observer source changed')
        target = out / name.replace('/', '-')
        target.write_bytes(original)
    save(out / 'result.json', dict(status='INDEPENDENT_UNDO_METRICS_RAW_EVIDENCE_VERIFIED',
         evidence=str(root), catalog_sha256=digest, versions=versions,
         observers_inherited_unchanged_from_Git='9180cddfbb52005a85b8250096812fdce6e7ba24',
         independent_human_or_other_agent_review='OPEN', WP07_complete=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(validation=str(out), catalog_sha256=sha(out / 'catalog.json'))), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--catalog-sha256', required=True)
    args = parser.parse_args()
    run(args.evidence, args.catalog_sha256)
