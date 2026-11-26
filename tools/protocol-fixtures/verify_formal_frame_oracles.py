"""只读验证正式帧夹具、新Java执行封存及78处理器期望，不重新执行Java。"""
import hashlib
import json
from pathlib import Path
from capture_formal_frames import generated_owner

ROOT = Path(__file__).resolve().parents[2]
PIN = 'cba73e180fd8896a99df6415b859d275122feaa7'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify():
    fixture = ROOT / 'tests/fixtures/protocol/v1/formal_frames'
    sources = []
    for leaf in ['manifest.json', 'certification.json']:
        certificate = json.loads((fixture / leaf).read_bytes())
        evidence = (ROOT / certificate['evidence']).resolve()
        if not evidence.is_relative_to(ROOT) or certificate['source_pin'] != PIN or certificate['cases'] != 37:
            raise ValueError('Unexpected formal frame authority')
        if sha(fixture / 'java.tsv') != certificate['java_tsv_sha256'] or sha(evidence / 'catalog.json') != certificate['catalog_sha256']:
            raise ValueError('Formal frame index identity changed')
        if (fixture / 'java.tsv').read_bytes() != (evidence / 'java.tsv').read_bytes():
            raise ValueError('Actual Java capture differs')
        members = json.loads((evidence / 'catalog.json').read_bytes())['members']
        for row in members:
            path = (evidence / row['path']).resolve()
            if not path.is_relative_to(evidence) or sha(path) != row['sha256']:
                raise ValueError('Java capture member changed')
            sources.append(str(path))
        sources.append(str(evidence / 'catalog.json'))
        result = json.loads((evidence / 'result.json').read_bytes())
        if result['source_pin'] != PIN or result['java_frames'] != 37 or not result['actual_original_encoder_decoder'] or result['junit_engine']:
            raise ValueError('Actual Java execution missing')
        classes = {row['class']: row['sha256'] for row in json.loads((evidence / 'original-classes.json').read_bytes())}
        for row in json.loads((evidence / 'generated-classes.json').read_bytes()):
            owner = generated_owner(row['class'], row['source'])
            if owner is None or owner not in classes:
                raise ValueError('Unbound or unexpected non-file production class')
        values = json.loads((evidence / 'java.json').read_bytes())
        for row in values['origins']:
            if classes.get(row['class']) != row['sha256']:
                raise ValueError('Critical Java class unbound')
        frames = values['frames']
        if len(frames) != 37 or len({r['name'] for r in frames}) != 37:
            raise ValueError('Java frame case set changed')
        lines = ['\t'.join([r['name'], str(r['id']), str(r['kind']), r['head_hex'], r['frame_hex'], r['payload_hex']]) for r in frames]
        if ('\n'.join(lines) + '\n').encode() != (fixture / 'java.tsv').read_bytes():
            raise ValueError('Java TSV differs from captured field/byte records')
    directory = ROOT / 'tests/fixtures/protocol/v1/handler_matrix'
    manifest = json.loads((directory / 'manifest.json').read_bytes())
    if manifest['java_source'] != PIN or manifest['sample_count'] != 78 or manifest['case_count'] != 79:
        raise ValueError('Handler fixture case set changed')
    names = set()
    for row in manifest['samples']:
        if row['case_id'] in names:
            raise ValueError('Duplicate handler case')
        names.add(row['case_id'])
        if sha(directory / row['file']) != row['sha256'] or sha(directory / row['expectation']) != row['expectation_sha256']:
            raise ValueError('Handler payload/expectation changed')
    if len(names) != 78 or {r['file'] for r in manifest['samples']} != {p.name for p in directory.glob('*.bin')}:
        raise ValueError('Incomplete handler fixture set')
    return {'actual_java_frames': 37, 'historical_handler_contexts': 78,
            'source_files': sorted(set(sources)), 'new_java_execution_in_this_verifier': False}


if __name__ == '__main__':
    values = verify()
    values['source_files'] = len(values['source_files'])
    print(json.dumps(values))
