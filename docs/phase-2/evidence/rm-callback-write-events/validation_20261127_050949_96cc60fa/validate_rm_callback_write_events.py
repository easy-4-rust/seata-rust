"""Bind local-write events to original TC frames without asserting TC delivery."""
import argparse
import copy
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import uuid

from inventory_rebuilt import ROOT, need, save, sha
from qualify_rebuilt import verify_catalog

spec = importlib.util.spec_from_file_location('original_rm_wire', ROOT / 'tools/formal-rm-java/validate.py')
wire = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wire)
WRITE_FIELDS = {'target', 'message', 'request_id', 'connection_generation', 'response_count', 'outcome'}
DECISION_FIELDS = {'target', 'message', 'request_id', 'connection_generation', 'xid', 'branch_id',
                   'phase', 'branch_status', 'result_code', 'resource_sha256'}


def verify(data, scenario):
    events, rows = data['events'], data['frames']
    writes = [e for e in events if e.get('target') == 'seata_rm::callback_write']
    decisions = [e for e in events if e.get('target') == 'seata_rm::callback']
    need(len(events) == len(writes) + len(decisions), 'Unexpected supplemental event target')
    frames = [wire.envelope(row) for row in rows if row.get('event') == 'frame']
    kind = 5 if scenario.startswith('rollback') else 3
    callbacks = [r for r in frames if r['kind'] == kind and r['direction'] == 's2c']
    replies = [r for r in frames if r['kind'] == kind + 1 and r['direction'] == 'c2s']
    need(callbacks and len(callbacks) == len(replies) == len(writes) == len(decisions), 'Callback/event counts differ')
    need(len({r['request_id'] for r in callbacks}) == len(callbacks), 'Selected original TC callback IDs ambiguous')
    by_reply = {(r['connection'], r['request_id']): r for r in replies}
    by_write = {e['request_id']: e for e in writes}
    by_decision = {e['request_id']: e for e in decisions}
    need(len(by_reply) == len(replies) and len(by_write) == len(writes) and len(by_decision) == len(decisions), 'Duplicate event or reply')
    generations = {}
    for callback in callbacks:
        key = str(callback['request_id'])
        need(key in by_write and key in by_decision, 'Original callback event missing')
        write, decision = by_write[key], by_decision[key]
        need(set(write) == WRITE_FIELDS and set(decision) == DECISION_FIELDS, 'Event contains payload or unexpected fields')
        need(write['message'] == 'rm_callback_response_write' and write['outcome'] == 'local_written' and write['response_count'] == '1', 'Local write classification changed')
        need(decision['message'] == 'rm_callback_decision', 'Wrong local decision message')
        generation = write['connection_generation']
        need(type(generation) is str and generation.isdecimal() and int(generation) > 0 and generation == decision['connection_generation'], 'Origin generation differs')
        connection = callback['connection']
        need(generations.setdefault(connection, generation) == generation, 'Same origin has conflicting generations')
        identity = wire.callback(callback['body'])
        need(decision['xid'] == identity['xid'] and decision['branch_id'] == str(identity['branch_id']), 'Original branch identity differs')
        digest = hashlib.sha256(identity['resource'].encode()).hexdigest()
        need(decision['resource_sha256'] == digest, 'Resource digest differs')
        need(decision['phase'] == ('Rollback' if kind == 5 else 'Commit') and decision['result_code'] == 'Success', 'Original branch decision differs')
        reply = by_reply.get((connection, callback['request_id']))
        need(reply is not None and reply['index'] > callback['index'], 'Original reply order differs')
        terminal = 8 if kind == 5 else 5
        need(reply['body'][2:4] == b'\x01\x00' and reply['body'][-1] == terminal, 'Raw callback response differs')
        need(decision['branch_status'] == ('PhaseTwoRollbacked' if kind == 5 else 'PhaseTwoCommitted'), 'Logged branch status differs')
    need(len(set(generations.values())) == len(generations), 'Replacement and original generation conflated')
    loss = scenario.endswith('reply_loss')
    dropped = [r for r in replies if r['action'] == 'drop_and_close']
    need(len(dropped) == int(loss) and all(r['action'] in ['forward', 'drop_and_close'] for r in replies), 'Original proxy drop differs')
    if loss:
        need(len(callbacks) == 2 and len(generations) == 2, 'Lost response did not retry on replacement')
        need(dropped[0]['delivery_count'] == 0, 'Dropped reply delivered unexpectedly')
    need(rows[-1]['event'] == 'summary' and rows[-1]['succeeded'] and rows[-1]['faults'] == int(loss), 'Original observer did not qualify')
    return dict(callbacks=len(callbacks), local_writes=len(writes), TC_dropped_replies=len(dropped),
                distinct_generations=len(generations))


