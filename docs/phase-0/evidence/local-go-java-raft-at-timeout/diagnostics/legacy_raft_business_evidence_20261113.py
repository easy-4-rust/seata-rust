"""Combine original SQL/RPC assertions with actual elections, replicas, logs and snapshots."""
import hashlib,importlib.util,json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
business=module('raft_full_business_parser',HERE/'business_evidence.py');checks=module('raft_full_cluster_checks',HERE/'cluster_checks.py');validate=module('raft_full_business_checks',HERE/'verify.py').validate

def derive(directory):
 directory=Path(directory);proof=validate(business.derive(directory));phase=proof['phase'];runtime=json.loads((directory/'runtime.json').read_bytes())
 if sorted((directory/'business-tables.txt').read_text().splitlines())!=['accounts','undo_log']:raise RuntimeError('RAFT TC state must not live in the business database')
 bundles={}
 for name in ['timeout','election_one','lost','restored','election_two','final','final_replicated']:
  bundle=json.loads((directory/('cluster-'+name+'.json')).read_bytes());checks.verify_stage(bundle,phase,name.startswith('final'));bundles[name]=bundle
 for first,last in [('timeout','election_one'),('restored','election_two')]:checks.verify_transition(bundles[first],bundles[last])
 original=bundles['timeout']['nodes'][str(bundles['timeout']['leader'])]['records'][0]
 for name,bundle in bundles.items():
  if name.startswith('final'):continue
  for node in bundle['nodes'].values():
   if {k:v for k,v in node['records'][0].items() if k!='active'}!={k:v for k,v in original.items() if k!='active'}:raise RuntimeError('Original replicated branch/session changed across elections')
 if len(runtime['all_tc_pids'])!=5 or len(set(runtime['all_tc_pids']))!=5:raise RuntimeError('Three original plus two restarted TC JVMs required')
 origins=[]
 for stage,index in [('initial',0),('operator_loss',1),('operator_replay',2)]:
  route=json.loads((directory/stage/'route.json').read_bytes());source=Path(route['source_log']);raw=source.read_bytes()
  if route['pid']!=runtime['tc_pids'][index] or raw!=(directory/stage/'tc-stdout.log').read_bytes():raise RuntimeError('Business stage TC logs do not belong to actual routed leader')
  node=route['node'];process=json.loads((source.parent/'process.json').read_bytes())
  if process['pid']!=route['pid'] or process['index']!=node or route['port']!=28291+2*node:raise RuntimeError('Proxy route does not belong to selected owned node')
  events=json.loads((directory/stage/'proxy-events.json').read_bytes())
  if any(e.get('backend_port')!=route['port'] for e in events if e['event']=='connected'):raise RuntimeError('Actual SDK connection targets a different TC')
  config=(directory/stage/'application.yml').read_bytes()
  if config!=(source.parent/'application.yml').read_bytes() or b'  store:\n    mode: raft\n    session:\n      mode: raft\n    lock:\n      mode: raft\n' not in config:raise RuntimeError('Actual leader RAFT configuration differs')
 for path in directory.glob('node_*_generation_*/tc-stdout.log'):
  text=path.read_text(encoding='utf-8',errors='replace');pid=json.loads((path.parent/'process.json').read_bytes())['pid']
  if pid not in runtime['all_tc_pids']:raise RuntimeError('Unexpected actual TC process')
  for cls in ['org.apache.seata.server.cluster.raft.RaftStateMachine','org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.cluster.raft.RaftBusinessControl']:
   lines=[line for line in text.splitlines() if ' '+cls+' source: ' in line]
   if len(lines)!=1 or 'file:/'+(directory/'overlay').as_posix()+'/' not in lines[0]:raise RuntimeError('Actual candidate class source differs')
   origins.append(dict(pid=pid,type=cls,line=lines[0]))
  if '_generation_1' in path.parent.name and 'on snapshot load end index:' not in text:raise RuntimeError('Restart did not demonstrate actual RAFT snapshot load')
  if 'NOT properly handled RollbackRetryTimeout' in text or 'asynchronous session reload failed' in text:raise RuntimeError('Actual candidate reload still fails')
 return dict(business=proof,cluster=bundles,actual_class_origins=origins,actual_tc_jvms=5,actual_elections=2,actual_snapshot_loads=2)
