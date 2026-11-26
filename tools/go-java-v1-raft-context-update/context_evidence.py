"""Require original acknowledged Prepare data at every replica, callback and SQL row."""
import hashlib,importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
wire=module('updated_context_actual_wire',ROOT/'tools/go-java-v1-real-at/verify.py')
def application_data(value):
 frame=value['frame'];head=int.from_bytes(frame[7:9],'big');reader=wire.Reader(frame[head+2:]);reader.string()
 if value['code']==11:reader.integer(1);reader.string();reader.string(4)
 else:reader.integer(8);reader.integer(1);reader.string()
 data=reader.string(4);reader.end();return data if data else None
def verify_updated_context(phase,registered,reported,replicas,callbacks,prepared,accepted):
 if not reported or registered==reported:raise RuntimeError('Prepare did not change original acknowledged context')
 try:old=json.loads(registered)['actionContext'];new=json.loads(reported)['actionContext']
 except (TypeError,ValueError,KeyError) as error:raise RuntimeError('Original business context missing') from error
 token=hashlib.sha256((phase['xid']+'/'+str(phase['tcc_branch_id'])+'/10').encode()).hexdigest();expected={'prepared-units':'10','prepared-branch':str(phase['tcc_branch_id']),'prepared-token':token}
 if any(key in old or new.get(key)!=value for key,value in expected.items()):raise RuntimeError('Acknowledged report lacks actual generated Prepare values')
 if len(replicas)!=12 or any(value!=reported for value in replicas):raise RuntimeError('Replicas do not retain actual acknowledged Prepare context')
 if len(callbacks)!=3 or any(value!=reported for value in callbacks):raise RuntimeError('Original/elected/replayed callback context differs from acknowledged report')
 row=[phase['xid'],str(phase['tcc_branch_id']),'10',token]
 if len(prepared)!=5 or any(value!=row for value in prepared):raise RuntimeError('Durable prepared SQL row differs from original acknowledged context')
 if accepted!=[token,token]:raise RuntimeError('Actual loss/replay business did not accept original prepared token')
 return dict(updated_prepare_values=expected,replica_report_bindings=12,callback_stage_bindings=3,durable_context_observations=5,business_callback_acceptances=2)

def derive(directory,proof):
 directory=Path(directory);phase=proof['business']['phase'];runtime=json.loads((directory/'runtime.json').read_bytes());registered=None;callbacks=[];callback_counts=[]
 reports=[row for row in proof['business']['original_branch_reports'] if row['branch_type']==1]
 if len(reports)!=1 or reports[0]['accepted'] is not True:raise RuntimeError('One original acknowledged TCC report required')
 reported=reports[0]['application_data']
 for stage in ['initial','operator_loss','operator_replay']:
  contexts=[]
  for event in json.loads((directory/stage/'proxy-events.json').read_bytes()):
   if event['event']!='wire':continue
   value=wire.decode(event)
   if stage=='initial' and value['code']==11 and value['branch_type']==1:
    if registered is not None:raise RuntimeError('Ambiguous original TCC registration')
    registered=application_data(value)
   if value['code']==3 and value['branch_type']==1:
    data=application_data(value)
    if data!=reported:raise RuntimeError('Actual raw TCC callback lost updated Prepare context')
    contexts.append(data)
  if not contexts:raise RuntimeError('Actual TCC callback stage missing')
  callbacks.append(contexts[0]);callback_counts.append(len(contexts))
 replicas=[]
 for stage in ['timeout','election_one','lost','restored','election_two']:
  for node in proof['cluster'][stage]['nodes'].values():
   branch=next(b for b in node['records'][0]['branches'] if b['branch_id']==phase['tcc_branch_id']);replicas.append(branch['application_data'])
 prepared=[]
 for name in ['timeout','restart','lost','second_restart','final']:
  rows=[row.split('\t') for row in (directory/('prepared-context-'+name+'.tsv')).read_text().splitlines()]
  if len(rows)!=1:raise RuntimeError('One actual durable prepared context row required')
  prepared.append(rows[0])
 token=hashlib.sha256((phase['xid']+'/'+str(phase['tcc_branch_id'])+'/10').encode()).hexdigest();accepted=[]
 for index,stage in enumerate(['initial','operator_loss','operator_replay']):
  log=(directory/stage/'sdk-stdout.log').read_text(encoding='utf-8',errors='replace');pid=runtime['sdk_pids'][index]
  if 'REAL_TCC_CONTEXT_REJECTED ' in log:raise RuntimeError('Real business rejected prepared context')
  prefix='REAL_TCC_CONTEXT_PREPARED' if index==0 else 'REAL_TCC_CONTEXT_ACCEPTED';expected=f'{prefix} {phase["xid"]} {phase["tcc_branch_id"]} {token} {pid}'
  lines=[line.strip() for line in log.splitlines() if line.startswith(prefix+' ')]
  if lines!=[expected]:raise RuntimeError('Actual prepared/accepted marker differs from original branch, token or SDK process')
  if index:accepted.append(token)
 result=verify_updated_context(phase,registered,reported,replicas,callbacks,prepared,accepted);origins=[]
 classes=['org.apache.seata.server.storage.raft.session.RaftSessionManager','org.apache.seata.server.cluster.raft.sync.msg.dto.BranchTransactionDTO','org.apache.seata.server.cluster.raft.execute.branch.UpdateBranchSessionExecute']
 for path in directory.glob('node_*_generation_*/tc-stdout.log'):
  text=path.read_text(encoding='utf-8',errors='replace');pid=json.loads((path.parent/'process.json').read_bytes())['pid']
  for name in classes:
   lines=[line for line in text.splitlines() if ' '+name+' source: ' in line]
   if len(lines)!=1 or 'file:/'+(directory/'overlay').as_posix()+'/' not in lines[0]:raise RuntimeError('Real JVM did not load tested context replication candidate')
   origins.append(dict(pid=pid,type=name,line=lines[0]))
 if len(origins)!=15:raise RuntimeError('All five actual JVM generations must load three context candidate classes')
 result.update(registered_tcc_context=registered,acknowledged_report_tcc_context=reported,actual_tcc_callback_counts=callback_counts,actual_context_class_origins=origins,uniform_patched_nodes_only=True,mixed_version_upgrade_certified=False,historic_lost_data_reconstruction_certified=False)
 return result
