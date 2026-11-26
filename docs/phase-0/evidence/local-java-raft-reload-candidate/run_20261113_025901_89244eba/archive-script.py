from pathlib import Path
import hashlib,importlib.util,json,subprocess,sys
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
BASE=ROOT/'docs/phase-0/evidence/local-java-raft-reload-candidate'
OUT=BASE/'run_20261113_025901_89244eba';UNIT=Path('D:/codex-builds/raft-observation-20261113')
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def put(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);data=data.encode() if isinstance(data,str) else data
 if path.exists():
  assert path.read_bytes()==data,('Refusing overwrite',path)
 else:
  with path.open('xb') as stream:stream.write(data)
def save(path,value):put(path,json.dumps(value,indent=2)+'\n')
sys.path.insert(0,str(ROOT/'tools/java-raft-reload-candidate'))
import checks,native_observer
result=json.loads((OUT/'result.json').read_bytes());assert result['succeeded'] and result['ports_released'] and not result['cleanup_errors']
assert len(result['owned_pids'])==len(set(result['owned_pids']))==12
final=json.loads((OUT/'final-input-check.json').read_bytes());assert all(final.values())
for row in json.loads((OUT/'source-input-audit.json').read_bytes())+json.loads((OUT/'generated-class-inputs.json').read_bytes()):
 assert sha(Path(row['path']).read_bytes())==row['sha256']==sha((OUT/row['snapshot']).read_bytes())
h=module('archive_raft_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
assert h.sha(ROOT.parent/'seata-java-phase09-oracle/server/target/seata-server.jar')==h.JAR_HASH
for port in [28091,28092,28093,28094,28095,28096,28101,28102,28103]:assert not h.port_open(port)
class Stream:
 def __init__(self,data):self.data=data;self.sent=bytearray()
 def recv(self,size):part=self.data[:min(size,3)];self.data=self.data[len(part):];return part
 def sendall(self,value):self.sent.extend(value)
queries=0;confirmations=0;source_rows=[]
for cohort in result['cohorts']:
 status=cohort['seeded_status'];directory=OUT/f'status_{status}'
 assert cohort['actual_snapshot_reload'] and cohort['first_reload_exceptions']==cohort['second_reload_exceptions']==0
 leaders={}
 for stage in ['initial','seeded','snapshotted','election_one','restarted','election_two']:
  values=[json.loads(p.read_bytes()) for p in directory.glob(stage+'-node_*.json')]
  leader=next(v for v in values if v['leader']);assert sum(v['leader'] for v in values)==1
  checks.verify_completion(leader)
  again=json.loads((directory/(stage+'-leader-confirmation.json')).read_bytes());checks.verify_completion(again)
  assert all(again[k]==leader[k] for k in ['term','node_id','leader_id','reload_complete_term'])
  confirmations+=1;leaders[stage]=leader
  assert all(v['applied']>=leader['committed'] and v['leader_id']==leader['leader_id'] for v in values)
  if stage!='initial':
   for value in values:checks.verify_records(value,status,'192.168.1.10',stage in ['election_one','restarted','election_two'])
 checks.verify_transition(leaders['snapshotted'],leaders['election_one'],leaders['snapshotted']['node_id'])
 checks.verify_transition(leaders['restarted'],leaders['election_two'],leaders['restarted']['node_id'])
 for stage in ['election_one','restarted','election_two']:
  for index in range(2):
   path=directory/f'{stage}-native-{index}.json';proof=json.loads(path.read_bytes());frames=json.loads((directory/f'{stage}-native-{index}-frames.json').read_bytes())
   assert frames==proof['frames'] and len(frames)==6
   for frame in frames:assert sha(bytes.fromhex(frame['frame_hex']))==frame['sha256']
   requests=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='observer-to-tc'];responses=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='tc-to-observer']
   assert [int.from_bytes(f[16:18],'big') for f in requests]==[101,15,21]
   expected=5 if status==1 and index==0 else 15 if status==1 else status
   stream=Stream(b''.join(responses));observed=native_observer.query(stream,f'192.168.1.10:28091:{940000+index}',f'mysql://raft-retry-{index}','orders:1',expected,status==16 or index!=0)
   assert not stream.data and bytes(stream.sent)==b''.join(requests) and observed==proof
   queries+=1
 for path in directory.glob('*-events.json'):
  events=json.loads(path.read_bytes())['recording']['events'];stage=path.name.split('_a-events')[0]
  window=json.loads((directory/(stage+'-reload-exceptions.json')).read_bytes())['boundary']
  assert not checks.reload_exceptions(events,status,window)
 for path in directory.glob('node_*_generation_*/tc-stdout.log'):
  text=path.read_text(encoding='utf-8',errors='replace')
  for name in ['org.apache.seata.server.cluster.raft.RaftStateMachine','org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.cluster.raft.RaftReloadObservationControl']:
   line=next(line for line in text.splitlines() if ' '+name+' source: ' in line)
   assert (OUT/'overlay').as_uri().replace('file:///','file:/') in line
   source_rows.append(dict(log=path.relative_to(OUT).as_posix(),type=name,line=line))
 assert b'on snapshot load end index:' in (directory/f"node_{cohort['first_leader']}_generation_1/tc-stdout.log").read_bytes()
