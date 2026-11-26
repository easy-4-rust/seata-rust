"""Export failure diagnostics; detecting a known divergence is not interoperability certification."""
from pathlib import Path
import argparse
import hashlib
import json
import re


def export_fixtures(source: Path, destination: Path) -> dict:
    raw = source.read_bytes()
    rows = json.loads(raw)
    if not isinstance(rows, list) or len(rows) != 7:
        raise ValueError('Expected seven failure diagnostic cases')
    outputs, entries, names = {}, [], set()
    compatible = divergent = 0
    for row in rows:
        if not isinstance(row, dict): raise ValueError('Case must be an object')
        name, kind = row.get('name'), row.get('type_id')
        if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z][A-Za-z0-9]*', name):
            raise ValueError('Unsafe case name')
        if name.casefold() in names: raise ValueError('Case-insensitive duplicate case name')
        names.add(name.casefold())
        if type(kind) is not int or not 1 <= kind <= 65535: raise ValueError('Invalid message type')
        status = row.get('compatibility')
        if status not in ('byte_equal', 'error_text_length', 'exception_code_ordinal'):
            raise ValueError('Unknown compatibility classification')
        entry = {'name': name, 'type_id': kind, 'compatibility': status, 'input': row.get('input'), 'payloads': {}}
        payloads = {}
        for language in ('java', 'go'):
            for field in ('body_hex', 'serialized_hex'):
                value = row.get(language + '_' + field)
                if not isinstance(value, str) or not re.fullmatch(r'(?:[0-9a-fA-F]{2})+', value):
                    raise ValueError('Invalid hex bytes')
            body = bytes.fromhex(row[language + '_body_hex'])
            payload = bytes.fromhex(row[language + '_serialized_hex'])
            if payload != kind.to_bytes(2, 'big') + body: raise ValueError('Wrong type prefix/body')
            filename = name + '.' + language + '.bin'
            payloads[language] = payload
            outputs[filename] = payload
            entry['payloads'][language] = {'file': filename, 'bytes': len(payload),
                                           'sha256': hashlib.sha256(payload).hexdigest()}
        if (payloads['java'] == payloads['go']) != (status == 'byte_equal'):
            raise ValueError('Bytes contradict compatibility classification')
        if status == 'byte_equal': compatible += 1
        else: divergent += 1
        entries.append(entry)
    if (compatible, divergent) != (4, 3): raise ValueError('Expected four compatible and three open divergent cases')
    manifest = {'format': 'seata_v1_failure_diagnostics_v1',
                'java_source': 'cba73e180fd8896a99df6415b859d275122feaa7',
                'go_source': '82570d905919c1c66bb025a3a5979f25390de498',
                'protocol_version': 1, 'serializer_id': 1, 'compressor_id': 0,
                'source_json_lf_sha256': hashlib.sha256(raw.replace(b'\r\n', b'\n')).hexdigest(),
                'source_hash_normalization': 'CRLF to LF for JSON provenance only; payload bytes unchanged',
                'sample_count': len(entries), 'compatible_cases': compatible, 'open_divergences': divergent,
                'samples': entries, 'boundary': 'Java authority; Go differences remain OPEN, not interoperability passes'}
    outputs['manifest.json'] = (json.dumps(manifest, indent=2) + '\n').encode()
    for name, data in outputs.items():
        path = destination / name
        if path.exists() and path.read_bytes() != data: raise ValueError('Refusing to overwrite different artifact: ' + str(path))
    destination.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        path = destination / name
        try:
            with path.open('xb') as stream: stream.write(data)
        except FileExistsError:
            if path.read_bytes() != data: raise ValueError('Artifact changed during export: ' + str(path))
    return manifest


if __name__ == '__main__':
    repo = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=repo / 'tools/go-failure-variants/expected.json')
    parser.add_argument('--destination', type=Path, default=repo / 'tests/fixtures/protocol/v1/failure_variants')
    options = parser.parse_args()
    result = export_fixtures(options.source, options.destination)
    print(f"Exported {result['sample_count']} pairs: {result['compatible_cases']} compatible, {result['open_divergences']} OPEN divergences")
