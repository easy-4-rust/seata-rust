"""Real TC/SDK response loss with natural20s deadline; original JAR plus certified Java overlay."""
import datetime
import importlib.util
import json
from pathlib import Path
import subprocess
from collections import Counter
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
audit=module('inflight_live_audit',HERE/'audit.py');sdk=audit.sdk;proxy=module('inflight_live_proxy',HERE/'proxy.py');java=module('inflight_java_runtime',ROOT/'tools/java-preface-candidate/runtime.py');prep=module('inflight_java_prepare',ROOT/'tools/java-preface-candidate/prepare_candidate.py')
def verify(directory,proof):
 result=json.loads((directory/'proxy-result.json').read_bytes());events=json.loads((directory/'proxy-events.json').read_bytes())
 if result['connections']!=4 or result['errors'] or result['fragmented_directions']!=['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Fault proxy connections/errors differ: '+str(result))
 if {r['label'] for r in result['faults']}!={'begin-close','commit-close','rollback-close','begin-timeout'} or len(result['faults'])!=4:raise RuntimeError('Need exactly four actual suppressed successful TC responses')
 wires=[e for e in events if e['event']=='wire'];requests=[e for e in wires if e['direction']=='sdk-to-tc' and e['kind'] in [0,2]];responses=[e for e in wires if e['direction']=='tc-to-sdk' and e['kind']==1]
 queries=proof['known_status_queries'];total=12+queries
 if len(requests)!=total or len(responses)!=total or Counter(e['payload_type'] for e in requests)!=Counter({101:4,1:4,7:1,9:3,15:queries}):raise RuntimeError('Actual SDK mutating/query wire inventory differs')
 if Counter(e['payload_type'] for e in responses)!=Counter({102:4,2:4,8:1,10:3,16:queries}):raise RuntimeError('Actual TC response inventory differs')
 expected=dict(real_getty_loop=True,connections=4,natural_timeout_seconds=20,transaction_resends=0,owned_future_ids_checked=128,owned_futures_remaining=0)
 if any(proof.get(key)!=value for key,value in expected.items()) or len(set(proof['xids']))!=4:raise RuntimeError('SDK actual observations differ')
 if {r['label'] for r in proof['unknown']}!={r['label'] for r in result['faults']}:raise RuntimeError('Typed unknown observations differ')
 for unknown in proof['unknown']:
  if unknown['outcome']!='UnknownOutcome' or unknown['deadline']!=(unknown['label']=='begin-timeout'):raise RuntimeError('Unknown cause classification differs')
  actual=next(r for r in result['faults'] if r['label']==unknown['label'])
  if actual['id']!=unknown['id']:raise RuntimeError('Caller unknown not correlated to suppressed response')
  if unknown['deadline'] and not 19000<=unknown['elapsed_ms']<30000:raise RuntimeError('Natural deadline was shortened')
 if sum(e['event']=='late-original-response' for e in events)!=1:raise RuntimeError('Late original timeout response missing')
 history=proof['commit_statuses']
 if not history or history[-1]!=15 or any(state not in [8,9] for state in history[:-1]) or queries!=5+len(history):raise RuntimeError('TC async commit reconciliation history differs')
 request_by_id={(r['connection'],r['id']):r for r in requests};observed_commit=[]
 for response in responses:
  if response['payload_type']!=16:continue
  request=request_by_id[(response['connection'],response['id'])];raw=bytes.fromhex(request['frame_hex']);body=raw[int.from_bytes(raw[7:9],'big'):];length=int.from_bytes(body[2:4],'big');xid=body[4:4+length].decode('utf-8')
  if xid==proof['xids'][1]:
   raw=bytes.fromhex(response['frame_hex']);body=raw[int.from_bytes(raw[7:9],'big'):]
   if len(body)!=5 or body[2]!=1:raise RuntimeError('Commit query TC result not authoritative success')
   observed_commit.append(body[4])
 if observed_commit!=history:raise RuntimeError('SDK commit history differs from actual TC response bytes')
 for number in range(1,5):
  registrations=[e for e in responses if e['connection']==number and e['payload_type']==102]
  if len(registrations)!=1:raise RuntimeError('Each reconnect must actually register')
 pings={e['id'] for e in wires if e['connection']==4 and e['direction']=='sdk-to-tc' and e['kind']==3};pongs={e['id'] for e in wires if e['connection']==4 and e['direction']=='tc-to-sdk' and e['kind']==4}
 if not pings or not pongs or not pongs.issubset(pings):raise RuntimeError('Healthy natural-timeout connection heartbeat absent')
 tc=(directory/'tc-stdout.log').read_bytes()
 for name in ['inflight-begin-close','inflight-commit-source','inflight-rollback-source','inflight-begin-timeout']:
  if tc.count(('transactionName: '+name+',').encode())!=1:raise RuntimeError('Begin mutation resent/missing: '+name)
 for xid in [proof['xids'][0],proof['xids'][2],proof['xids'][3]]:
  if ('Rollback global transaction successfully, xid = '+xid).encode() not in tc:raise RuntimeError('Original rollback TC confirmation missing')
 return dict(requests=total,responses=total,connections=4,mutating_begins=4,mutating_commits=1,mutating_rollbacks=3,status_queries=queries,typed_unknown=4,late_original=1,transaction_resends=0,commit_statuses=history)
