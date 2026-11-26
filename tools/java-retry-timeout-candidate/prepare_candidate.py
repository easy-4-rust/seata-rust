"""Reproduce the isolated retention patch without overwriting a differing existing artifact."""
import difflib,hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;PIN='cba73e180fd8896a99df6415b859d275122feaa7';ORACLE=ROOT.parent/'seata-java-phase09-oracle';DEST=ROOT/'compatibility/seata-java'/PIN
FILES={'SessionHolder.java':'server/src/main/java/org/apache/seata/server/session/SessionHolder.java','FileSessionManager.java':'server/src/main/java/org/apache/seata/server/storage/file/session/FileSessionManager.java','DefaultCoordinator.java':'server/src/main/java/org/apache/seata/server/coordinator/DefaultCoordinator.java'}
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 if path.exists():
  current=path.read_bytes()
  if current!=data and not (path.suffix=='.json' and current.replace(b'\r\n',b'\n')==data.replace(b'\r\n',b'\n')):raise RuntimeError('Existing candidate artifact differs; reconcile explicitly: '+str(path))
 else:
  with path.open('xb') as stream:stream.write(data)
def run():
 parts=[];rows=[]
 for leaf,relative in FILES.items():
  old=subprocess.check_output(['git','-C',str(ORACLE),'show',PIN+':'+relative]).replace(b'\r\n',b'\n');new=(HERE/leaf).read_bytes().replace(b'\r\n',b'\n');parts.append(f'diff --git a/{relative} b/{relative}\n');parts.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),new.decode().splitlines(keepends=True),fromfile='a/'+relative,tofile='b/'+relative));rows.append(dict(path=relative,original_lf_sha256=sha(old),patched_lf_sha256=sha(new)))
 patch=''.join(parts).encode();put(DEST/'0002-retain-retry-timeout-sessions.patch',patch);manifest=dict(java_pin=PIN,patch='0002-retain-retry-timeout-sessions.patch',patch_sha256=sha(patch),files=rows,scope='Independent FILE/RAFT operator-retention candidate sharing original coordinator startup lock policy; original source/JAR and Rust TC policy unchanged.');put(DEST/'retry-timeout-manifest.json',(json.dumps(manifest,indent=2)+'\n').encode());return manifest
if __name__=='__main__':print(json.dumps(run(),indent=2))
