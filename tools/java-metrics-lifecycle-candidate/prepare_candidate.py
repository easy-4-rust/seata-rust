"""Create an independent metrics lifecycle patch; preserve all earlier candidates."""
import difflib,hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1];PIN='cba73e180fd8896a99df6415b859d275122feaa7';REL='server/src/main/java/org/apache/seata/server/metrics/MetricsSubscriber.java'
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 if path.exists():
  if path.read_bytes()!=data:raise RuntimeError('Refusing to replace existing candidate artifact: '+str(path))
 else:
  with path.open('xb') as stream:stream.write(data)
def run():
 old=subprocess.check_output(['git','-C',str(ROOT.parent/'seata-java-phase09-oracle'),'show',PIN+':'+REL]).replace(b'\r\n',b'\n');new=(HERE/'MetricsSubscriber.java').read_bytes().replace(b'\r\n',b'\n')
 name='0004-honor-metrics-event-lifecycle.patch';patch=('diff --git a/'+REL+' b/'+REL+'\n'+''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='a/'+REL,tofile='b/'+REL))).encode();dest=ROOT/'compatibility/seata-java'/PIN
 manifest=dict(java_pin=PIN,patch=name,patch_sha256=sha(patch),files=[dict(path=REL,original_lf_sha256=sha(old),patched_lf_sha256=sha(new))],scope='Honor doing/done event timing without changing producer, transaction semantics or metric names/tags; independent candidate only')
 put(dest/name,patch);put(dest/'metrics-lifecycle-manifest.json',(json.dumps(manifest,indent=2)+'\n').encode());return manifest
if __name__=='__main__':print(json.dumps(run(),indent=2))
