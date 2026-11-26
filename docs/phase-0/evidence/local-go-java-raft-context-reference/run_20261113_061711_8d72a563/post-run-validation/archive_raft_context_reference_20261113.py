from pathlib import Path
import hashlib,importlib.util,json,os,socket,subprocess
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-raft-context-reference';OUT=BASE/'run_20261113_061711_8d72a563'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def put(p,v):
 p.parent.mkdir(parents=True,exist_ok=True);v=v.encode() if isinstance(v,str) else v
 if p.exists():assert p.read_bytes()==v,('Refusing overwrite',p)
 else:p.write_bytes(v)
def save(p,v):put(p,json.dumps(v,indent=2)+'\n')
def module(n,p):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
s=json.loads((OUT/'summary.json').read_bytes());r=json.loads((OUT/'result.json').read_bytes());assert s['expected_defect_reproduced'] and s['recovery_certified'] is False and r['succeeded'] and r['container_state']=='exited' and r['ports_released']
rows=json.loads((OUT/'source-input-audit.json').read_bytes());assert len(rows)==s['frozen_inputs']==967
for row in rows:assert sha(Path(row['path']))==row['sha256']==sha(OUT/row['snapshot'])
e=module('context_ref_archive_evidence',ROOT/'tools/go-java-v1-raft-context-update/reference_evidence.py');proof=e.derive(OUT);assert proof==json.loads((OUT/'proof.json').read_bytes())==r['proof']
runner=module('context_ref_archive_runner',ROOT/'tools/go-java-v1-raft-context-update/run_reference.py');a=module('context_ref_archive_audit',ROOT/'tools/go-java-v1-inflight/audit.py');h=a.sdk.h;deps=module('context_ref_archive_deps',ROOT/'tools/java-retry-timeout-candidate/file_runtime.py')
assert a.run(Path(s['cohort']))==json.loads((OUT/'go-cohort-audit.json').read_bytes());a.sdk.c.audit_go();assert h.sha(a.sdk.c.ORACLE/'server/target/seata-server.jar')==s['java_jar_sha256']==h.JAR_HASH;assert h.sha(h.DRIVER)==s['driver_sha256'];assert sha(OUT/'real-commit.exe')==s['binary_sha256']
cp=(OUT/'effective-classpath.txt').read_text().strip();inventory=deps.dependency_inventory(cp);assert inventory==json.loads((OUT/'compile-dependency-audit.json').read_bytes())
for folder,count in [('overlay',21),('compiled',3)]:
 actual=[dict(path=p.relative_to(OUT/folder).as_posix(),sha256=sha(p)) for p in sorted((OUT/folder).rglob('*.class'))];assert len(actual)==count and actual==json.loads((OUT/(folder+'-manifest.json')).read_bytes())
parent=Path(s['raft_parent']);check=runner.catalog(parent,'e2a284c1cdf7f2b09821ec2daf363730ce9b115f445a907303760d80a5cb79a2')
for row in json.loads((parent/'overlay-manifest.json').read_bytes()):
 if 'RaftReloadObservationControl' in row['path']:continue
 check(parent/'overlay'/row['path']);assert sha(OUT/'overlay'/row['path'])==row['sha256']
receipt=json.loads((OUT/'leader-stop-election_one.json').read_bytes());before=json.loads((OUT/'cluster-timeout.json').read_bytes());after=json.loads((OUT/'cluster-election_one.json').read_bytes());stop=module('context_ref_stop',ROOT/'tools/go-java-v1-raft-commit-timeout/stop_binding.py');assert stop.verify_stop_binding(receipt,before,after,json.loads(Path(receipt['process_file']).read_bytes()))
origins=[];original_types=['org.apache.seata.server.cluster.raft.sync.msg.dto.BranchTransactionDTO','org.apache.seata.server.storage.raft.session.RaftSessionManager','org.apache.seata.server.cluster.raft.execute.branch.UpdateBranchSessionExecute']
for path in OUT.glob('node_*_generation_*/tc-stdout.log'):
 text=path.read_text(encoding='utf-8',errors='replace');pid=json.loads((path.parent/'process.json').read_bytes())['pid']
 for name in ['org.apache.seata.server.cluster.raft.RaftStateMachine','org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.cluster.raft.RaftBusinessControl']+original_types:
  hits=[line for line in text.splitlines() if ' '+name+' source: ' in line];assert len(hits)==1,(path,name)
  if name in original_types:assert 'jar:nested:' in hits[0] and 'seata-server.jar/!BOOT-INF/classes/!/' in hits[0]
  else:assert 'file:/'+(OUT/'overlay').as_posix()+'/' in hits[0]
  origins.append(dict(pid=pid,type=name,line=hits[0]))
