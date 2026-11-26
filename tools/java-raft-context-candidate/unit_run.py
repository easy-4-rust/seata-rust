"""Same frozen target/regression test bytes against original JAR classes and optional candidate."""
import argparse,hashlib,importlib.util,json,os,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;OUT=Path('D:/codex-builds/raft-context-unit-v4-20261113')
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
def quote(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def run(label,overlay=None):
 OUT.mkdir(exist_ok=True);h=module('context_unit_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py');deps=module('context_unit_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
 parent=Path('D:/codex-builds/metrics-lifecycle-20261113');cp=(parent/'effective-classpath.txt').read_text().strip();dependencies=deps.dependency_inventory(cp)
 if dependencies!=json.loads((parent/'frozen-compile-dependencies.json').read_bytes()):raise RuntimeError('Certified original server classpath changed')
 put(OUT/'effective-classpath.txt',cp+'\n');save(OUT/'dependency-before.json',dependencies)
 jar=ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original executable JAR changed')
 original=OUT/'original-jar';prefixes=['org/apache/seata/server/cluster/raft/sync/msg/dto/BranchTransactionDTO','org/apache/seata/server/storage/raft/session/RaftSessionManager','org/apache/seata/server/cluster/raft/execute/branch/UpdateBranchSessionExecute']
 with zipfile.ZipFile(jar) as archive:
  for member in archive.namelist():
   if member.startswith('BOOT-INF/classes/'):
    relative=member[len('BOOT-INF/classes/'):]
    if relative.endswith('.class') and any(relative==prefix+'.class' or relative.startswith(prefix+'$') for prefix in prefixes):put(original/relative,archive.read(member))
 save(OUT/'original-class-audit.json',[dict(path=p.relative_to(original).as_posix(),sha256=h.sha(p)) for p in sorted(original.rglob('*.class'))])
 tests=OUT/'test-classes';tests.mkdir(exist_ok=True);selected=Path(overlay) if overlay else original
 classpath=str(tests)+os.pathsep+str(selected)+os.pathsep+cp
 def command(binary,args,name):
  path=OUT/(name+'.args');put(path,'\n'.join(quote(x) for x in args)+'\n');result=h.command([binary,'@'+str(path)],timeout=120,check=False);put(OUT/(name+'.log'),result.stdout);return result
 if label=='red':
  maker_source=OUT/'legacy-source/LegacyRaftContextBlob.java';put(maker_source,(HERE/'LegacyRaftContextBlob.java').read_bytes());maker=OUT/'legacy-maker';maker.mkdir(exist_ok=True);legacy_cp=str(original)+os.pathsep+cp
  result=command(h.JAVA.parent/'javac.exe',['-encoding','UTF-8','-cp',legacy_cp,'-d',maker,maker_source],'legacy-compile')
  if result.returncode:raise RuntimeError(result.stdout.decode(errors='replace'))
  result=command(h.JAVA,['-XX:ActiveProcessorCount=2','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(OUT),'-cp',str(maker)+os.pathsep+legacy_cp,'LegacyRaftContextBlob',OUT/'legacy-status-only.bin'],'legacy-produce')
  if result.returncode:raise RuntimeError(result.stdout.decode(errors='replace'))
  save(OUT/'legacy-blob-audit.json',dict(producer_source_sha256=h.sha(maker_source),producer_class_sha256=h.sha(maker/'LegacyRaftContextBlob.class'),blob_sha256=h.sha(OUT/'legacy-status-only.bin'),original_jar_sha256=h.JAR_HASH,original_classes=json.loads((OUT/'original-class-audit.json').read_bytes())))
  relative='server/src/test/java/org/apache/seata/server/cluster/raft/sync/msg/dto/BranchTransactionDTOTest.java';regression=h.command(['git','-C',ROOT.parent/'seata-java-phase09-oracle','show',h.PIN+':'+relative]).stdout
  inputs={HERE/'RaftContextReplicationTest.java':(HERE/'RaftContextReplicationTest.java').read_bytes(),ROOT/'tools/java-raft-reload-candidate/ObservationJUnit.java':(ROOT/'tools/java-raft-reload-candidate/ObservationJUnit.java').read_bytes(),Path('original-pin')/relative:regression}
  rows=[];sources=[]
  for path,data in inputs.items():
   snapshot=OUT/'frozen-sources'/sha(str(path).encode())[:20]/path.name;put(snapshot,data);sources.append(snapshot);rows.append(dict(path=str(path),snapshot=str(snapshot),sha256=sha(data)))
  save(OUT/'source-audit.json',rows);result=command(h.JAVA.parent/'javac.exe',['-encoding','UTF-8','-cp',classpath,'-d',tests]+sources,'compile')
  if result.returncode:raise RuntimeError(result.stdout.decode(errors='replace'))
  save(OUT/'test-binary-audit.json',[dict(path=p.relative_to(tests).as_posix(),sha256=h.sha(p)) for p in sorted(tests.rglob('*.class'))])
 before=json.loads((OUT/'test-binary-audit.json').read_bytes())
 if before!=[dict(path=p.relative_to(tests).as_posix(),sha256=h.sha(p)) for p in sorted(tests.rglob('*.class'))]:raise RuntimeError('Frozen test class bytes changed')
 result=command(h.JAVA,['-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(OUT),'-Dseata.context.legacy.blob='+str(OUT/'legacy-status-only.bin'),'-cp',classpath,'ObservationJUnit','org.apache.seata.server.cluster.raft.execute.branch.RaftContextReplicationTest','org.apache.seata.server.cluster.raft.sync.msg.dto.BranchTransactionDTOTest'],label)
 if deps.dependency_inventory(cp)!=dependencies:raise RuntimeError('Actual test dependencies changed')
 save(OUT/(label+'-dependency-after.json'),dependencies);print(result.stdout.decode(errors='replace')[-6500:]);return result.returncode
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('label');parser.add_argument('--overlay',type=Path);args=parser.parse_args();raise SystemExit(run(args.label,args.overlay))
