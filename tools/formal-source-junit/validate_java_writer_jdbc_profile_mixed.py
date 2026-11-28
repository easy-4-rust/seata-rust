"""Verify actual shared temporal keys, original TC conflicts, admission and GlobalFinished fences."""
import argparse
import json
from pathlib import Path
from validate_java_writer_jdbc_profile_tc import ROOT, module, need, sha, verify


def mixed(out, full):
    verify(out, full)
    result = json.loads((out / 'result.json').read_bytes())
    versions = result['mixed_rust_versions']
    need(versions == ['1.99', '1.85'], 'Both Rust toolchains required')
    binaries = json.loads((out / 'rust-binaries.json').read_bytes())
    for version in versions:
        need(sha(Path(binaries[version]['path'])) == binaries[version]['sha256'], 'Native Rust executable drift')
        need(('rustc ' + version + '.0').encode() in (out / ('rustc-' + version + '.stdout')).read_bytes(), 'Rust toolchain differs')
    decoder = module('temporal_mixed_evidence_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    counts = dict(rejected=0, admitted=0)
    xids = set()
    for profile in result['profiles']:
        if profile < 5: continue
        for direction in result['directions']:
            label = str(profile) + '-' + direction; case = out / label
            java = json.loads((case / 'phase-one.json').read_bytes())
            xids.add(java['xid'])
            need((out / (label + '-mixed-held-tc.stdout')).read_bytes() == (out / (label + '-phase-tc.stdout')).read_bytes(),
                 'Rejected Rust contender changed Java locks')
            need((out / (label + '-mixed-held-business.stdout')).read_bytes() == (out / (label + '-phase-business.stdout')).read_bytes(),
                 'Rejected Rust contender leaked local changes')
            rows = json.loads((case / 'proxy-events.json').read_bytes())
            frames = [(r, decoder.decode(r)) for r in rows if r['event'] == 'wire']
            markers = set()
            for version in versions:
                folder = case / ('rust-' + version)
                command = json.loads((folder / 'command.json').read_bytes())
                need(command['executable_sha256'] == binaries[version]['sha256'] and command['argv'][0] == binaries[version]['path'],
                     'Executed Rust binary differs')
                need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Native Rust test missing')
                finished = json.loads((folder / 'finished.json').read_bytes())
                need(finished == dict(version=version, profile=profile, conflicts=2, admitted=2), 'Rust coverage differs')
                conflicts = json.loads((folder / 'conflicts.json').read_bytes())
                need(conflicts['version'] == version and conflicts['profile'] == profile
                     and conflicts['resource'] == java['resource'] and {c['case'] for c in conflicts['cases']} == {'date', 'time'},
                     'Native conflict identity differs')
                for name, table, pk in [('date', 'temporal_dates', '1582-10-05'), ('time', 'temporal_times', '-25:01:02.123456')]:
                    lock = table + ':' + pk
                    conflict, = [c for c in conflicts['cases'] if c['case'] == name]
                    admitted = json.loads((folder / ('admitted-' + name + '.json')).read_bytes())
                    for rejected, observation in [(True, conflict), (False, admitted)]:
                        xid = observation['xid']; need(xid not in xids, 'Mixed XID reused'); xids.add(xid)
                        need(observation['lock'] == lock, 'Exact lock spelling differs')
                        registrations = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == 11 and v['xid'] == xid]
                        need(len(registrations) == 1, 'Rust branch registration missing')
                        request = registrations[0]
                        need(request['resource'] == java['resource'] and request['lock_key'] == lock and request['version'] == 1,
                             'Rust native lock request differs')
                        replies = [v for r, v in frames if r['direction'] == 'tc-to-sdk' and v['code'] == 12
                                   and v['connection'] == request['connection'] and v['id'] == request['id']]
                        need(len(replies) == 1, 'Original Rust registration reply missing')
                        if rejected:
                            need(replies[0]['result'] == 0 and replies[0]['error'] == conflict['exception'], 'Not an original TC rejection')
                            counts['rejected'] += 1
                        else:
                            branch = admitted['branch_id']; markers.add((xid, branch))
                            need(replies[0]['result'] == 1 and replies[0]['branch_id'] == branch, 'Original TC admission differs')
                            prefix = label + '-rust-' + version + '-' + name
                            expected = ['\t'.join([xid, str(branch), java['resource'], '1']), '\t'.join([xid, str(branch), table, pk])]
                            need((out / (prefix + '-tc.stdout')).read_text().splitlines() == expected, 'Independent Rust lock rows differ')
                            need((out / (prefix + '-business.stdout')).read_bytes() == (out / (label + '-final-business.stdout')).read_bytes(),
                                 'Uncommitted Rust changes became visible')
                            callbacks = [v for r, v in frames if r['direction'] == 'tc-to-sdk' and v['code'] == 5 and v['xid'] == xid]
                            need(len(callbacks) == 1 and callbacks[0]['branch_id'] == branch, 'Original TC rollback missing')
                            callback = callbacks[0]
                            completed = [v for r, v in frames if r['direction'] == 'sdk-to-tc' and v['code'] == 6
                                         and v['connection'] == callback['connection'] and v['id'] == callback['id']]
                            need(len(completed) == 1 and completed[0]['result'] == 1 and completed[0]['status'] == 8
                                 and completed[0]['xid'] == xid and completed[0]['branch_id'] == branch, 'Rust rollback callback differs')
                            counts['admitted'] += 1
                terminal = (out / (label + '-rust-' + version + '-terminal.stdout')).read_bytes().strip()
                need(terminal == ('0\t0\t0\t' + str(len(markers))).encode(), 'Rust terminal counts differ')
                fences = [line.split('\t') for line in (out / (label + '-rust-' + version + '-fences.stdout')).read_text().splitlines()]
                need(len(fences) == len(markers) and {(r[0], int(r[1])) for r in fences} == markers
                     and all(r[2] == '1' for r in fences), 'GlobalFinished fences differ')
                need((out / (label + '-rust-' + version + '-final.stdout')).read_bytes() == (out / (label + '-final-business.stdout')).read_bytes(),
                     'Rust rollback changed Java outcome')
    expected = len([p for p in result['profiles'] if p >= 5]) * len(result['directions']) * len(versions) * 2
    need(counts == dict(rejected=expected, admitted=expected), 'Mixed key coverage differs')
    print('JAVA_RUST_TEMPORAL_MIXED_VERIFIED ' + json.dumps(counts) + ' ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet', type=Path); parser.add_argument('--full', action='store_true')
    args = parser.parse_args(); mixed(args.packet, args.full)
