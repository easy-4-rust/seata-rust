"""Add formal Rust contenders to the original Java complete-profile fixture."""
import argparse
import json
from pathlib import Path
import subprocess
import time

from java_writer_jdbc_profile_tc import ROOT, need, put, save, sha, support, wait, run


class MixedRust:
    def __init__(self, work, out, env, command, versions):
        self.work, self.out, self.env, self.command = work, out, env, command
        self.versions = versions
        self.binaries, self.active = {}, []
        for version in versions:
            env.update(CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_INCREMENTAL='0',
                       CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2',
                       CARGO_TARGET_DIR=str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185')))
            command('rustc-' + version, ['rustc', '+' + version + '.0', '--version'])
            argv = ['cargo', '+' + version + '.0', 'test', '--manifest-path', ROOT / 'Cargo.toml', '--offline',
                    '--locked', '-p', 'seata-db-mysql', '--test', 'live_exact_temporal_mixed_java',
                    '--no-run', '--message-format=json']
            result = command('cargo-' + version, argv, timeout=240)
            artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith(b'{')]
            original, = [Path(row['executable']) for row in artifacts if row.get('executable')
                         and row['target']['name'] == 'live_exact_temporal_mixed_java']
            target = work / ('rust-' + version + '.exe'); put(target, original.read_bytes())
            self.binaries[version] = dict(path=str(target), sha256=sha(target), original=str(original))
        save(out / 'rust-binaries.json', self.binaries)

    def inputs(self):
        return [Path(__file__).resolve(), ROOT / 'Cargo.toml', ROOT / 'Cargo.lock',
                *sorted(p for p in (ROOT / 'crates').rglob('*') if p.is_file())]

    def held(self, profile, label, case, listener, db_port, sql, phase, business):
        need(not self.active, 'Previous mixed case still active')
        for version in self.versions:
            folder = case / ('rust-' + version); folder.mkdir()
            binary = Path(self.binaries[version]['path'])
            need(sha(binary) == self.binaries[version]['sha256'], 'Rust executable drift')
            args = [binary, 'original_java_and_rust_share_exact_temporal_locks', '--ignored', '--exact', '--nocapture']
            env = self.env | dict(SEATA_MYSQL_PORT=str(db_port), SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only',
                SEATA_MYSQL_DATABASE='admission', SEATA_MIXED_TEMPORAL_PROFILE=str(profile),
                SEATA_MIXED_TEMPORAL_OUT=str(folder), SEATA_MIXED_RUST_VERSION=version,
                SEATA_RM_ENDPOINT='127.0.0.1:' + str(listener), SEATA_TM_ENDPOINT='127.0.0.1:' + str(listener))
            save(folder / 'command.json', dict(argv=list(map(str, args)), executable_sha256=sha(binary)))
            process = support.spawn(args, ROOT, env, folder, 'client')
            self.active.append((version, folder, process))
        for version, folder, process in self.active:
            wait((folder / 'conflicts.json').exists, process, label + ' Rust ' + version + ' conflicts')
            conflicts = json.loads((folder / 'conflicts.json').read_bytes())
            need(conflicts['profile'] == profile and conflicts['version'] == version
                 and conflicts['resource'] == phase['resource']
                 and {c['case'] for c in conflicts['cases']} == {'date', 'time'}, 'Rust conflict coverage differs')
        raw = self.command(label + '-mixed-held-tc', sql + [
            'SELECT xid,branch_id,resource_id,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table ORDER BY table_name,pk']).stdout
        need(raw == (self.out / (label + '-phase-tc.stdout')).read_bytes(), 'Rust conflict changed original Java locks')
        current = self.command(label + '-mixed-held-business', sql + [self.business_sql()]).stdout
        need(current == business, 'Rejected Rust transaction leaked business changes')

    def released(self, label, case, sql, business):
        for version, folder, process in self.active:
            put(folder / 'java-released', 'Original Java terminal observed')
            for name, table, pk in [('date', 'temporal_dates', '1582-10-05'),
                                    ('time', 'temporal_times', '-25:01:02.123456')]:
                wait((folder / ('admitted-' + name + '.json')).exists, process, label + ' Rust admitted ' + name)
                observation = json.loads((folder / ('admitted-' + name + '.json')).read_bytes())
                prefix = label + '-rust-' + version + '-' + name
                rows = self.command(prefix + '-tc', sql + [
                    'SELECT xid,branch_id,resource_id,status FROM branch_table; SELECT xid,branch_id,table_name,pk FROM lock_table']).stdout.decode().splitlines()
                need(rows == ['\t'.join([observation['xid'], str(observation['branch_id']), observation['resource'], '1']),
                     '\t'.join([observation['xid'], str(observation['branch_id']), table, pk])], 'Admitted Rust TC identity differs')
                need(observation['lock'] == table + ':' + pk, 'Rust exact lock spelling differs')
                actual = self.command(prefix + '-business', sql + [self.business_sql()]).stdout
                need(actual == business, 'Prepared Rust transaction committed business')
                put(folder / ('admitted-' + name + '.ack'), 'Independent TC lock observed')
            wait((folder / 'finished.json').exists, process, label + ' Rust completion')
            need(process.wait(timeout=10) == 0, 'Rust client failed')
            need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Rust live test did not execute')
            counts = self.command(label + '-rust-' + version + '-terminal', sql + [
                'SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)']).stdout
            need(counts.strip() == b'0\t0\t0\t0', 'Rust terminal state remains')
            need(self.command(label + '-rust-' + version + '-final', sql + [self.business_sql()]).stdout == business,
                 'Rust rollback changed original Java result')
        self.active.clear()

    @staticmethod
    def business_sql():
        return ('SELECT id,v FROM admission_business ORDER BY id; SELECT id,CAST(d AS CHAR),CAST(t AS CHAR) FROM temporal_values ORDER BY id; '
                'SELECT CAST(k AS CHAR),v FROM temporal_dates ORDER BY k; SELECT CAST(k AS CHAR),v FROM temporal_times ORDER BY k')

    def close(self):
        for _, _, process in self.active:
            support.stop(process)
        self.active.clear()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--profiles', nargs='+', type=int, default=list(range(1, 7)))
    parser.add_argument('--directions', nargs='+', choices=['commit', 'rollback'], default=['commit', 'rollback'])
    parser.add_argument('--versions', nargs='+', choices=['1.99', '1.85'], default=['1.99', '1.85'])
    options = parser.parse_args()
    run(options.profiles, options.directions, options.versions)
