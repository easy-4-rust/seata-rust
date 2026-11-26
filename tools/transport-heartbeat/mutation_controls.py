"""隔离副本里的行为破坏负控；从不改写正式源码或声称这些是原始产品缺陷。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid


def run(root, out, env):
    candidate = Path('D:/codex-builds') / ('seata-heartbeat-controls-' + uuid.uuid4().hex[:12])
    candidate.mkdir()
    for leaf in ['Cargo.toml', 'Cargo.lock']:
        shutil.copyfile(root / leaf, candidate / leaf)
    for package in ['seata_protocol', 'seata_transport']:
        shutil.copytree(root / 'crates' / package, candidate / 'crates' / package)
    source = candidate / 'crates/seata_transport/src/netty/tcp_connection.rs'
    original = source.read_text(encoding='utf-8')
    # 每个mutation使用完全相同的已验证基线，只改变一处活动刷新规则。
    cases = [
        ('partial_tcp_bytes_refresh_reader',
         '        let mut input = &buffer[..count];',
         '        idle.reset();\n        let mut input = &buffer[..count];',
         ['--test', 'heartbeat_idle'], 'partial_frame_drips_cannot_keep_a_generation_alive'),
        ('business_writes_do_not_refresh_writer',
         '                if written {\n                    idle.reset();',
         '                if written {',
         ['--test', 'heartbeat_idle'], 'sustained_business_writes_reset_writer_idle_and_suppress_probes'),
        ('pong_writes_do_not_refresh_writer',
         '                if written {\n                    idle.reset();',
         '                if written {',
         ['--test', 'heartbeat_idle'], 'outbound_pong_replies_reset_writer_idle_and_suppress_own_probes'),
        ('skipped_requests_refresh_writer',
         '                        return Ok(false);',
         '                        return Ok(true);',
         ['--lib'], 'netty::tcp_connection::tests::skipped_cancelled_queue_entries_do_not_reset_writer_idle'),
    ]
    manifest = []
    try:
        for name, old, new, target, test in cases:
            if original.count(old) != 1:
                raise ValueError('Mutation anchor must be unique: ' + name)
            mutated = original.replace(old, new)
            source.write_text(mutated, encoding='utf-8', newline='\n')
            result = subprocess.run(['cargo', 'test', '--offline', '--locked', '-p', 'seata-transport', *target, test, '--', '--exact'],
                                    cwd=candidate, env=env | dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-heartbeat-controls'),
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    creationflags=subprocess.CREATE_NO_WINDOW, timeout=60)
            (out / (name + '.log')).write_bytes(result.stdout)
            if result.returncode != 101 or b'1 failed' not in result.stdout or (test + ' ... FAILED').encode() not in result.stdout:
                raise ValueError('Behavior mutation was not rejected by its actual test: ' + name)
            manifest.append(dict(name=name, candidate=str(candidate), source_sha256=hashlib.sha256(mutated.encode()).hexdigest(),
                                 source_patch=dict(old=old, new=new), test=test, exit_code=result.returncode,
                                 actual_runtime_assertion_failed=True, original_product_defect=False))
    finally:
        source.write_text(original, encoding='utf-8', newline='\n')
    (out / 'mutation-controls.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return manifest
