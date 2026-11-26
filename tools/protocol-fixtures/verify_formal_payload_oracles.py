"""只读核对正式Rust使用的Java记录索引及其不可变来源，不改写任何夹具。"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tests/fixtures/protocol/v1/formal_payload_oracles'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify():
    manifest = json.loads((HERE / 'manifest.json').read_bytes())
    if manifest['java_pin'] != 'cba73e180fd8896a99df6415b859d275122feaa7':
        raise ValueError('Java baseline changed')
    counts = {'exceptions.tsv': 250, 'states.tsv': 168, 'failures.tsv': 7}
    if {row['file'] for row in manifest['files']} != set(counts):
        raise ValueError('Need all three oracle families')
    total = 0
    for row in manifest['files']:
        path = HERE / row['file']
        source = (ROOT / row['source']).resolve()
        if not source.is_relative_to(ROOT) or sha(source) != row['source_sha256']:
            raise ValueError('Source identity differs: ' + row['file'])
        if sha(path) != row['sha256'] or row['records'] != counts[row['file']]:
            raise ValueError('Index identity differs: ' + row['file'])
        values = json.loads(source.read_bytes())
        if row['file'] == 'exceptions.tsv':
            lines = ['\t'.join([str(r['type_id']), str(r['error_code']), r['error_name'], r['serialized_hex']]) for r in values]
        elif row['file'] == 'states.tsv':
            lines = ['\t'.join([str(r['type']), str(r['status']), str(r['result']), r['hex']]) for r in values['responses']]
        else:
            lines = []
            for value in values['samples']:
                data = value['input']
                binary = source.parent / value['payloads']['java']['file']
                if sha(binary) != value['payloads']['java']['sha256']:
                    raise ValueError('Original Java binary changed')
                lines.append('\t'.join([str(value['type_id']), str(data['java_exception_code']), data['msg'].encode().hex(),
                    data.get('xid', '').encode().hex(), str(data.get('branch_id', 0)), str(data.get('branch_status', 0)),
                    str(data.get('global_status', 0)), str(int(data.get('lockable', False))), binary.read_bytes().hex()]))
        if len(lines) != counts[row['file']] or ('\n'.join(lines) + '\n').encode() != path.read_bytes():
            raise ValueError('Index does not exactly reproduce recorded Java output')
        total += len(lines)
    return {'verified_java_records': total, 'new_java_execution': False, 'source_pin': manifest['java_pin']}


if __name__ == '__main__':
    print(json.dumps(verify()))