def run(directory,work,parent):
 h=sdk.h;l=sdk.l;directory.mkdir(exist_ok=False);runtime=Path('D:/codex-builds')/('inflight-live-'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S'));runtime.mkdir(exist_ok=False)
 if h.port_open(28091) or h.port_open(28092):raise RuntimeError('Owned live ports already occupied')
 for row in json.loads((parent/'overlay-manifest.json').read_bytes()):
  if h.sha(parent/'overlay'/row['path'])!=row['sha256']:raise RuntimeError('Certified Java class overlay drift')
 audit.run(work);binary=directory/'sdk-inflight.exe';l.run_command(['go','build','-o',binary,HERE/'live_main.go'],work,sdk.c.GO_ENV,directory/'go-build.log')
 config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091');h.write(runtime/'application.yml',config);h.write(directory/'application.yml',config)
 env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=runtime.as_posix();jar=sdk.c.ORACLE/'server/target/seata-server.jar'
 if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original Java JAR drift')
 tc=None;relay=None;success=False;proof=None;wire=None;sources=None
 try:
  tc=h.spawn([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dspring.config.location=file:'+(runtime/'application.yml').as_posix(),'-verbose:class','-Dloader.path='+(parent/'overlay').as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1'],directory,'tc',runtime,env)
  h.wait_until(lambda:h.port_open(28091),90,'Owned inflight TC startup',tc);relay=proxy.InflightProxy(runtime,directory);relay.start()
  result=subprocess.run([str(binary),str(runtime)],cwd=runtime,env=env,capture_output=True,creationflags=h.FLAGS,timeout=95);h.write(directory/'client.log',result.stdout+result.stderr)
  relay.close();relay=None
  if (runtime/'sdk.log').exists():h.write(directory/'sdk.log',(runtime/'sdk.log').read_bytes())
  rows=[json.loads(line.split(b'INFLIGHT_LIVE_PASS ',1)[1]) for line in result.stdout.splitlines() if line.startswith(b'INFLIGHT_LIVE_PASS ')]
  if result.returncode or len(rows)!=1:raise RuntimeError('Real inflight SDK scenario failed')
  proof=rows[0];wire=verify(directory,proof);sources=java.class_sources(directory/'tc-stdout.log',parent/'overlay',prep);success=True
 finally:
  if relay:relay.close()
  h.stop(tc);h.wait_until(lambda:not h.port_open(28091) and not h.port_open(28092),10,'Owned inflight cleanup')
  state=dict(succeeded=success,tc_pid=tc.pid if tc else None,runtime=str(runtime),ports_released=not h.port_open(28091) and not h.port_open(28092),original_jar_sha256=h.sha(jar),sdk_binary_sha256=h.sha(binary),source_sha256=h.sha(HERE/'live_main.go'),java_overlay_parent=str(parent),class_sources=sources,proof=proof,wire_inventory=wire,scope='Explicit synchronous SDK candidate. FILE/auth-disabled TC with independently certified Java preface overlay. Begin XID recovery uses test-only observer; no general Begin idempotency, real RM/Undo, production Rust, full client or WP00 certification.')
  h.write(directory/'result.json',json.dumps(state,indent=2)+'\n')
 return state
