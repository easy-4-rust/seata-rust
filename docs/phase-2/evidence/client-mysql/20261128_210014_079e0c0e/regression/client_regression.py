"""Record dual-Rust workspace checks and isolated lifecycle regression counterexamples."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
from java_writer_jdbc_profile_tc import ROOT, need, sha, save
from validate_client_mysql import verify


def run(packet):
    verify(packet, True)
    out = packet / 'regression'
    out.mkdir(exist_ok=False)
    env = os.environ | dict(CARGO_HOME=str(ROOT.parent / '.seata-cargo-cache'), TEMP='D:/Temp', TMP='D:/Temp',
        CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2', PYTHONUTF8='1')
    frozen = {r['path']: r['sha256'] for r in json.loads((packet / 'source-inputs.json').read_bytes())}
    frozen[Path(__file__).relative_to(ROOT).as_posix()] = sha(Path(__file__))
    save(out / 'source-inputs.json', frozen)
    shutil.copyfile(__file__, out / 'client_regression.py')
    commands = []

    def command(label, argv, expected=0, cwd=ROOT):
        print(label, flush=True)
        result = subprocess.run(list(map(str, argv)), cwd=cwd, env=env, capture_output=True,
            timeout=1200, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        commands.append(dict(label=label, argv=list(map(str, argv)), cwd=str(cwd), target=env['CARGO_TARGET_DIR'], exit_code=result.returncode))
        save(out / 'commands.json', commands)
        need(result.returncode == expected, label + ': ' + (result.stdout + result.stderr)[-3500:].decode(errors='replace'))
        return result.stdout

    work = Path('D:/codex-builds') / ('client-counterexamples-locked-' + packet.name)
    work.mkdir(exist_ok=False)
    variants = []
    variant_sources = []
    original = (ROOT / 'crates/seata_client/src/seata_client.rs').read_text(encoding='utf-8')
    mutations = [
        ('short-circuit', '    let tm = tm.await.err().map(Box::new);',
         '    let tm = tm.await.err().map(Box::new);\n    if tm.is_some() { return Err(crate::ClientShutdownError { tm, rm: None }); }',
         'seata_client::shutdown_tests::tm_failure_still_polls_rm_and_preserves_both_original_errors'),
        ('skip-drain', 'Some(rm) => rm.shutdown().await.err(),', 'Some(rm) => { drop(rm); None },',
         'seata_client::shutdown_tests::startup_preserves_tm_failure_when_rm_cleanup_cannot_drain'),
    ]
    for name, before, after, test in mutations:
        need(original.count(before) == 1, 'Counterexample mutation anchor differs')
        target = work / name
        client = target / 'crates/seata_client'
        shutil.copytree(ROOT / 'crates/seata_client', client)
        for support in ['session_peer.rs', 'tls_peer.rs']:
            dest = target / 'crates/seata_transport/tests/support' / support
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / 'crates/seata_transport/tests/support' / support, dest)
        manifest = (ROOT / 'Cargo.toml').read_text(encoding='utf-8')
        manifest = re.sub(r'^members = .*$', 'members = ["crates/seata_client"]', manifest, flags=re.M)
        manifest = re.sub(r'^exclude = .*\n', '', manifest, flags=re.M)
        manifest = manifest.replace('path = "third_party/', 'path = "' + ROOT.as_posix() + '/third_party/')
        (target / 'Cargo.toml').write_text(manifest, encoding='utf-8')
        cm = (client / 'Cargo.toml').read_text(encoding='utf-8').replace('path = "../', 'path = "' + ROOT.as_posix() + '/crates/')
        # 反例仅运行生命周期库测试；保留正式依赖，去除只服务数据库现场的 dev-dependencies。
        cm = re.sub(r'\[dev-dependencies\]\n.*?(?=\n\[lints\])',
            '[dev-dependencies]\ntokio = { workspace = true, features = ["test-util"] }\nseata-protocol = { path = "' + ROOT.as_posix() + '/crates/seata_protocol" }\n', cm, flags=re.S)
        cm = cm.replace('experimental_changed_commit = ["seata-db-mysql/experimental_changed_commit"]', 'experimental_changed_commit = []')
        (client / 'Cargo.toml').write_text(cm, encoding='utf-8')
        shutil.copyfile(ROOT / 'Cargo.lock', target / 'Cargo.lock')
        mutated = original.replace(before, after)
        (client / 'src/seata_client.rs').write_text(mutated, encoding='utf-8')
        (out / (name + '.original.rs')).write_text(original, encoding='utf-8')
        (out / (name + '.mutated.rs')).write_text(mutated, encoding='utf-8')
        for path in sorted(target.rglob('*')):
            if path.is_file() and path.name != 'Cargo.lock':
                relative = path.relative_to(target)
                snapshot = out / 'counterexamples' / name / relative
                snapshot.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, snapshot)
                variant_sources.append(dict(path=str(path), snapshot=snapshot.relative_to(out).as_posix(), sha256=sha(path)))
        variants.append((name, target, test))
    save(out / 'counterexample-inputs.json', variant_sources)
    for version in ['1.99', '1.85']:
        env['CARGO_TARGET_DIR'] = str(ROOT.parent / ('.seata-target-nodebug' if version == '1.99' else '.seata-target-nodebug-185'))
        command(version + '-workspace', ['cargo','+'+version+'.0','test','--offline','--locked','--workspace','--all-features'])
        command(version + '-clippy', ['cargo','+'+version+'.0','clippy','--offline','--locked','-p','seata-client',
            '--all-targets','--all-features','--','-D','warnings'])
        command(version + '-default', ['cargo','+'+version+'.0','test','--offline','--locked','-p','seata-client','--no-default-features'])
        for name, target, test in variants:
            # Cargo 的根包缓存不能在不同源码反例之间共用；每个版本/变体使用独立目标目录。
            env['CARGO_TARGET_DIR'] = str(work / ('target-' + version + '-' + name))
            raw = command(version + '-' + name, ['cargo','+'+version+'.0','test','--offline','--manifest-path', target / 'Cargo.toml',
                '-p','seata-client','--lib', test,'--','--exact'], expected=101, cwd=target)
            need(b'test result: FAILED. 0 passed; 1 failed;' in raw and (test + ' ... FAILED').encode() in raw,
                'Counterexample did not expose the intended lifecycle behavior')
    for name, target, _ in variants:
        shutil.copyfile(target / 'Cargo.lock', out / (name + '.Cargo.lock'))
    raw = command('client-normal-tree', ['cargo','+1.99.0','tree','--offline','--locked','-p','seata-client','--edges','normal'])
    need(not re.search(rb'\b(sqlx|rbatis|rbdc|sea-orm|seata-at|seata-db-mysql) v\d', raw), 'Client acquired a mode/ORM driver')
    need(all(sha(ROOT / name) == digest for name, digest in frozen.items()), 'Source drift during regressions')
    need(all(sha(Path(row['path'])) == row['sha256'] for row in variant_sources), 'Isolated source changed during execution')
    save(out / 'source-end.json', frozen)
    save(packet / 'catalog.json', dict(members=[dict(path=p.relative_to(packet).as_posix(), sha256=sha(p))
        for p in sorted(packet.rglob('*')) if p.is_file() and p.name != 'catalog.json']))
    verify_regressions(packet)


def verify_regressions(packet):
    verify(packet, True)
    out = packet / 'regression'
    need(sha(Path(__file__)) == sha(out / 'client_regression.py'), 'Regression tool snapshot differs')
    frozen = json.loads((out / 'source-inputs.json').read_bytes())
    need(frozen == json.loads((out / 'source-end.json').read_bytes()), 'Regression source end differs')
    for name, digest in frozen.items():
        need(sha(ROOT / name) == digest, 'Regression source changed: ' + name)
    for row in json.loads((out / 'counterexample-inputs.json').read_bytes()):
        need(sha(Path(row['path'])) == row['sha256'] == sha(out / row['snapshot']), 'Isolated executed source differs')
    rows = json.loads((out / 'commands.json').read_bytes())
    commands = {r['label']: r for r in rows}
    need(len(commands) == len(rows) == 11, 'Regression command coverage differs')
    for version in ['1.99','1.85']:
        for suffix in ['workspace','clippy','default','short-circuit','skip-drain']:
            label = version + '-' + suffix
            expected = 101 if suffix in ['short-circuit','skip-drain'] else 0
            need(commands[label]['exit_code'] == expected, 'Unexpected command exit: ' + label)
        raw = (out / (version + '-workspace.stdout')).read_bytes()
        groups = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw)
        need([sum(int(g[i]) for g in groups) for i in range(3)] == [870,0,421], 'Full workspace test coverage differs')
        need(commands[version + '-workspace']['argv'] == ['cargo','+'+version+'.0','test','--offline','--locked','--workspace','--all-features'], 'Workspace execution scope differs')
        need(commands[version + '-clippy']['argv'][-5:] == ['--all-targets','--all-features','--','-D','warnings'], 'Strict Clippy scope differs')
        for suffix in ['short-circuit','skip-drain']:
            need(b'test result: FAILED. 0 passed; 1 failed;' in (out / (version+'-'+suffix+'.stdout')).read_bytes(), 'Counterexample absent')
    normal = (out / 'client-normal-tree.stdout').read_bytes()
    need(commands['client-normal-tree']['exit_code'] == 0 and not re.search(rb'\b(sqlx|rbatis|rbdc|sea-orm|seata-at|seata-db-mysql) v\d', normal), 'Client normal graph differs')
    print('CLIENT_REGRESSIONS_VERIFIED ' + sha(packet / 'catalog.json'), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('packet', type=Path)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    (verify_regressions if args.verify_only else run)(args.packet)
