"""Compile target tests once, then execute identical binaries against original/candidate classes."""
import importlib.util,json,os,subprocess,sys,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def quote(value):return '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
def run(out,label,overlay=None,compile_tests=False):
 h=module('raft_observation_unit_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py');out=Path(out);out.mkdir(parents=True,exist_ok=True)
 report=Path('D:/codex-builds/jret_20261113_010005_fe3b7eb3/test-suite/test-new-version/target/surefire-reports/TEST-org.apache.seata.server.session.RetryTimeoutRetentionCandidateTest.xml')
 suite=ET.parse(report).getroot();cp=os.pathsep.join(x for x in next(p.get('value') for p in suite.findall('properties/property') if p.get('name')=='surefire.test.class.path').split(os.pathsep) if x)
 classes=out/'test-classes';classes.mkdir(exist_ok=True)
 cp=str(classes)+';'+(str(overlay)+';' if overlay else '')+cp
 def command(args,log):
  path=out/(log+'.args');path.write_text('\n'.join(quote(x) for x in args)+'\n',encoding='utf-8')
  result=h.command([h.JAVA if log.endswith('-run') else h.JAVA.parent/'javac.exe','@'+str(path)],timeout=120,check=False)
  with (out/(log+'.log')).open('xb') as stream:stream.write(result.stdout)
  return result
 if compile_tests:
  result=command(['-encoding','UTF-8','-cp',cp,'-d',classes,HERE/'RaftReloadObservationTest.java',HERE/'ObservationJUnit.java'],label+'-compile')
  if result.returncode:raise RuntimeError(result.stdout.decode(errors='replace'))
 result=command(['-Djava.io.tmpdir=D:/Temp','-cp',cp,'ObservationJUnit','org.apache.seata.server.cluster.raft.RaftReloadObservationTest'],label+'-run')
 print(result.stdout.decode(errors='replace')[-16000:]);return result.returncode
if __name__=='__main__':sys.exit(run(sys.argv[1],sys.argv[2],sys.argv[3] if len(sys.argv)>3 else None,compile_tests=sys.argv[2].startswith('red')))
