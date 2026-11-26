"""Bind actual quorum, current-term reload, original business identity and retained locks."""
import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('business_raft_completion',ROOT/'tools/java-raft-reload-candidate/checks.py');completion=importlib.util.module_from_spec(spec);spec.loader.exec_module(completion)

def verify_stage(bundle,phase,final=False):
 try:
  nodes=bundle['nodes'];index=str(bundle['leader']);leader=nodes[index];confirmation=bundle['confirmation']
  if not 2<=len(nodes)<=3 or len({n['pid'] for n in nodes.values()})!=len(nodes):raise RuntimeError('Actual live quorum/process identities missing')
  if set(bundle['processes'])!=set(nodes):raise RuntimeError('Actual process identities do not cover this live quorum')
  if [key for key,node in nodes.items() if node['leader'] is True]!=[index]:raise RuntimeError('Actual unique leader missing')
  completion.verify_completion(leader);completion.verify_completion(confirmation)
  for key in ['pid','term','node_id','leader_id','raft_peer_id','reload_complete_term']:
   if leader[key]!=confirmation[key]:raise RuntimeError('Leader changed across repeated current-term samples')
  original=None
  for key,node in nodes.items():
   identity=bundle['processes'][key];peer=leader['raft_peer_id'].rsplit(':',1)[0]+':'+str(28292+2*int(key))
   if node['pid']!=identity['pid'] or node['raft_peer_id']!=identity['peer_id'] or node['raft_peer_id']!=peer:raise RuntimeError('Replica peer/process identity differs')
   if node['raft_term_before']!=leader['term'] or node['raft_term_after']!=leader['term']:raise RuntimeError('Replica actual term changed or disagrees with current leader')
   if any(node[k] is not (key==index) for k in ['raft_leader_before','raft_leader_after']):raise RuntimeError('Replica actual leadership flags contradict this quorum')
   if any(node[k]!=leader['raft_peer_id'] for k in ['raft_leader_id_before','raft_leader_id_after']):raise RuntimeError('Replica actual leader identity changed')
   if node['leader_id']!=leader['raft_peer_id'] or node['applied']<leader['committed']:raise RuntimeError('Replica leadership/applied progress differs')
   if phase is None:
    if node['all_sessions']!=0 or node['records']:raise RuntimeError('Fresh business quorum must have no injected transactions')
    continue
   if node['all_sessions']!=(0 if final else 1) or len(node['records'])!=(0 if final else 1) or node['lockable'] is not final:raise RuntimeError('Actual original session/lock state differs')
   if final:continue
   record=node['records'][0];branches=record['branches']
   if record['xid']!=phase['xid'] or record['status']!=17 or len(branches)!=1:raise RuntimeError('Actual timeout identity/state differs')
   if key==index and record['active'] is not False:raise RuntimeError('Current timeout leader retained an active transaction')
   branch=branches[0]
   if (branch['branch_id'],branch['xid'],branch['type'],branch['resource'],branch['lock_key'])!=(phase['branch_id'],phase['xid'],0,phase['resource_id'],'ACCOUNTS:1;'):raise RuntimeError('Actual original branch identity/resource differs')
   # active是非复制的本地生命周期字段；比较复制语义和完整分支上下文。
   replicated={k:v for k,v in record.items() if k!='active'}
   if original is not None and replicated!=original:raise RuntimeError('Replicas disagree on original transaction/branch context')
   original=replicated
 except (KeyError,TypeError,ValueError) as error:raise RuntimeError('Incomplete RAFT stage evidence') from error
 return bundle

def verify_transition(before,after):
 try:
  old=before['nodes'][str(before['leader'])];new=after['nodes'][str(after['leader'])]
  completion.verify_completion(new)
  if new['raft_peer_id']==old['raft_peer_id'] or new['term']<=old['term'] or new['pid']==old['pid']:raise RuntimeError('Actual different leader with a greater term required')
 except (KeyError,TypeError,ValueError) as error:raise RuntimeError('Incomplete actual election evidence') from error
 return True
