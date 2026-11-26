"""Resolve selected SQLx dependencies offline using checksum-verified existing public C: cache, writing D:/E: only."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path('C:/Users/hnxyh/.cargo/registry')
TARGET = Path('D:/codex-builds/seata-rust-cargo-home/registry')
INDEX = 'index.crates.io-1949cf8c6b5b557f'
OUT = Path('D:/codex-builds/seata-rust-protocol-start-20261113')


def copy_package(name, copies):
    key = ('1/' + name if len(name) == 1 else '2/' + name if len(name) == 2
        else '3/' + name[0] + '/' + name if len(name) == 3 else name[:2] + '/' + name[2:4] + '/' + name)
    source = SOURCE / 'index' / INDEX / '.cache' / key
    if not source.is_file():
        raise ValueError('Existing public cache has no index for ' + name)
    versions = {}
    for part in source.read_bytes().split(b'\0'):
        if part.startswith(b'{'):
            row = json.loads(part)
            versions[row['vers']] = row['cksum']
    for package in tomllib.loads((ROOT / 'Cargo.lock').read_text('utf-8'))['package']:
        if package['name'] == name and package.get('checksum'):
            if versions.get(package['version']) != package['checksum']:
                raise ValueError('Current pinned package absent/different in public index')
    destination = TARGET / 'index' / INDEX / '.cache' / key
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    count = 0
    for source in (SOURCE / 'cache' / INDEX).glob(name + '-*.crate'):
        version = source.name[len(name) + 1:-6]
        if version not in versions:
            continue
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        if digest != versions[version]:
            raise ValueError('Public cached archive checksum differs')
        destination = TARGET / 'cache' / INDEX / source.name
        if destination.exists():
            if hashlib.sha256(destination.read_bytes()).hexdigest() != digest:
                raise ValueError('Existing D archive differs')
        else:
            shutil.copyfile(source, destination)
            copies.append(dict(name=name, version=version, checksum=digest))
        count += 1
    print('PUBLIC_CACHE ' + name + ' ' + str(count), flush=True)


if __name__ == '__main__':
    env = os.environ | dict(CARGO_HOME=str(TARGET.parent), CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',
        TEMP='D:/Temp', TMP='D:/Temp', CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS='fallback')
    copies, attempted = [], set()
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text('utf-8'))['workspace']['members']
    local_names = {tomllib.loads((ROOT / path / 'Cargo.toml').read_text('utf-8'))['package']['name'] for path in workspace}
    # serde的新版本将运行核心拆包；离线求解器可能先报告derive冲突，尚未显示缺失的core。
    for name in ['serde', 'serde_core', 'serde_derive', 'serde_json']:
        copy_package(name, copies)
    for attempt in range(220):
        result = subprocess.run(['cargo', 'metadata', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'],
            cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
        if result.returncode == 0:
            (OUT / 'mysql-dependency-metadata.json').write_bytes(result.stdout)
            (OUT / 'mysql-dependency-resolution.log').write_bytes(result.stderr)
            break
        error = result.stderr.decode('utf-8', 'replace')
        (OUT / 'mysql-dependency-last-error.log').write_text(error, encoding='utf-8')
        names = re.findall(r'no matching package named `([^`]+)` found|failed to select a version for `([^`]+)`|failed to download `([^ ]+) v[^`]+`|package `([^ ]+) v[^`]+`', error)
        names = list(dict.fromkeys(value for group in names for value in group if value and value not in attempted and value not in local_names))
        if not names:
            raise ValueError(error[-4000:])
        for name in names:
            attempted.add(name)
            copy_package(name, copies)
    else:
        raise ValueError('Offline dependency resolution attempt budget exceeded')
    (OUT / 'mysql-public-cache-copies.json').write_text(json.dumps(copies, indent=2) + '\n', encoding='utf-8')
    print('METADATA_READY copied=' + str(len(copies)), flush=True)
