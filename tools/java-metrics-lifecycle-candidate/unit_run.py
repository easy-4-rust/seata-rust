"""Identical test binaries against the original JAR and isolated metrics candidate."""
import hashlib,importlib.util,json,os,sys,xml.etree.ElementTree as ET,zipfile
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1];OUT=Path('D:/codex-builds/metrics-lifecycle-20261113')
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():
  if path.read_bytes()!=data:raise RuntimeError('Refusing to replace existing unit artifact: '+str(path))
 else:
  with path.open('xb') as stream:stream.write(data)
def quote(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def run(stage,overlay=None):
 OUT.mkdir(parents=True,exist_ok=True)
 h=module('metrics_unit_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
 audit=module('metrics_unit_dependencies',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
 report=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3/server/target/surefire-reports/TEST-org.apache.seata.server.session.FileSessionManagerTest.xml')
 raw=next(p.get('value') for p in ET.parse(report).getroot().findall('properties/property') if p.get('name')=='surefire.test.class.path')
 cp=os.pathsep.join(x for x in raw.split(os.pathsep) if x);put(OUT/'classpath-report.xml',report.read_bytes());put(OUT/'effective-classpath.txt',cp+'\n')
 dependencies=audit.dependency_inventory(cp);put(OUT/'dependency-before.json',json.dumps(dependencies,indent=2)+'\n')
 jar=ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR changed')
 original=OUT/'original-jar';leaf='org/apache/seata/server/metrics/MetricsSubscriber.class'
 with zipfile.ZipFile(jar) as stream:put(original/leaf,stream.read('BOOT-INF/classes/'+leaf))
 tests=OUT/'test-classes';tests.mkdir(exist_ok=True);selected=Path(overlay) if overlay else original
 cp=str(tests)+';'+str(selected)+';'+cp
 def command(binary,args,label):
  path=OUT/(label+'.args');put(path,'\n'.join(quote(arg) for arg in args)+'\n');result=h.command([binary,'@'+str(path)],timeout=120,check=False);put(OUT/(label+'.log'),result.stdout);return result
 if stage=='red':
  relative='server/src/test/java/org/apache/seata/server/metrics/MetricsSubscriberUnitTest.java'
  data=h.command(['git','-C',ROOT.parent/'seata-java-phase09-oracle','show',h.PIN+':'+relative]).stdout
  source=OUT/'original-sources/MetricsSubscriberUnitTest.java';put(source,data)
  result=command(h.JAVA.parent/'javac.exe',['-encoding','UTF-8','-cp',cp,'-d',tests,HERE/'MetricsLifecycleCandidateTest.java',source,ROOT/'tools/java-raft-reload-candidate/ObservationJUnit.java'],'test-compile')
  if result.returncode:raise RuntimeError(result.stdout.decode(errors='replace'))
  put(OUT/'test-binary-audit.json',json.dumps([dict(path=p.relative_to(tests).as_posix(),sha256=sha(p)) for p in sorted(tests.rglob('*.class'))],indent=2)+'\n')
 before=json.loads((OUT/'test-binary-audit.json').read_bytes())
 if before!=[dict(path=p.relative_to(tests).as_posix(),sha256=sha(p)) for p in sorted(tests.rglob('*.class'))]:raise RuntimeError('Test binaries changed')
 result=command(h.JAVA,['-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(OUT),'-cp',cp,'ObservationJUnit','org.apache.seata.server.metrics.MetricsLifecycleCandidateTest','org.apache.seata.server.metrics.MetricsSubscriberUnitTest'],stage)
 if audit.dependency_inventory(raw.rstrip(os.pathsep))!=dependencies:raise RuntimeError('Unit dependencies changed')
 put(OUT/(stage+'-dependency-after.json'),json.dumps(dependencies,indent=2)+'\n')
 print(result.stdout.decode(errors='replace')[-17000:]);return result.returncode
if __name__=='__main__':sys.exit(run(sys.argv[1],sys.argv[2] if len(sys.argv)>2 else None))
