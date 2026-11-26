'Lossless NTFS compression of stopped owned fixture data; excludes reparse/socket objects.'
import hashlib,json,os,stat,subprocess,sys,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(packet):
    runtime=json.loads((packet/'owned-runtime.json').read_bytes());data=Path(runtime['runtime'])/'data'
    if data.parent.parent.resolve()!=Path('D:/codex-builds').resolve() or not data.parent.name.startswith('seata_commit_direction_'):raise ValueError('Unexpected owned data path')
    env=os.environ|dict(DOCKER_CONFIG='D:/Temp/seata-docker-config')
    inspected=subprocess.run(['docker','inspect',runtime['container_id']],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
    if inspected.returncode==0 or b'no such object' not in inspected.stdout.lower():raise ValueError('Container not confirmed absent')
    files=[]
    for p in data.rglob('*'):
        st=p.lstat()
        if getattr(st,'st_file_attributes',0)&0x400 or not stat.S_ISREG(st.st_mode) or st.st_size<1048576 or getattr(st,'st_file_attributes',0)&0x800:continue
        files.append((p,sha(p)))
    for offset in range(0,len(files),16):
        result=subprocess.run(['C:/Windows/System32/compact.exe','/C','/I','/Q']+[str(p) for p,_ in files[offset:offset+16]],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
        if result.returncode:raise ValueError(result.stdout.decode(errors='replace'))
    for p,digest in files:
        if sha(p)!=digest:raise ValueError('Compression changed content')
    audit=ROOT/'docs/phase-2/evidence/mysql-commit-direction'/('data-maintenance-'+uuid.uuid4().hex+'.json')
    with audit.open('x',encoding='utf-8') as f:json.dump(dict(packet=str(packet),owned_runtime_sha256=sha(packet/'owned-runtime.json'),confirmed_container_absent=True,data_retained=str(data),regular_files=[dict(path=str(p),sha256=digest,post_sha256=sha(p)) for p,digest in files],reparse_objects_excluded=True,business_validation=False),f,indent=2)
    return dict(path=str(audit.relative_to(ROOT)),sha256=sha(audit))
if __name__=='__main__':
    for p in sys.argv[1:]:print(json.dumps(run(Path(p))),flush=True)