assert queries==18 and confirmations==18 and len(source_rows)==48
save(OUT/'class-origins.json',source_rows)
save(OUT/'independent-revalidation.json',dict(native_queries_replayed=queries,raw_frames=queries*6,stable_leader_confirmations=confirmations,actual_leader_transitions=6,actual_snapshot_reloads=3,class_origins=48))
binary_before=json.loads((UNIT/'test-binary-before.json').read_bytes())
assert binary_before=={p.relative_to(UNIT/'test-classes').as_posix():sha(p.read_bytes()) for p in (UNIT/'test-classes').rglob('*.class')}
for path in (UNIT/'test-classes').rglob('*.class'):put(OUT/'unit/test-classes'/path.relative_to(UNIT/'test-classes'),path.read_bytes())
for path in (UNIT/'original-jar').rglob('*.class'):put(OUT/'unit/original-jar'/path.relative_to(UNIT/'original-jar'),path.read_bytes())
for name in ['canonical-original-jar-red-run.log','runtime-exact-green-run.log','red-final-compile.args','red-final-compile.log','canonical-original-jar-red-run.args','runtime-exact-green-run.args','test-binary-before.json','sealed-regression.args','sealed-regression.log','sealed-regression-dependency-before.json','sealed-regression-dependency-after.json','spring-regression-compile.args','spring-regression-compile.log']:
 put(OUT/'unit'/name,(UNIT/name).read_bytes())
for path in (UNIT/'spring-regression-classes').rglob('*.class'):put(OUT/'unit/regression-classes'/path.relative_to(UNIT/'spring-regression-classes'),path.read_bytes())
for name in ['BaseSpringBootTest.java','cluster/raft/RaftStateMachineTest.java']:
 relative='server/src/test/java/org/apache/seata/server/'+name;data=h.command(['git','-C',ROOT.parent/'seata-java-phase09-oracle','show',h.PIN+':'+relative]).stdout
 assert data.replace(b'\r\n',b'\n')==(ROOT.parent/'seata-java-phase09-oracle'/relative).read_bytes().replace(b'\r\n',b'\n')
 put(OUT/'unit/original-sources'/Path(name).name,data)
assert b'OBSERVATION_JUNIT tests=8 succeeded=2 failed=6 skipped=0' in (OUT/'unit/canonical-original-jar-red-run.log').read_bytes()
assert b'OBSERVATION_JUNIT tests=8 succeeded=8 failed=0 skipped=0' in (OUT/'unit/runtime-exact-green-run.log').read_bytes()
assert b'OBSERVATION_JUNIT tests=44 succeeded=44 failed=0 skipped=0' in (OUT/'unit/sealed-regression.log').read_bytes()
assert (UNIT/'sealed-regression-dependency-before.json').read_bytes()==(UNIT/'sealed-regression-dependency-after.json').read_bytes()
python_log=OUT/'unit/python-tests.log'
if not python_log.exists():
 r=subprocess.run([sys.executable,'-m','unittest','discover','-s',str(ROOT/'tools/java-raft-reload-candidate'),'-p','test_*.py'],env=h.ENV,capture_output=True,timeout=30)
 assert r.returncode==0;put(python_log,r.stdout+r.stderr)
assert b'Ran 18 tests' in python_log.read_bytes() and b'OK' in python_log.read_bytes()
put(OUT/'archive-script.py',Path(__file__).read_bytes())
print('REVALIDATED',OUT,'queries',queries,'java',52,'python',18,flush=True)
