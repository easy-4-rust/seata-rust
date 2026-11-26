"""只读核对正式聚合测试消费的历史Java捕获，不启动Java或改写夹具。"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = 'cba73e180fd8896a99df6415b859d275122feaa7'
SOURCES = {
    'control_envelopes': 'docs/phase-0/evidence/local-control-envelopes/run_20261112_174052_29ab5466/capture',
    'merge_profile': 'docs/phase-0/evidence/local-go-java-merge-profile/run_20261112_193633_d41d73bc/capture',
}


def verify():
    sources = []
    counts = {}
    for family, relative in SOURCES.items():
        fixture = ROOT / 'tests/fixtures/protocol/v1' / family
        capture = ROOT / relative
        for leaf in ['manifest.json', 'vectors.json', 'vectors.tsv']:
            path = capture / leaf
            if path.read_bytes() != (fixture / leaf).read_bytes():
                raise ValueError('Historical capture differs: ' + family + '/' + leaf)
            sources.append(str(path))
        manifest = json.loads((capture / 'manifest.json').read_bytes())
        if manifest['java_pin'] != PIN:
            raise ValueError('Unexpected Java pin')
        declared = set()
        for row in manifest['files']:
            for directory in [fixture, capture]:
                path = (directory / row['path']).resolve()
                if not path.is_relative_to(directory) or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
                    raise ValueError('Binary identity differs: ' + row['path'])
            sources.append(str(capture / row['path']))
            declared.add(row['path'])
        if declared != {path.name for path in fixture.glob('*.bin')} | {'vectors.json', 'vectors.tsv'}:
            raise ValueError('Binary manifest set differs')
        rows = json.loads((capture / 'vectors.json').read_bytes())
        names = set()
        for row in rows:
            name = row['name']
            if name in names:
                raise ValueError('Duplicate Java case')
            names.add(name)
            if row['serialized_hex']:
                if bytes.fromhex(row['serialized_hex']) != (fixture / (name + '.payload.bin')).read_bytes():
                    raise ValueError('Payload differs from original Java output')
        counts[family] = len(names)
    if counts != {'control_envelopes': 9, 'merge_profile': 38}:
        raise ValueError('Java case set incomplete')
    return {'existing_java_merge_payloads': 44, 'existing_heartbeat_frames_not_formal_frame_tests': 3,
            'new_java_execution': False, 'source_pin': PIN, 'source_files': sources}


if __name__ == '__main__':
    print(json.dumps(verify()))
