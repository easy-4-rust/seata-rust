"""Build a reviewable Java-only patch; immutable reference export/JAR are never edited."""
import difflib
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
ORACLE=Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
DEST=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7'
FILES={
 'ProtocolDetectHandler.java':'core/src/main/java/org/apache/seata/core/rpc/netty/ProtocolDetectHandler.java',
 'SeataDetector.java':'core/src/main/java/org/apache/seata/core/protocol/detector/SeataDetector.java',
 'HttpDetector.java':'core/src/main/java/org/apache/seata/core/protocol/detector/HttpDetector.java',
 'Http2Detector.java':'core/src/main/java/org/apache/seata/core/protocol/detector/Http2Detector.java',
 'IncrementalProtocolDetector.java':'core/src/main/java/org/apache/seata/core/protocol/detector/IncrementalProtocolDetector.java',
 'core_pom.xml':'core/pom.xml',
}
def sha(data):return hashlib.sha256(data).hexdigest()
def files():return {target:(HERE/source).read_bytes().replace(b'\r\n',b'\n') for source,target in FILES.items()}
def run():
 DEST.mkdir(parents=True,exist_ok=True);parts=[];rows=[]
 for path,new in files().items():
  source=ORACLE/path;old=source.read_bytes().replace(b'\r\n',b'\n') if source.exists() else b''
  parts.append(f'diff --git a/{path} b/{path}\n')
  if not source.exists():parts.append('new file mode 100644\n')
  parts.extend(difflib.unified_diff(old.decode('utf-8').splitlines(keepends=True),new.decode('utf-8').splitlines(keepends=True),fromfile='a/'+path if old else '/dev/null',tofile='b/'+path))
  rows.append(dict(path=path,original_lf_sha256=sha(old) if source.exists() else None,patched_lf_sha256=sha(new)))
 data=''.join(parts).encode('utf-8');patch=DEST/'0001-incremental-protocol-preface.patch';patch.write_bytes(data)
 lombok=Path('E:/Java/.m2/repository/org/projectlombok/lombok/1.18.36/lombok-1.18.36.jar')
 manifest=dict(java_pin=DEST.name,patch=patch.name,patch_sha256=sha(data),files=rows,
  compile_only_dependency=dict(coordinates='org.projectlombok:lombok:1.18.36',scope='provided',optional=True,cached_jar_sha256=sha(lombok.read_bytes())),
  existing_interface_unchanged='core/src/main/java/org/apache/seata/core/protocol/detector/ProtocolDetector.java',
  scope='Independent optional Java reference candidate; original JAR/source remain unchanged and still reject fragmented first magic byte.')
 (DEST/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8');print(str(patch))
if __name__=='__main__':run()