def controls(data):
    labels = ['local_write_as_delivered', 'secret_field', 'wrong_request_id', 'wrong_resource_digest',
              'conflated_generations', 'duplicate_event', 'missing_write', 'forged_TC_delivery', 'broken_original_frame']
    rejected = []
    for label in labels:
        value = copy.deepcopy(data)
        writes = [e for e in value['events'] if e['target'] == 'seata_rm::callback_write']
        decision = next(e for e in value['events'] if e['target'] == 'seata_rm::callback')
        if label == 'local_write_as_delivered': writes[0]['outcome'] = 'tc_delivered'
        elif label == 'secret_field': writes[0]['resource_id'] = 'mysql://secret'
        elif label == 'wrong_request_id': writes[0]['request_id'] = '-2147483648'
        elif label == 'wrong_resource_digest': decision['resource_sha256'] = '0' * 64
        elif label == 'conflated_generations':
            for event in value['events']: event['connection_generation'] = '1'
        elif label == 'duplicate_event': value['events'].append(copy.deepcopy(writes[0]))
        elif label == 'missing_write': value['events'].remove(writes[0])
        elif label == 'forged_TC_delivery':
            row = next(r for r in value['frames'] if r.get('action') == 'drop_and_close')
            row.update(action='forward', delivery_count=1)
        else:
            row = next(r for r in value['frames'] if r.get('event') == 'frame')
            row['frame_hex'] = '00' + row['frame_hex'][2:]
        try: verify(value, 'commit_callback_reply_loss')
        except (ValueError, KeyError): rejected.append(label)
        else: raise ValueError('Weakened local-write evidence accepted: ' + label)
    return rejected


def run(root, digest):
    verify_catalog(root, digest)
    records = json.loads((root / 'scenarios.json').read_bytes())
    need(len(records) == 6 and len({(r['Rust'], r['scenario']) for r in records}) == 6, 'Original TC scenario set incomplete')
    qualified = []
    negatives = {}
    for row in records:
        folder = ROOT / row['evidence']
        need(sha(folder / 'verified.json') == row['verified_sha256'], 'Original TC qualification drift')
        event_file, = row['stage_files']
        need(sha(folder / event_file['path']) == event_file['sha256'], 'Supplemental event bytes drift')
        data = dict(events=json.loads((folder / event_file['path']).read_bytes()),
                    frames=[json.loads(line) for line in (folder / 'RM-frames.jsonl').read_text(encoding='utf-8').splitlines()])
        qualified.append(dict(Rust=row['Rust'], scenario=row['scenario'], **verify(data, row['scenario'])))
        need(json.loads((folder / 'process-cleanup.json').read_bytes())['stopped'], 'Original TC process leaked')
        if row['scenario'] == 'commit_callback_reply_loss': negatives[row['Rust']] = controls(data)
    for source in json.loads((root / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / source['path']) == source['sha256'], 'Executed source drift')
    need(json.loads((root / 'cleanup.json').read_bytes())['owned_container_removed'], 'Owned container leaked')
    out = root.parent / ('validation_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir()
    save(out / 'result.json', dict(status='ORIGINAL_TC_LOCAL_WRITE_EVENTS_BOUND_TO_RAW_FRAMES', evidence=str(root),
         catalog_sha256=digest, qualified=qualified, negative_controls=negatives,
         independent_external_review='OPEN', WP07_complete=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    shutil.copyfile(ROOT / 'tools/formal-rm-java/validate.py', out / 'original-rm-wire-verifier.py')
    save(out / 'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(validation=str(out), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--catalog-sha256', required=True)
    args = parser.parse_args()
    run(args.evidence, args.catalog_sha256)
