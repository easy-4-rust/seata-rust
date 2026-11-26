"""Immutable inputs and original-JAR overlay for the owned three-node RAFT candidate run."""
import argparse,datetime,hashlib,importlib.util,json,os,socket,uuid,xml.etree.ElementTree as ET
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
MAIN='org.apache.seata.server.cluster.raft.RaftReloadObservationControl'
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def save(path,data):put(path,json.dumps(data,indent=2)+'\n')
def quote(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def freeze(output,paths):
 rows=[]
 for path in sorted(set(paths)):
  data=path.read_bytes();leaf='source-inputs/'+sha(str(path).encode())[:20]+'.bin';put(output/leaf,data)
  rows.append(dict(path=str(path),snapshot=leaf,sha256=sha(data)))
 save(output/'source-input-audit.json',rows);return rows
def verify_final_inputs(output,source_inputs,dependencies,inventory,classes):
 for row in source_inputs:
  if sha(Path(row['path']).read_bytes())!=row['sha256']:raise RuntimeError('Run input changed: '+row['path'])
 f=module('raft_final_dependency_inventory',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
 cp=(output/'effective-classpath.txt').read_text().strip()
 if f.dependency_inventory(cp)!=dependencies:raise RuntimeError('Compile dependencies changed during run')
 actual=[dict(path=p.relative_to(classes).as_posix(),sha256=sha(p.read_bytes())) for p in sorted(classes.rglob('*.class'))]
 if actual!=inventory:raise RuntimeError('Runtime overlay classes changed')
 save(output/'final-input-check.json',dict(source_inputs_unchanged=True,dependencies_unchanged=True,overlay_unchanged=True))

def run(args):
 for port in [28091,28092,28093,28094,28095,28096,28101,28102,28103]:
  with socket.socket() as listener:listener.bind((args.host if port in [28092,28094,28096] else '0.0.0.0',port))
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
 output=ROOT/'docs/phase-0/evidence/local-java-raft-reload-candidate'/('run_'+stamp);output.mkdir(parents=True)
 runtime=Path('D:/codex-builds')/('jro_'+stamp);runtime.mkdir();token='phase0-raft-'+stamp.rsplit('_',1)[1]
 parent=ROOT/'docs/phase-0/evidence/local-java-retry-timeout-candidate/run_20261113_021912_ac00a207'
 patchdir=ROOT/'compatibility/seata-java/cba73e180fd8896a99df6415b859d275122feaa7'
 report=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3/test-suite/test-new-version/target/surefire-reports/TEST-org.apache.seata.server.session.RetryTimeoutRetentionCandidateTest.xml')
 paths=[p for p in HERE.iterdir() if p.is_file()]+[ROOT/'docs/phase-0/java-raft-reload-observation-candidate-plan.md',ROOT/'tools/java-tc-integration/backend_recovery/run.py',ROOT/'tools/java-tc-integration/raft_recovery/checks.py',ROOT/'tools/java-tc-integration/raft_recovery/ReadRaftExceptions.java',ROOT/'tools/java-tc-integration/raft_recovery/run.py',ROOT/'tools/java-tc-integration/config/application.yml',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py',ROOT/'tools/java-v1-golden/RaftRetryTimeoutControl.java',report,parent/'overlay-manifest.json',parent/'result.json',parent/'compile-dependency-audit.json',parent.parent/'manifest.json']+list((parent/'overlay').rglob('*.class'))+list(patchdir.glob('*.json'))+list(patchdir.glob('*.patch'))
 source_inputs=freeze(output,paths)
 catalog=(parent.parent/'manifest.json').read_bytes()
 if sha(catalog)!='90f7cc98a194ebb652ff0a1fe725b4e01c16daf08479309549ba3483118263a1':raise RuntimeError('Certified retention parent catalog changed')
 for path in [parent/'overlay-manifest.json',parent/'result.json',parent/'compile-dependency-audit.json']+list((parent/'overlay').rglob('*.class')):
  relative=path.relative_to(parent.parent).as_posix()
  expected=next(row['sha256'] for row in json.loads(catalog)['files'] if row['path']==relative)
  if sha(path.read_bytes())!=expected:raise RuntimeError('Certified retention parent artifact changed: '+relative)
 if len(list((parent/'overlay').rglob('*.class')))!=19:raise RuntimeError('Certified retention overlay inventory count changed')
 f=module('raft_dependency_inventory',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
 raw=next(p.get('value') for p in ET.parse(report).getroot().findall('properties/property') if p.get('name')=='surefire.test.class.path')
 cp=os.pathsep.join(x for x in raw.split(os.pathsep) if x);put(output/'effective-classpath.txt',cp+'\n');dependencies=f.dependency_inventory(cp);save(output/'compile-dependency-audit.json',dependencies)
 if dependencies!=json.loads((parent/'compile-dependency-audit.json').read_bytes()):raise RuntimeError('Compile dependency bytes differ from the certified parent cohort')
 h=module('raft_live_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
 jar=ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR changed')
 if h.command(['git','-C',jar.parents[2],'rev-parse','HEAD']).stdout.decode().strip()!=h.PIN:raise RuntimeError('Original Java pin changed')
 manifest=json.loads((patchdir/'raft-reload-observation-manifest.json').read_bytes())
 if sha((patchdir/manifest['patch']).read_bytes())!=manifest['patch_sha256'] or sha((HERE/'RaftStateMachine.java').read_bytes().replace(b'\r\n',b'\n'))!=manifest['files'][0]['patched_lf_sha256']:raise RuntimeError('Observation patch mismatch')
 compiled=output/'compiled';compiled.mkdir();argfile=runtime/'compile.args'
 put(argfile,'\n'.join(quote(x) for x in ['-encoding','UTF-8','-cp',cp,'-d',compiled,HERE/'RaftStateMachine.java',HERE/'RaftReloadObservationControl.java',ROOT/'tools/java-tc-integration/raft_recovery/ReadRaftExceptions.java'])+'\n')
 result=h.command([h.JAVA.parent/'javac.exe','@'+str(argfile)],check=False,timeout=90);put(output/'compile.log',result.stdout)
 if result.returncode:raise RuntimeError('Actual runtime compilation failed')
 generated=[dict(path=str(p),snapshot=p.relative_to(output).as_posix(),sha256=sha(p.read_bytes())) for p in sorted(compiled.rglob('*.class'))]
 save(output/'generated-class-inputs.json',generated);source_inputs+=generated
 classes=output/'overlay'
 for row in json.loads((parent/'overlay-manifest.json').read_bytes()):
  path=parent/'overlay'/row['path']
  if h.sha(path)!=row['sha256']:raise RuntimeError('Sealed parent overlay differs')
  put(classes/row['path'],path.read_bytes())
 for path in (compiled/'org/apache/seata/server/cluster/raft').glob('*.class'):put(classes/path.relative_to(compiled),path.read_bytes())
 inventory=[dict(path=p.relative_to(classes).as_posix(),sha256=h.sha(p)) for p in sorted(classes.rglob('*.class'))];save(output/'overlay-manifest.json',inventory)
 reader_classes=compiled;probe=None
 compiled=classes/'org/apache/seata/server/cluster/raft/RaftReloadObservationControl.class'
 print('RAFT_RUNTIME_READY '+str(output),flush=True)
 body=module('candidate_owned_live_body',HERE/'live_body.py')
 body.execute(args,h,output,runtime,jar,classes,compiled,reader_classes,probe,token,source_inputs,dependencies,inventory)

if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--host',default='192.168.1.10');run(parser.parse_args())
