"""Run native TLS events and reject removal of the actual callback write settlement."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile

from inventory_rebuilt import ROOT, need, save, sha


def run():
    out = ROOT / 'docs/phase-2/evidence/transport-tls' / ('native_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir(parents=True)
    print('TRANSPORT_TLS_NATIVE ' + str(out), flush=True)
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp', CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0')
    files = [p for p in (ROOT / 'crates/seata_transport').rglob('*') if p.is_file()]
    sources = []
    with zipfile.ZipFile(out / 'source-inputs.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted(files):
            path = p.relative_to(ROOT).as_posix()
            archive.writestr(path, p.read_bytes())
            sources.append(dict(path=path, sha256=sha(p)))
    save(out / 'source-inputs.json', sources)
    upstream = ROOT.parent / '.seata-cargo-cache/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-rustls-0.26.4/src/common/mod.rs'
    save(out / 'upstream-flush-contract.json', dict(package='tokio-rustls',version='0.26.4',
        source_sha256=sha(upstream),poll_write_line=279,poll_flush_line=352,
        accepted_plaintext_may_remain_buffered=True,original_Cargo_lock_sha256=sha(ROOT/'Cargo.lock')))

    commands = []

    def execute(version, label, arguments, expected=0):
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        argv = ['cargo', '+' + version + '.0', *arguments]
        result = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (version + '-' + label + '.stdout')).write_bytes(result.stdout)
        (out / (version + '-' + label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(Rust=version, label=label, argv=argv, exit_code=result.returncode))
        save(out / 'commands.json', commands)
        need(result.returncode == expected, 'Native TLS event result differs: ' + label + '\n' + (result.stdout + result.stderr)[-2000:].decode(errors='replace'))
        return result

    target = ROOT / 'crates/seata_transport/src/netty/connection_pool.rs'
    original = target.read_bytes()
    needle = b'TcpConnection::connect_tls(core.config.endpoint, core.config.connection, tls)'
    need(original.count(needle) == 1, 'Actual callback write settlement target ambiguous')
    mutated = original.replace(needle, b'{ let _ = tls; TcpConnection::connect(core.config.endpoint, core.config.connection) }')
    for version in ['1.99', '1.85']:
        execute(version, 'rm-tests', ['test', '-p', 'seata-transport', '--features', 'tls', '--test', 'tls_transport', '--offline', '--locked'])
        arguments = ['test', '-p', 'seata-transport', '--features', 'tls', '--test', 'tls_transport', 'registered_pool_reconnects_with_tls_before_repeating_only_session_registration', '--offline', '--locked', '--', '--exact']
        try:
            target.write_bytes(mutated)
            result = execute(version, 'mutation', arguments, expected=101)
            need(b'PoolAcquireTimeout' in result.stdout and b'could not compile' not in result.stderr, 'TLS bypass did not fail at actual registered pool runtime')
            save(out / (version + '-mutation.json'), dict(path=target.relative_to(ROOT).as_posix(), original_sha256=__import__('hashlib').sha256(original).hexdigest(), mutation_sha256=sha(target), TLS_pool_path_bypassed=True))
        finally:
            target.write_bytes(original)
        execute(version, 'restored', arguments)
        write_target = ROOT / 'crates/seata_transport/src/netty/tcp_connection.rs'
        write_original = write_target.read_bytes()
        flush = b'            writer.flush().await?;'
        need(write_original.count(flush) == 1, 'Actual TLS flush target ambiguous')
        flush_arguments = ['test', '-p', 'seata-transport', '--features', 'tls', '--lib',
            'buffered_response_is_not_complete_until_flush_drains_the_original_writer', '--offline', '--locked']
        try:
            write_target.write_bytes(write_original.replace(flush, b'            let _ = &writer;'))
            result = execute(version, 'flush-mutation', flush_arguments, expected=101)
            need(b'buffered plaintext acceptance cannot publish a completed response' in result.stdout,
                 'Missing flush did not fail at actual buffered write boundary')
        finally:
            write_target.write_bytes(write_original)
        execute(version, 'restored-library', ['test', '-p', 'seata-transport', '--features', 'tls', '--lib', '--offline', '--locked'])

    for row in sources:
        need(sha(ROOT / row['path']) == row['sha256'], 'Final native TLS source drift')
    save(out / 'result.json', dict(status='NATIVE_MTLS_AND_PLAINTEXT_BYPASS_MUTATION_VERIFIED', Rust_versions=['1.99', '1.85'], mutation_rejected_per_version=True, missing_flush_rejected_per_version=True, restored_target_passed_per_version=True, WP07_complete=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    run()
