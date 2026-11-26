"""Restore only locked public dependency cache bytes after temporary D: cleanup; never install tools."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT=Path(__file__).resolve().parents[2]
EVIDENCE=ROOT/'docs/phase-2/evidence/mysql-image-race-gate/run_20261114_052627_abb1eddd'
CACHE=Path('D:/codex-builds/seata-rust-cargo-home')
PUBLIC=Path('C:/Users/hnxyh/.cargo/registry')
INDEX='index.crates.io-1949cf8c6b5b557f'
OUT=Path('D:/codex-builds/seata-cache-restoration-20261115')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def need(value,message):
    if not value:raise ValueError(message)


def index_key(name):
    return '1/'+name if len(name)==1 else '2/'+name if len(name)==2 else '3/'+name[0]+'/'+name if len(name)==3 else name[:2]+'/'+name[2:4]+'/'+name


def entries(data):
    return {row['vers']:row for part in data.split(b'\0') if part.startswith(b'{') for row in [json.loads(part)]}


def run():
    need(sha((EVIDENCE/'catalog.json').read_bytes())=='cac7531795b6296a541b81c61841e9c2552846b87539079dc7f412225d79311a','Archived manifest identity differs')
    snapshots={Path(row['path']):row for row in json.loads((EVIDENCE/'source-inputs.json').read_text('utf-8'))}
    registry=[row for row in tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))['package'] if row.get('source')]
    planned=[]
    for package in registry:
        need(package['source']=='registry+https://github.com/rust-lang/crates.io-index','Unexpected non-public registry')
        destination=CACHE/'registry/cache'/INDEX/(package['name']+'-'+package['version']+'.crate')
        need(destination.resolve().is_relative_to(CACHE.resolve()) and destination.drive.upper()=='D:','Cache path must stay in dedicated D directory')
        record=snapshots[destination];payload=(EVIDENCE/record['snapshot']).read_bytes()
        need(sha(payload)==record['sha256']==package['checksum'],'Archived locked package differs')
        key=index_key(package['name']);source_index=PUBLIC/'index'/INDEX/'.cache'/key
        metadata=source_index.read_bytes();version=entries(metadata).get(package['version'])
        need(version is not None and version['cksum']==package['checksum'],'Public cached index does not match locked archive')
        planned.append((destination,payload,key,metadata,package))
    # 所有锁定身份先核对，再写入D盘；已有不同archive拒绝覆盖。
    copied=0
    for destination,payload,key,metadata,package in planned:
        destination.parent.mkdir(parents=True,exist_ok=True)
        if destination.exists():need(sha(destination.read_bytes())==package['checksum'],'Existing task archive differs; not overwriting')
        else:
            with destination.open('xb') as file:file.write(payload)
            copied+=1
        index=CACHE/'registry/index'/INDEX/'.cache'/key
        index.parent.mkdir(parents=True,exist_ok=True)
        if index.exists():
            version=entries(index.read_bytes()).get(package['version'])
            need(version is not None and version['cksum']==package['checksum'],'Existing task index differs; not overwriting')
        else:
            with index.open('xb') as file:file.write(metadata)
    config=CACHE/'registry/index'/INDEX/'config.json'
    if not config.exists():config.write_bytes((PUBLIC/'index'/INDEX/'config.json').read_bytes())
    OUT.mkdir(parents=True,exist_ok=True)
    env=os.environ|dict(CARGO_HOME=str(CACHE),CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production',TEMP='D:/Temp',TMP='D:/Temp')
    value=subprocess.run(['cargo','metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],cwd=ROOT,env=env,
        stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW)
    (OUT/'metadata.json').write_bytes(value.stdout);(OUT/'metadata.log').write_bytes(value.stderr)
    need(value.returncode==0,'Offline locked cache restoration metadata failed')
    restored=0
    for path,record in snapshots.items():
        if path.is_relative_to(CACHE/'registry/cache') or path.is_relative_to(CACHE/'registry/src'):
            need(path.is_file() and sha(path.read_bytes())==record['sha256'],'Restored source/archive differs: '+str(path))
            restored+=1
    result=dict(status='restored_locked_dependency_cache',archives_verified=len(registry),archives_copied=copied,
        original_source_and_archive_files_verified=restored,source_evidence=str(EVIDENCE),Cargo_lock_changed=False,
        public_C_cache_read_only=True,network_download=False,tool_installation=False,old_mutant_sources_or_executables_restored=False)
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False))


if __name__=='__main__':run()
