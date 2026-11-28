"""Audit original retention frames, native inputs, real database observations and channel reuse."""
import argparse
import hashlib
import json
from pathlib import Path
from java_writer_jdbc_profile_tc import ROOT, module, need, reg, sha


def verify(out):
    names = set()
    for row in json.loads((out / 'catalog.json').read_bytes())['members']:
        name = row['path']; need(not Path(name).is_absolute() and '..' not in Path(name).parts and name not in names, 'Unsafe member')
        names.add(name); need(sha(out / name) == row['sha256'], 'Catalog hash differs: ' + name)
    need(names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'}, 'Catalog incomplete')
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Executed source differs')
    for row in json.loads((out / 'dependencies.json').read_bytes()): need(sha(Path(row['path'])) == row['sha256'], 'Java runtime differs')
    result = json.loads((out / 'result.json').read_bytes())
    need(result['status'] == 'passed' and result['versions'] == ['1.99', '1.85'] and result['mocks'] is False
         and result['tc_jar_sha256'] == reg.JAR_SHA and result['deleted_per_rust'] == 6107 and result['java_days'] == [7, 0, -1], 'Native scope differs')
    workspace = json.loads((out / 'workspace-results.json').read_bytes())
    need({r['version'] for r in workspace} == {'1.99', '1.85'} and len(workspace) == 2, 'Workspace toolchains missing')
    for run in workspace:
        need(run['exit_code'] == run['failed'] == 0 and run['passed'] == 831 and run['ignored'] == 413
             and sha(out / run['log']) == run['sha256'] and run['argv'] == ['cargo', '+' + run['version'] + '.0', 'test', '--offline', '--locked', '--workspace', '--all-features'], 'Workspace regression differs')
    commands = json.loads((out / 'commands.json').read_bytes()); need(all(c['exit_code'] == 0 for c in commands), 'Command failed')
    def values(prefix): return [(out / (c['label'] + '.stdout')).read_bytes() for c in commands if c['label'].startswith(prefix + '-')]
    for days in (7, 0, -1):
        oracle = json.loads((out / ('java-' + str(days) + '.json')).read_bytes())
        need(oracle['days'] == days and oracle['batches'] == [3000, 3000, 107], 'Original batching/default differs')
    keep = ['keep-micro', 'keep-noon', 'keep-normal', 'keep-young-fence']
    need(len(values('java-kept')) == 3 and all(v.decode().splitlines() == keep for v in values('java-kept')), 'Java boundaries differ')
    binaries = json.loads((out / 'rust-binaries.json').read_bytes())
    observations = json.loads((out / 'observations.json').read_bytes()); need(len(observations) == 2, 'Missing Rust version')
    wire = module('retention_wire_audit', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    for observation in observations:
        version = observation['version']; folder = out / version; probe = observation['probe']
        need(version in ['1.99', '1.85'] and observation['expired_deleted'] == 6107 and observation['remaining'] == keep
             and observation['binding_rejected'] and observation['trigger_rejected'] and probe['value'] == 10, 'Rust observation differs')
        need(sha(Path(binaries[version]['path'])) == binaries[version]['sha256'], 'Native executable differs')
        need(any(('rustc ' + version + '.0').encode() in v for v in values('rustc-' + version)), 'Toolchain differs')
        need(any(c['label'].startswith('clippy-' + version + '-') and c['argv'][-2:] == ['-D', 'warnings'] for c in commands), 'Strict Clippy missing')
        for prefix in ('rm-tests-', 'db-tests-'):
            need(any(b'test result: ok.' in v for v in values(prefix + version)), 'Component tests missing')
        command = json.loads((folder / 'command.json').read_bytes())
        need(command['argv'][0] == binaries[version]['path'] and command['sha256'] == binaries[version]['sha256'], 'Executed binary differs')
        need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Native test missing')
        need(json.loads((folder / 'cleaned.json').read_bytes())['remaining'] == keep, 'Actual Rust cleanup differs')
        decoded = []
        for row in json.loads((folder / 'proxy-events.json').read_bytes()):
            if row['event'] != 'wire': continue
            raw = bytes.fromhex(row['frame_hex']); need(hashlib.sha256(raw).hexdigest() == row['sha256'], 'Raw wire hash differs')
            if row['payload_type'] == 111:
                head = int.from_bytes(raw[7:9], 'big')
                need(raw[:3] == b'\xda\xda\x01' and int.from_bytes(raw[3:7], 'big') == len(raw) and head == 16
                     and raw[9:12] == b'\x02\x01\x00' and row['direction'] == 'tc-to-sdk'
                     and int.from_bytes(raw[12:16], 'big', signed=True) == row['id'], 'Original oneway envelope differs')
                reader = wire.Reader(raw[head + 2:]); branch = reader.integer(1); resource = reader.string(); days = reader.integer(2, True); reader.end()
                need(branch == 0 and resource == probe['resource'] and days == 7, 'Cleanup fields differ')
                message = dict(code=111)
            else: message = wire.decode(row)
            decoded.append((row, message))
        cleanup = [(r, m) for r, m in decoded if m['code'] == 111]; need(len(cleanup) >= 4, 'Scheduled cleanup coverage missing')
        for name, expected in [('binding-rejected.json', 6111), ('trigger-rejected.json', 1)]:
            rejection = json.loads((folder / name).read_bytes()); need(rejection['count'] == expected and rejection['frames'], 'Refusal evidence missing')
            need(all(r['payload_type'] == 111 for r in rejection['frames']), 'Refusal was not after original TC message')
        callbacks = [(r, m) for r, m in decoded if m['code'] == 5 and m['xid'] == probe['xid'] and m['branch_id'] == probe['branch_id']]
        replies = [(r, m) for r, m in decoded if m['code'] == 6 and m['xid'] == probe['xid'] and m['branch_id'] == probe['branch_id'] and m['result'] == 1 and m['status'] == 8]
        need(callbacks and replies, 'Real branch rollback/reply missing')
        connection = callbacks[0][0]['connection']
        need(all(r['connection'] == connection for r, _ in cleanup + callbacks + replies), 'RM connection changed during cleanup/refusal/callback')
        need(any(r['connection'] == connection and m['code'] == 103 for r, m in decoded), 'Original RM registration missing')
        cleanup_ids = {r['id'] for r, _ in cleanup}
        need(not any(r['direction'] == 'sdk-to-tc' and r['kind'] == 1 and r['id'] in cleanup_ids for r, _ in decoded), 'Oneway cleanup emitted response')
        need(any(m['code'] == 11 and m['xid'] == probe['xid'] and m['resource'] == probe['resource'] for _, m in decoded), 'Real branch registration missing')
    need(values('tc-terminal')[-1].strip() == b'0\t0\t0' and all(v.strip() == b'10' for v in values('probe-value')), 'Independent terminal state differs')
    print('UNDO_RETENTION_AUDIT_PASS ' + str(out))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('out', type=Path); verify(parser.parse_args().out.resolve())
