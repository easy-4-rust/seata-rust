"""Combine original SQL/RPC assertions with actual elections, replicas, logs and snapshots."""
import hashlib,importlib.util,json,os,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
business=module('raft_full_business_parser',HERE/'business_evidence.py');checks=module('raft_full_cluster_checks',HERE/'cluster_checks.py');validate=module('raft_full_business_checks',HERE/'verify.py').validate

def parse_metadata(raw):
 prefix=b'SNAPSHOT_METADATA_JSON=';lines=[line[len(prefix):] for line in raw.splitlines() if line.startswith(prefix)]
 if len(lines)!=1:raise RuntimeError('One explicit protobuf reader result is required')
 try:return json.loads(lines[0])
 except (ValueError,TypeError) as error:raise RuntimeError('Malformed protobuf reader result') from error

def derive(directory,archive_metadata=False):
 directory=Path(directory);proof=validate(business.derive(directory));phase=proof['phase'];runtime=json.loads((directory/'runtime.json').read_bytes())
 if sorted((directory/'business-tables.txt').read_text().splitlines())!=['accounts','commit_account','commit_reservation','undo_log']:raise RuntimeError('RAFT TC state must not live in the business database')
 bundles={}
 for name in ['timeout','election_one','lost','restored','election_two','final','final_replicated']:
  bundle=json.loads((directory/('cluster-'+name+'.json')).read_bytes());checks.verify_stage(bundle,phase,name.startswith('final'),name in ['lost','restored','election_two']);bundles[name]=bundle
 for first,last in [('timeout','election_one'),('restored','election_two')]:checks.verify_transition(bundles[first],bundles[last])
 original=bundles['timeout']['nodes'][str(bundles['timeout']['leader'])]['records'][0]
 for name,bundle in bundles.items():
  if name.startswith('final'):continue
  for node in bundle['nodes'].values():
   record=node['records'][0];expected_ids={phase['branch_id'],phase['tcc_branch_id']} if name in ['timeout','election_one'] else {phase['tcc_branch_id']}
   if {k:v for k,v in record.items() if k not in ['active','branches']}!={k:v for k,v in original.items() if k not in ['active','branches']}:raise RuntimeError('Original replicated global session changed across commit elections')
   actual_branches={b['branch_id']:b for b in record['branches']};original_branches={b['branch_id']:b for b in original['branches']}
   if set(actual_branches)!=expected_ids or any(actual_branches[key]!=original_branches[key] for key in expected_ids):raise RuntimeError('Original pending commit branch identity/context changed across elections')
 if len(runtime['all_tc_pids'])!=5 or len(set(runtime['all_tc_pids']))!=5:raise RuntimeError('Three original plus two restarted TC JVMs required')
 origins=[]
 for stage,index in [('initial',0),('operator_loss',1),('operator_replay',2)]:
  route=json.loads((directory/stage/'route.json').read_bytes());source=Path(route['source_log']);raw=source.read_bytes()
  if route['pid']!=runtime['tc_pids'][index] or raw!=(directory/stage/'tc-stdout.log').read_bytes():raise RuntimeError('Business stage TC logs do not belong to actual routed leader')
  start=json.loads((directory/f'cluster-{stage}_sdk_start.json').read_bytes());checks.verify_stage(start,None if index==0 else phase,partial=index==2);leader=start['nodes'][str(start['leader'])]
  if route['node']!=start['leader'] or (route['pid'],route['peer'],route['term'])!=(leader['pid'],leader['raft_peer_id'],leader['term']):raise RuntimeError('Actual SDK route does not bind the current confirmed leader term')
  for name in [['timeout'],['restart','lost'],['second_restart','final']][index]:
   actual=json.loads((directory/('business-leader-'+name+'.json')).read_bytes());checks.completion.verify_completion(actual)
   if (actual['pid'],actual['term'],actual['raft_peer_id'])!=(route['pid'],route['term'],route['peer']):raise RuntimeError('Actual business observation belongs to a different leader term')
   after=json.loads((directory/('business-leader-after-'+name+'.json')).read_bytes());checks.completion.verify_completion(after)
   if any(after[key]!=actual[key] for key in ['pid','term','raft_peer_id']):raise RuntimeError('Actual business observation spans a leader change')
  if index:
   for when in ['before','after']:
    actual=json.loads((directory/stage/('operator-leader-'+when+'.json')).read_bytes());checks.completion.verify_completion(actual)
    if (actual['pid'],actual['term'],actual['raft_peer_id'])!=(route['pid'],route['term'],route['peer']):raise RuntimeError('Operator did not remain on the selected current leader')
  node=route['node'];process=json.loads((source.parent/'process.json').read_bytes())
  if process['pid']!=route['pid'] or process['index']!=node or route['port']!=28291+2*node:raise RuntimeError('Proxy route does not belong to selected owned node')
  events=json.loads((directory/stage/'proxy-events.json').read_bytes())
  if any(e.get('backend_port')!=route['port'] for e in events if e['event']=='connected'):raise RuntimeError('Actual SDK connection targets a different TC')
  config=(directory/stage/'application.yml').read_bytes()
  if config!=(source.parent/'application.yml').read_bytes() or b'  store:\n    mode: raft\n    session:\n      mode: raft\n    lock:\n      mode: raft\n' not in config:raise RuntimeError('Actual leader RAFT configuration differs')
 actual_logs=list(directory.glob('node_*_generation_*/tc-stdout.log'));log_pids=[];restarts=[]
 for path in actual_logs:
  text=path.read_text(encoding='utf-8',errors='replace');pid=json.loads((path.parent/'process.json').read_bytes())['pid']
  if pid not in runtime['all_tc_pids']:raise RuntimeError('Unexpected actual TC process')
  log_pids.append(pid)
  if '_generation_1' in path.parent.name:restarts.append(path)
  process=json.loads((path.parent/'process.json').read_bytes());location=next(arg.split('file:',1)[1] for arg in process['command'] if arg.startswith('-Dspring.config.location=file:'));config=(path.parent/'application.yml').read_bytes()
  if Path(location).read_bytes()!=config:raise RuntimeError('Actual node configuration changed or no longer matches runtime command')
  for cls in ['org.apache.seata.server.cluster.raft.RaftStateMachine','org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.cluster.raft.RaftBusinessControl']:
   lines=[line for line in text.splitlines() if ' '+cls+' source: ' in line]
   if len(lines)!=1 or 'file:/'+(directory/'overlay').as_posix()+'/' not in lines[0]:raise RuntimeError('Actual candidate class source differs')
   origins.append(dict(pid=pid,type=cls,line=lines[0]))
  if '_generation_1' in path.parent.name and 'on snapshot load end index:' not in text:raise RuntimeError('Restart did not demonstrate actual RAFT snapshot load')
  if 'NOT properly handled CommitRetryTimeout' in text or 'asynchronous session reload failed' in text:raise RuntimeError('Actual candidate reload still fails')
 if len(actual_logs)!=5 or set(log_pids)!=set(runtime['all_tc_pids']) or len(restarts)!=2:raise RuntimeError('Five actual TC generations and both restart logs required')
 for bundle in list(bundles.values())+[json.loads((directory/'cluster-ready.json').read_bytes())]:
  for key,identity in bundle['processes'].items():
   path=Path(identity['process_file']);process=json.loads(path.read_bytes())
   if not path.is_relative_to(directory) or process['pid']!=identity['pid'] or process['index']!=int(key):raise RuntimeError('Observed replica does not match actual owned generation process')
 metadata=[];snapshot_audits={}
 for stage in ['timeout','lost']:
  for node in range(3):
   observation=json.loads((directory/f'snapshot-{stage}-node_{node}.json').read_bytes());audit=json.loads((directory/f'snapshot-files-{stage}-node_{node}.json').read_bytes());before=observation['before'];after=observation['after'];snapshot=audit['snapshot']
   generation=bundles['timeout' if stage=='timeout' else 'restored']['nodes'][str(node)]
   if any(after[key]!=generation[key] or before[key]!=generation[key] for key in ['pid','raft_peer_id']):raise RuntimeError('Snapshot completed on a different node generation')
   if audit['node']!=node or audit['pid']!=after['pid'] or after['pid']!=before['pid'] or snapshot!=after['snapshot'] or snapshot['index']<before['applied'] or snapshot['index']>after['applied'] or not 0<snapshot['term']<=after['raft_term_after']:raise RuntimeError('Actual completed snapshot cut point differs from original state')
   if before['records']!=after['records'] or before['all_sessions']!=1 or before['records'][0]['xid']!=phase['xid']:raise RuntimeError('Snapshot did not preserve this original timeout transaction')
   root=directory/f'snapshot-data-{stage}-node_{node}';files=audit['files'];paths={r['path'] for r in files}
   if len(paths)!=len(files) or not {'__raft_snapshot_meta','session'}<=paths or paths!={p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}:raise RuntimeError('Actual snapshot session/metadata files missing')
   for row in files:
    relative=Path(row['path'])
    if relative.is_absolute() or '..' in relative.parts or ':' in row['path'] or '\\' in row['path']:raise RuntimeError('Snapshot path is not owned')
    raw=(root/relative).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('Frozen actual snapshot bytes differ')
   metadata.append(root/'__raft_snapshot_meta');snapshot_audits[(stage,node)]=audit
 support=module('raft_snapshot_read_support',HERE.parents[1]/'tools/java-tc-integration/backend_recovery/run.py');cp=(directory/'effective-classpath.txt').read_text().strip();raw=support.command([support.JAVA,'-Djava.io.tmpdir=D:/Temp','-cp',str(directory/'compiled')+os.pathsep+cp,'SnapshotMetadataReader']+metadata).stdout
 parsed=parse_metadata(raw)
 if archive_metadata:
  with (directory/'snapshot-metadata-reader-output.log').open('xb') as stream:stream.write(raw)
 if parsed!=[dict(index=audit['snapshot']['index'],term=audit['snapshot']['term']) for audit in snapshot_audits.values()]:raise RuntimeError('Archived real protobuf metadata index/term differs from claimed snapshots')
 loaded=[]
 for label,stage in [('first','timeout'),('second','lost')]:
  resume=json.loads((directory/('resume-'+label+'.json')).read_bytes());node=resume['node'];audit=snapshot_audits[(stage,node)];path=directory/f'node_{node}_generation_1/tc-stdout.log';old=json.loads(Path(resume['stopped_process']).read_bytes());new=json.loads((path.parent/'process.json').read_bytes());index=audit['snapshot']['index']
  if resume['snapshot_stage']!=stage or resume['snapshot']!=audit['snapshot'] or resume['files']!=audit['files'] or old['pid']==new['pid'] or old['index']!=node or new['index']!=node:raise RuntimeError('Restart did not consume this original node snapshot')
  if Path(audit['snapshot']['directory']).name!='snapshot_'+str(index) or not re.search(r'on snapshot load end index:\s*'+str(index)+r'(?:\D|$)',path.read_text(encoding='utf-8',errors='replace')):raise RuntimeError('Actual restored snapshot index does not match previous owned snapshot')
  if (Path(resume['stopped_process']).parent/'application.yml').read_bytes()!=(path.parent/'application.yml').read_bytes() or hashlib.sha256((path.parent/'application.yml').read_bytes()).hexdigest()!=resume['configuration_sha256']:raise RuntimeError('Restart node configuration/disk identity changed')
  loaded.append(dict(node=node,index=index,old_pid=old['pid'],new_pid=new['pid']))
 for label in ['election_one','election_two']:
  audit=json.loads((directory/('reload-'+label+'.json')).read_bytes());events=json.loads((directory/(label+'-exceptions.json')).read_bytes())['recording']['events'];leader=bundles[label]['nodes'][str(bundles[label]['leader'])]
  receipt=json.loads((directory/('leader-stop-'+label+'.json')).read_bytes());old=json.loads(Path(receipt['process_file']).read_bytes());checks.completion.verify_completion(receipt['before'])
  if receipt['pid']!=old['pid'] or receipt['node']!=old['index'] or receipt['returncode'] is None or receipt['pid']==leader['pid'] or audit['boundary']!=receipt['boundary'] or receipt['boundary']>receipt['stopped_at']:raise RuntimeError('Actual JFR window does not bind the stopped previous leader')
  stop_binding=module('raft_commit_stop_identity',HERE/'stop_binding.py');prior=bundles['timeout' if label=='election_one' else 'restored'];stop_binding.verify_stop_binding(receipt,prior,bundles[label],old)
  actual=support.command([support.JAVA,'-Djava.io.tmpdir=D:/Temp','-Dfile.encoding=UTF-8','-cp',directory/'compiled','ReadRaftExceptions',directory/(label+'.jfr')]).stdout
  if json.loads(actual)!=json.loads((directory/(label+'-exceptions.json')).read_bytes()):raise RuntimeError('Archived exception JSON does not reproduce the actual JFR')
  if audit['pid']!=leader['pid'] or audit['node']!=bundles[label]['leader'] or audit['exceptions'] or checks.completion.reload_exceptions(events,16,audit['boundary']):raise RuntimeError('Actual election reload exception audit differs')
  if hashlib.sha256((directory/(label+'.jfr')).read_bytes()).hexdigest()!=audit['jfr_sha256'] or hashlib.sha256((directory/'compiled/ReadRaftExceptions.class').read_bytes()).hexdigest()!=audit['reader_sha256']:raise RuntimeError('Actual JFR/read class bytes differ')
 return dict(business=proof,cluster=bundles,actual_class_origins=origins,actual_tc_jvms=len(actual_logs),actual_elections=2,actual_snapshot_loads=loaded,snapshot_metadata=parsed)
