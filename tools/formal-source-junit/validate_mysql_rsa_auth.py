"""Independently bind the patched driver, raw MySQL auth, original TC and durable AT facts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from cryptography.hazmat.primitives import serialization
from java_writer_jdbc_profile_tc import ROOT, need, reg, sha, module


def verify(out):
    names = set()
    for row in json.loads((out / 'catalog.json').read_bytes())['members']:
        name = row['path']; need(not Path(name).is_absolute() and '..' not in Path(name).parts and name not in names, 'Unsafe member')
        names.add(name); need(sha(out / name) == row['sha256'], 'Catalog differs: ' + name)
    need(names == {p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file() and p.name != 'catalog.json'}, 'Catalog incomplete')
    for row in json.loads((out / 'source-inputs.json').read_bytes()):
        need(sha(ROOT / row['path']) == row['sha256'] == sha(out / 'source' / row['path']), 'Executed source differs: ' + row['path'])
    for row in json.loads((out / 'dependencies.json').read_bytes()): need(sha(Path(row['path'])) == row['sha256'], 'Original dependency differs')
    upstream = json.loads((ROOT / 'third_party/sqlx_mysql/upstream-manifest.json').read_bytes())
    changed = {r['path']: r['sha256'] for r in upstream['modified']}
    need(set(changed) == {'Cargo.toml','src/connection/auth.rs'} and upstream['crate_sha256'] == 'aa003f0038df784eb8fecbbac13affe3da23b45194bd57dba231c8f48199c526', 'Driver provenance differs')
    for row in upstream['members']: need(sha(ROOT / 'third_party/sqlx_mysql' / row['path']) == changed.get(row['path'], row['sha256']), 'Unexpected upstream code edit')
    audit = json.loads((out / 'cargo-audit.stdout').read_bytes())
    need(audit['settings']['ignore'] == [] and {v['advisory']['id'] for v in audit['vulnerabilities']['list']} == {'RUSTSEC-2026-0459','RUSTSEC-2026-0009'}, 'Audit remediation/remaining findings differ')
    need(b'\nname = "rsa"\n' not in (ROOT / 'Cargo.lock').read_bytes().replace(b'\r\n',b'\n'), 'RSA remains locked')
    normal = (out / 'workspace-normal-tree.log').read_bytes()
    need(not re.search(rb'\b(rsa|time|libsqlite3-sys|sqlx-sqlite) v\d',normal) and b'aws-lc-rs v1.19.0' in normal, 'Normal workspace activation changed')
    need((out / 'audit-db.stdout').read_text(encoding='utf-8').strip() == 'c5279d0b0c9e8b70df666eb6d1edbabcfd1176bd', 'Advisory DB differs')
    result = json.loads((out / 'result.json').read_bytes())
    need(result == dict(status='passed', versions=['1.99','1.85'], key_bits=[2048,3072,4096], globals=18, rejected=8, mocks=False,
                        tc_sha256=reg.JAR_SHA, rsa_advisory_removed=True, remaining_advisories=2), 'Native scope differs')
    commands = json.loads((out / 'commands.json').read_bytes())
    need(all(c['exit_code'] == (1 if c['label'] == 'cargo-audit' else 0) for c in commands), 'Command failed or audit hidden')
    binaries = json.loads((out / 'rust-binaries.json').read_bytes())
    for version in ['1.99','1.85']:
        need(sha(Path(binaries[version]['path'])) == binaries[version]['sha256'], 'Rust executable differs')
        need(('rustc ' + version + '.0').encode() in (out / ('rustc-' + version + '.stdout')).read_bytes(), 'Toolchain differs')
        tree = (out / ('tree-' + version + '.stdout')).read_bytes()
        need(b'rsa v' not in tree and b'aws-lc-rs v1.19.0' in tree and b'third_party' in tree, 'Actual crypto source differs')
    observations = json.loads((out / 'observations.json').read_bytes())
    expected = {(b,v,m) for b in [2048,3072,4096] for v in ['1.99','1.85'] for m in ['caching','sha256','tls']}
    expected |= {(2048,v,m) for v in ['1.99','1.85'] for m in ['bad_password','bad_pem','weak_key','wrong_ca']}
    need(len(observations) == 26 and {(o['bits'],o['version'],o['mode']) for o in observations} == expected, 'Auth matrix coverage differs')
    wire = module('rsa_auth_original_wire', ROOT / 'tools/java-go-at-shared-lock/wire.py')
    oracle = json.loads((out / 'java-oracle.json').read_bytes()); need(len(oracle['cases']) == 18 and oracle['jackson'] == '2.18.3', 'Original parser missing')
    xids = set()

    def text(label): return (out / (label + '.stdout')).read_text(encoding='utf-8').strip()
    def normalized(value):
        if isinstance(value,dict): return {k:normalized(v) for k,v in value.items() if k != '@class'}
        if isinstance(value,list):
            if len(value) == 2 and isinstance(value[0],str) and value[0].startswith('java.util.') and isinstance(value[1],list): return normalized(value[1])
            return [normalized(v) for v in value]
        return value

    for observation in observations:
        bits, version, mode = observation['bits'], observation['version'], observation['mode']
        label = str(bits) + '-' + version + '-' + mode; folder = out / label
        command = json.loads((folder / 'command.json').read_bytes())
        need(command['argv'][0] == binaries[version]['path'] and command['sha256'] == binaries[version]['sha256'], 'Executed binary differs')
        need(b'test result: ok. 1 passed;' in (folder / 'client-stdout.log').read_bytes(), 'Native test did not execute')
        proxy = json.loads((folder / 'mysql-proxy-result.json').read_bytes())
        need(proxy['connections'] > 0 and proxy['tls_translation'] is False and proxy['public_key_injection'] == (mode in ['bad_pem','weak_key']), 'Auth proxy scope differs')
        frames = json.loads((folder / 'mysql-auth-frames.json').read_bytes()); packets = []; channels = []
        for row in frames:
            if row['event'] == 'channel': channels.append(row)
            if row['event'] != 'packet': continue
            raw = bytes.fromhex(row['frame_hex'])
            need(len(raw) >= 4 and int.from_bytes(raw[:3],'little') + 4 == len(raw) and hashlib.sha256(raw).hexdigest() == row['sha256'], 'Raw MySQL envelope/hash differs')
            need(b'seata-mysql-fixture-only\0' not in raw[4:] if row['direction'] == 'client-to-server' else True, 'Plain password sent outside TLS')
            packets.append((row,raw[4:],raw[3]))
        tls = mode in ['tls','wrong_ca']; need(channels and all(r['tls'] == tls for r in channels), 'TLS mode downgraded or changed')
        public = [(r,b,s) for r,b,s in packets if r['direction'] == 'server-to-client' and b.startswith(b'\x01-----BEGIN PUBLIC KEY-----')]
        replacements = [r for r in frames if r['event'] == 'key_replacement']
        if tls:
            need(not public and not replacements and any(r['event'] == 'tls_bytes' and r['size'] > 0 for r in frames), 'TLS auth exposed RSA/plain auth packets')
        else:
            need(public, 'Cold original MySQL RSA key missing')
            for row, body, sequence in public:
                key = serialization.load_pem_public_key(body[1:]); need(key.key_size == bits, 'Original server RSA size differs')
                need(body[1:] == (out / ('public-' + str(bits) + '.pem')).read_bytes(), 'Original server key differs')
                replies = [(r,b,s) for r,b,s in packets if r['direction'] == 'client-to-server' and r['connection'] == row['connection'] and s == sequence + 1 and r['sequence'] > row['sequence']]
                if mode in ['bad_pem','weak_key']:
                    need(not replies, 'Rejected key produced a ciphertext/fallback reply')
                else:
                    need(len(replies) == 1 and len(replies[0][1]) == bits // 8, 'Original OAEP ciphertext missing/size differs')
                    reply = replies[0][0]
                    outcomes = [(r,b) for r,b,s in packets if r['direction'] == 'server-to-client' and r['connection'] == row['connection'] and s == sequence + 2 and r['sequence'] > reply['sequence']]
                    need(outcomes and outcomes[0][1][0] == (0xff if mode == 'bad_password' else 0), 'Real server did not accept/reject encrypted credential')
            if mode in ['bad_pem','weak_key']:
                need(replacements, 'Explicit negative key injection missing')
                for row in replacements:
                    raw = bytes.fromhex(row['forwarded_hex']); need(hashlib.sha256(raw).hexdigest() == row['forwarded_sha256'] and int.from_bytes(raw[:3],'little') + 4 == len(raw), 'Injected negative frame differs')
                    if mode == 'weak_key': need(serialization.load_pem_public_key(raw[5:]).key_size == 1024, 'Weak key was not 1024-bit')
                    else: need(raw[4:] == b'\x01invalid PEM', 'Bad PEM fixture changed')
        cold, = [c for c in commands if c['label'] == label + '-cold-cache']; need(cold['argv'][-1] == 'FLUSH PRIVILEGES', 'Cold cache reset missing')
        tc_frames = [wire.decode(r) for r in json.loads((folder / 'proxy-events.json').read_bytes()) if r['event'] == 'wire' and r['payload_type'] != 111]
        if observation['rejected']:
            need(json.loads((folder / 'rejected.json').read_bytes()) == dict(mode=mode,accepted=False), 'Rejected outcome differs')
            need(text(label + '-activity-before') == text(label + '-activity-after') and text(label + '-rejected-state') == '0\t0\t0\t0\n10', 'Rejected auth executed SQL or changed state')
            need(not tc_frames, 'Rejected authentication reached original TC')
            continue
        phase = json.loads((folder / 'phase.json').read_bytes()); xid, branch = phase['xid'],phase['branch_id']
        need(phase == observation['phase'] and xid not in xids and phase['checksum_persisted'] and phase['lock'] == 'auth_work:1', 'Native phase identity differs'); xids.add(xid)
        need(phase['resource'] == 'jdbc:mysql://127.0.0.1:' + command['environment']['SEATA_AUTH_PORT'] + '/admission', 'Proxy resource binding differs')
        connection = json.loads((folder / 'connection.json').read_bytes())
        need(connection == dict(mode=mode,xid=xid,resource=phase['resource']) and text(label + '-uncommitted') == '1\t0\t0\t0\n10', 'TLS sample was not from the original uncommitted transaction')
        sessions = [r.split('\t') for r in text(label + '-sessions').splitlines()]
        need(sessions and all(r[0] == ('auth_sha256' if mode == 'sha256' else 'auth_client') and ((len(r) > 1 and r[1].startswith('TLS_')) if tls else (len(r) == 1 or r[1] == '')) for r in sessions), 'Real session cipher/user differs')
        identities = text(label + '-phase-identity').splitlines()
        need(identities == ['\t'.join([xid,str(branch),phase['resource'],'2']), '\t'.join([xid,str(branch),'auth_work','1'])] and text(label + '-phase-value') == '11', 'Original branch/lock/business differs')
        undo = (folder / 'probe.undo.json').read_bytes(); raw = text(label + '-phase-undo').split('\t'); digest = hashlib.sha256(undo).hexdigest()
        need(bytes.fromhex(raw[0]) == undo and raw[1].lower() == digest and raw[2:] == ['1','1'], 'Original atomic Undo/hash/witness differs')
        graph = normalized(json.loads(undo)); need(graph['xid'] == xid and graph['branchId'] == branch and len(graph['sqlUndoLogs']) == 1, 'Original Undo identity differs')
        log = graph['sqlUndoLogs'][0]; need(log['tableName'] == 'auth_work' and log['sqlType'] == 'UPDATE', 'Original Undo SQL differs')
        for name,value in [('beforeImage',10),('afterImage',11)]:
            rows = log[name]['rows']; need(len(rows) == 1 and {f['name']:f['value'] for f in rows[0]['fields']} == dict(id=1,v=value), 'Original full image differs')
        need(graph == normalized(json.loads((folder / 'probe.undo.json.java.json').read_bytes())), 'Original Java parser changed fields')
        commit = mode == 'caching'; expected_value = 11 if commit else 10
        need(text(label + '-final-value') == str(expected_value) and observation['final']['value'] == expected_value, 'Final business differs')
        terminal = [c['label'] for c in commands if c['label'].startswith(label + '-terminal')]
        need(terminal and text(terminal[-1]) == '0\t0\t0\t0', 'TC/Normal cleanup missing')
        history = text(label + '-history').split('\t'); need(history[:4] == (['1','0','1','1'] if commit else ['2','1','0','1']) and history[4].lower() == digest, 'Durable history/checksum differs')
        registers = [m for m in tc_frames if m['code'] == 11 and m['xid'] == xid]
        need(len(registers) == 1 and registers[0]['resource'] == phase['resource'] and registers[0]['lock_key'] == phase['lock'], 'Original register differs')
        def reply(request,code):
            values = [m for m in tc_frames if m['code'] == code and m['connection'] == request['connection'] and m['id'] == request['id']]
            need(len(values) == 1 and values[0]['result'] == 1, 'Original response pairing differs'); return values[0]
        need(reply(registers[0],12)['branch_id'] == branch, 'Original registered receipt differs')
        reports = [m for m in tc_frames if m['code'] == 13 and m['xid'] == xid]
        need(len(reports) == 1 and reports[0]['branch_id'] == branch and reports[0]['status'] == 2, 'Original phase-one report differs')
        reply(reports[0],14)
        decisions = [m for m in tc_frames if m['code'] == (7 if commit else 9) and m['xid'] == xid]
        need(len(decisions) == 1 and reply(decisions[0],8 if commit else 10)['status'] in ({8,9} if commit else {11}), 'Original TM decision differs')
        code, reply_code, status = (3,4,5) if commit else (5,6,8)
        callbacks = [m for m in tc_frames if m['code'] == code and m['xid'] == xid]
        need(callbacks and all(m['branch_id'] == branch for m in callbacks), 'Original callback missing')
        for request in callbacks:
            replies = [m for m in tc_frames if m['code'] == reply_code and m['connection'] == request['connection'] and m['id'] == request['id']]
            need(len(replies) == 1 and replies[0]['xid'] == xid and replies[0]['branch_id'] == branch and replies[0]['result'] == 1 and replies[0]['status'] == status, 'Original callback reply differs')
    need(len(xids) == 18, 'Positive original transactions missing')
    components = json.loads((out / 'component-results.json').read_bytes())
    need(len(components) == 2 and {r['version'] for r in components} == {'1.99','1.85'}, 'Workspace toolchains missing')
    for run in components:
        need(run['exit_code'] == run['failed'] == 0 and run['passed'] == 840 and run['ignored'] == 415
             and run['test_argv'] == ['cargo','+'+run['version']+'.0','test','--offline','--locked','--workspace','--all-features']
             and run['clippy_argv'][-2:] == ['-D','warnings'], 'Workspace/strict checks differ')
        for row in run['logs']:
            need(sha(out / row['path']) == row['sha256'], 'Workspace log hash differs')
            raw = (out / row['path']).read_bytes()
            if row['path'].endswith('-workspace.log'):
                summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',raw)
                need([sum(int(r[i]) for r in summaries) for i in range(3)] == [840,0,415], 'Workspace actual test counts differ')
    print('MYSQL_RSA_AUTH_VERIFIED globals=18 rejected=8 ' + sha(out / 'catalog.json'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('packet',type=Path); verify(parser.parse_args().packet.resolve())
