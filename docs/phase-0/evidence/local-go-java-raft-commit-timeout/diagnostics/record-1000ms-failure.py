from pathlib import Path
import hashlib,json,re,datetime
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-go-java-raft-commit-timeout';OUT=BASE/'run_20261113_054700_4aed6e96'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def put(path,data):
 data=(json.dumps(data,indent=2)+'\n').encode();path.parent.mkdir(parents=True,exist_ok=True)
 if path.exists():assert path.read_bytes()==data
 else:
  with path.open('xb') as stream:stream.write(data)
result=json.loads((OUT/'result.json').read_bytes());stop=json.loads((OUT/'leader-stop-election_two.json').read_bytes());restored=json.loads((OUT/'cluster-restored.json').read_bytes());assert not result['succeeded'] and result['container_state']=='exited' and result['ports_released']
assert restored['leader']==stop['node']==2
cutoff=datetime.datetime.fromisoformat(stop['boundary']).astimezone(datetime.timezone(datetime.timedelta(hours=8))).strftime('%H:%M:%S.%f')[:12]
observations=[];term_sets=[]
for node,generation in [(0,0),(1,1)]:
 path=OUT/f'node_{node}_generation_{generation}/tc-stdout.log';config=path.parent/'application.yml';assert b'election-timeout-ms: 1000' in config.read_bytes();text=path.read_text(encoding='utf-8',errors='replace');records=[]
 for line in text.splitlines():
  match=re.search(r'Node <default/192\.168\.1\.10:'+str(28292+2*node)+r'> start vote and grant vote self, term=(\d+)',line)
  if match and line[:12]>=cutoff:records.append(dict(term=int(match.group(1)),line=line,line_sha256=hashlib.sha256(line.encode()).hexdigest()))
 assert records;term_sets.append({r['term'] for r in records});observations.append(dict(node=node,generation=generation,log=str(path),sha256=sha(path),config_sha256=sha(config),self_vote_records=records,stopped_peer_connect_timeout_mentions=text.count('connection timed out after 1000 ms: /192.168.1.10:28296')))
shared=sorted(term_sets[0]&term_sets[1]);assert shared
put(BASE/'election-1000ms-failure.json',dict(status='OPEN',primary_failed_run=OUT.name,election_timeout_ms=1000,failed_stage='election_two',observation_timeout_seconds=60,stopped_previous_leader=stop,actual_self_vote_term_overlap=shared,observations=observations,resources_stopped=True,facts='After the confirmed leader node2 stopped, the two remaining local nodes repeatedly granted votes to themselves in overlapping terms. No state satisfying unique-leader/current-term successful-reload gates was obtained in 60 seconds; runtime was terminal failed and owned resources stopped.',hypothesis='Connection attempts to the stopped peer take about the same time as the configured election window and may contribute to repeated split votes; not established as root cause.',separate_3000ms_profile_does_not_close_this=True))
print(json.dumps(dict(succeeded=True,failed_run=OUT.name,shared_self_vote_terms=shared)))