assert len(origins)==28
metadata=[]
for node in range(3):
 audit=json.loads((OUT/f'snapshot-files-timeout-node_{node}.json').read_bytes());observation=json.loads((OUT/f'snapshot-timeout-node_{node}.json').read_bytes());root=OUT/f'snapshot-data-timeout-node_{node}';files=[dict(path=p.relative_to(root).as_posix(),bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(root.rglob('*')) if p.is_file()];assert files==audit['files'] and audit['snapshot']==observation['after']['snapshot'];assert observation['before']['records']==observation['after']['records'];metadata.append(root/'__raft_snapshot_meta')
raw=h.command([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-cp',str(OUT/'compiled')+os.pathsep+cp,'SnapshotMetadataReader']+metadata).stdout
parser=module('context_ref_metadata',ROOT/'tools/go-java-v1-raft-commit-timeout/evidence.py');parsed=parser.parse_metadata(raw);assert len(parsed)==3
for node,value in enumerate(parsed):
 audit=json.loads((OUT/f'snapshot-files-timeout-node_{node}.json').read_bytes());assert (value['index'],value['term'])==(audit['snapshot']['index'],audit['snapshot']['term'])
if not (OUT/'snapshot-metadata-reader-output.log').exists():put(OUT/'snapshot-metadata-reader-output.log',raw)
replayed=h.command([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Dfile.encoding=UTF-8','-cp',OUT/'compiled','ReadRaftExceptions',OUT/'election_one.jfr']).stdout;assert json.loads(replayed)==json.loads((OUT/'election_one-exceptions.json').read_bytes());reload=json.loads((OUT/'reload-election_one.json').read_bytes());assert reload['boundary']==receipt['boundary'] and reload['jfr_sha256']==sha(OUT/'election_one.jfr')
assert not e.topology.completion.reload_exceptions(json.loads(replayed)['recording']['events'],16,receipt['boundary'])
restart=list(OUT.glob('node_*_generation_1/tc-stdout.log'));assert len(restart)==1 and 'on snapshot load end index:' in restart[0].read_text(encoding='utf-8',errors='replace')
runtime=json.loads((OUT/'runtime.json').read_bytes());item=json.loads(h.command(['docker','inspect',r['container']]).stdout)[0];assert item['State']['Status']=='exited' and item['Image']==h.IMAGES['db'] and item['Config']['Labels']['seata-raft-context']==Path(r['runtime']).name
assert next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')['Source'].replace('\\','/').lower()==(Path(r['runtime'])/'mysql').as_posix().lower()
for port in runner.PORTS:
 try:
  with socket.create_connection((runner.HOST if port in runner.RAFT else '127.0.0.1',port),timeout=.2):raise AssertionError('Owned port listening')
 except OSError:pass
ids=','.join(map(str,runtime['all_tc_pids']+runtime['sdk_pids']));command='$ownedIds=@('+ids+'); @(Get-CimInstance Win32_Process | Where-Object { $ownedIds -contains $_.ProcessId } | Select-Object ProcessId,CommandLine) | ConvertTo-Json -Compress';raw=h.command(['powershell','-NoProfile','-Command',command]).stdout.strip();present=json.loads(raw) if raw else [];present=[present] if isinstance(present,dict) else present;assert not any(Path(r['runtime']).name in str(row.get('CommandLine')) for row in present)
post=[Path(__file__),Path('D:/codex-builds/test_raft_context_reference_20261113.py')];audits=[]
for p in post:
 leaf='post-run-validation/'+p.name;put(OUT/leaf,p.read_bytes());audits.append(dict(path=str(p),snapshot=leaf,sha256=sha(p)))
save(OUT/'post-run-validation-inputs.json',audits);env=h.ENV.copy();env['SEATA_CTX_REFERENCE_EVIDENCE']=str(OUT);test=subprocess.run(['python',post[1],'-v'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=30,creationflags=h.FLAGS);assert test.returncode==0 and b'Ran 10 tests' in test.stdout
if not (OUT/'python-reference-tests.log').exists():put(OUT/'python-reference-tests.log',test.stdout)
save(OUT/'independent-revalidation.json',dict(succeeded=True,expected_defect_reproduced=True,recovery_certified=False,frozen_inputs=967,new_python_tests=10,actual_class_origins=origins,snapshot_metadata=parsed,actual_tc_jvms=4,actual_sdk_processes=2,actual_elections=1,actual_snapshot_loads=1,jfr_recordings_reparsed=1,compile_classpath_entries=len(inventory),compile_dependency_files=sum(1 if row['kind']=='file' else len(row['files']) for row in inventory),resources_stopped=True,ports_released=11))
put(BASE/'.gitattributes','* -text\n');put(BASE/'README.md','# 原始 RAFT 分支上下文丢失复现\n\n真实 Prepare 更新后，原主节点持有新上下文，两个副本保留注册时旧上下文。停止主节点并选主后，真实 TCC 回调明确拒绝确认；数据库确认量和执行次数均为 0。此目录证明缺陷，不证明恢复成功。包含一次选主、三个真实快照、一次旧节点快照加载及完整证据核验。\n')
save(BASE/'latest_run.json',dict(evidence_directory=str(OUT),expected_defect_reproduced=True,recovery_certified=False));manifest=dict(primary=OUT.name,expected_defect_reproduced=True,recovery_certified=False,frozen_runtime_inputs=967,additional_python_tests=10,sdk_binary_sha256=s['binary_sha256'],original_java_jar_sha256=s['java_jar_sha256'],scope=s['scope'],files=[dict(path=p.relative_to(BASE).as_posix(),sha256=sha(p),bytes=p.stat().st_size) for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'manifest.json']);save(BASE/'manifest.json',manifest);print(json.dumps(dict(expected_defect_reproduced=True,recovery_certified=False,manifest_sha256=sha(BASE/'manifest.json'),files=len(manifest['files']))))
