"""Unit-only cluster evidence examples, never a substitute for live consensus evidence."""
import copy,unittest
from cluster_checks import verify_stage,verify_transition

class ClusterGuards(unittest.TestCase):
 def good(self):
  record=dict(xid='127.0.0.1:28291:7',status=16,active=False,begin_time=100,branches=[dict(branch_id=8,xid='127.0.0.1:28291:7',type=0,status=2,resource='owned',lock_key='ACCOUNTS:1;',application_data=None),dict(branch_id=9,xid='127.0.0.1:28291:7',type=1,status=1,resource='owned-real-commit-action',lock_key='',application_data='original-tcc-context')])
  nodes={}
  for i in range(3):
   peer=f'127.0.0.1:{28292+i*2}';leader=i==0
   nodes[str(i)]=dict(pid=100+i,leader=leader,leader_id='127.0.0.1:28292',node_id=peer,raft_peer_id=peer,node_state='STATE_LEADER' if leader else 'STATE_FOLLOWER',term=2 if leader else 0,reload_complete_term=2 if leader else 0,term_before=2 if leader else 0,term_after=2 if leader else 0,raft_term_before=2,raft_term_after=2,raft_leader_before=leader,raft_leader_after=leader,raft_leader_id_before='127.0.0.1:28292',raft_leader_id_after='127.0.0.1:28292',committed=9,applied=9,all_sessions=1,records=[copy.deepcopy(record)],lockable=True)
  return dict(leader=0,nodes=nodes,confirmation=copy.deepcopy(nodes['0']),processes={key:dict(pid=node['pid'],peer_id=node['raft_peer_id']) for key,node in nodes.items()})
 def phase(self):return dict(xid='127.0.0.1:28291:7',branch_id=8,tcc_branch_id=9,resource_id='owned')
 def check(self,value=None,final=False):return verify_stage(value or self.good(),self.phase(),final)
 def bad(self,action):
  value=self.good();action(value)
  with self.assertRaises(RuntimeError):self.check(value)
 def test_control(self):self.check()
 def test_missing_reload(self):self.bad(lambda v:v['nodes']['0'].update(reload_complete_term=1))
 def test_actual_raft_term_changed(self):self.bad(lambda v:v['nodes']['0'].update(raft_term_after=3))
 def test_repeated_leader_changed(self):self.bad(lambda v:v['confirmation'].update(raft_peer_id='other'))
 def test_no_quorum(self):self.bad(lambda v:v.update(nodes={'0':v['nodes']['0']}))
 def test_two_leaders(self):self.bad(lambda v:v['nodes']['1'].update(leader=True))
 def test_replica_behind(self):self.bad(lambda v:v['nodes']['1'].update(applied=8))
 def test_replica_wrong_leader(self):self.bad(lambda v:v['nodes']['1'].update(leader_id='old'))
 def test_follower_term_changed(self):self.bad(lambda v:v['nodes']['1'].update(raft_term_after=99))
 def test_follower_peer_replaced(self):self.bad(lambda v:v['nodes']['1'].update(raft_peer_id='127.0.0.1:29999'))
 def test_follower_claims_actual_leadership(self):self.bad(lambda v:v['nodes']['1'].update(raft_leader_after=True))
 def test_recorded_process_replaced(self):self.bad(lambda v:v['processes']['1'].update(pid=999))
 def test_original_branch_replaced(self):self.bad(lambda v:v['nodes']['1']['records'][0]['branches'][0].update(branch_id=9))
 def test_original_resource_replaced(self):self.bad(lambda v:v['nodes']['1']['records'][0]['branches'][0].update(resource='other'))
 def test_context_changed_across_replicas(self):self.bad(lambda v:v['nodes']['1']['records'][0]['branches'][0].update(application_data='other'))
 def test_active_timeout(self):self.bad(lambda v:v['nodes']['0']['records'][0].update(active=True))
 def test_commit_lock_must_remain_available(self):self.bad(lambda v:v['nodes']['2'].update(lockable=False))
 def test_unexpected_record(self):self.bad(lambda v:v['nodes']['2'].update(all_sessions=2))
 def test_final_clean_control(self):
  value=self.good()
  for node in list(value['nodes'].values())+[value['confirmation']]:node.update(records=[],all_sessions=0,lockable=True)
  self.check(value,True)
 def partial(self):
  value=self.good()
  for node in list(value['nodes'].values())+[value['confirmation']]:node['records'][0]['branches']=[node['records'][0]['branches'][1]]
  return value
 def test_partial_commit_control(self):verify_stage(self.partial(),self.phase(),partial=True)
 def test_partial_commit_cannot_keep_completed_at(self):
  with self.assertRaises(RuntimeError):verify_stage(self.good(),self.phase(),partial=True)
 def test_partial_commit_cannot_lose_pending_tcc(self):
  value=self.partial();value['nodes']['1']['records'][0]['branches']=[]
  with self.assertRaises(RuntimeError):verify_stage(value,self.phase(),partial=True)
 def test_tcc_context_must_agree_across_replicas(self):
  value=self.partial();value['nodes']['1']['records'][0]['branches'][0]['application_data']='different'
  with self.assertRaises(RuntimeError):verify_stage(value,self.phase(),partial=True)
 def test_fake_election_same_peer(self):
  old=self.good();new=copy.deepcopy(old)
  with self.assertRaises(RuntimeError):verify_transition(old,new)
 def test_election_term_did_not_increase(self):
  old=self.good();new=copy.deepcopy(old);new['leader']=1;new['nodes']['1'].update(leader=True,term=2)
  with self.assertRaises(RuntimeError):verify_transition(old,new)

if __name__=='__main__':unittest.main()
