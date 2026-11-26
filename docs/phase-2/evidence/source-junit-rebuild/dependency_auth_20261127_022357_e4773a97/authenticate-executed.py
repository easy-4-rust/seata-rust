"""Compare resolved Maven input bytes with exact-version public Central checksums."""
import concurrent.futures,datetime,hashlib,json,re,urllib.error,urllib.request,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];REPO=Path('D:/codex-builds/seata-source-junit-m2')
def run(evidence_root=None):
    files=sorted([*REPO.rglob('*.pom'),*REPO.rglob('*.jar')]);stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    work=Path('D:/codex-builds')/('seata_junit_dependency_auth_'+stamp);work.mkdir()
    out=(evidence_root or ROOT/'docs/phase-2/evidence/source-junit')/('dependency_auth_'+stamp);out.mkdir(parents=True);print('SOURCE_JUNIT_DEPENDENCY_AUTH '+str(out)+' files='+str(len(files)),flush=True)
    def check(p):
        leaf=p.relative_to(REPO).as_posix();url='https://repo.maven.apache.org/maven2/'+leaf+'.sha1';data=p.read_bytes();value=dict(path=str(p),relative=leaf,sha256=hashlib.sha256(data).hexdigest(),sha1=hashlib.sha1(data).hexdigest(),checksum_url=url)
        try:
            request=urllib.request.Request(url,headers={'User-Agent':'Seata-Rust pinned source-JUnit dependency verifier'})
            with urllib.request.urlopen(request,timeout=25) as response:raw=response.read(4096)
            artifact=work/(hashlib.sha256(leaf.encode()).hexdigest()+'.sha1');artifact.write_bytes(raw);text=raw.decode('ascii').strip();match=re.search(r'(?i)\b[0-9a-f]{40}\b',text)
            if not match:return value|dict(status='UNRECOGNIZED_CHECKSUM',checksum_response=str(artifact))
            expected=match.group().lower();return value|dict(status='MATCH' if expected==value['sha1'] else 'MISMATCH',published_sha1=expected,checksum_response=str(artifact),checksum_response_sha256=hashlib.sha256(raw).hexdigest())
        except urllib.error.HTTPError as error:return value|dict(status='CHECKSUM_HTTP_'+str(error.code))
        except Exception as error:return value|dict(status='CHECKSUM_UNAVAILABLE',error=type(error).__name__+': '+str(error))
    rows=[]
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for row in pool.map(check,files):
            rows.append(row)
            if len(rows)%100==0:print('AUTHENTICATED_INPUT_PROGRESS '+str(len(rows)),flush=True)
    summary={status:sum(v['status']==status for v in rows) for status in sorted({v['status'] for v in rows})}
    (out/'dependency-authentication.json').write_text(json.dumps(dict(repository=str(REPO),work=str(work),counts=summary,files=rows,all_authenticated=all(v['status']=='MATCH' for v in rows)),indent=2)+'\n',encoding='utf-8')
    (out/'authenticate-executed.py').write_bytes(Path(__file__).read_bytes());print(json.dumps(dict(path=str(out),counts=summary)),flush=True)
    return out
if __name__=='__main__':run()
