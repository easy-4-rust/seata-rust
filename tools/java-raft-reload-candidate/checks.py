"""Require a successful reload in the current real leader term, plus exact original records."""
import copy,importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('original_raft_evidence_checks',ROOT/'tools/java-tc-integration/raft_recovery/checks.py')
original=importlib.util.module_from_spec(spec);spec.loader.exec_module(original)
verify_transition=original.verify_transition
reload_exceptions=original.reload_exceptions

def verify_completion(state):
 term=state.get('term',0)
 if state.get('leader') is not True or term<=0 or state.get('reload_complete_term')!=term:
  raise RuntimeError('Current leader term lacks actual successful asynchronous session reload')
 if any(state.get(key)!=term for key in ['term_before','term_after','raft_term_before','raft_term_after']):
  raise RuntimeError('Leader term changed during observation or differs from actual JRaft term')
 if any(state.get(key) is not True for key in ['raft_leader_before','raft_leader_after']):
  raise RuntimeError('JRaft did not confirm actual leadership throughout observation')
 if not state.get('raft_peer_id') or any(state.get(key)!=state['raft_peer_id'] for key in ['raft_leader_id_before','raft_leader_id_after']):
  raise RuntimeError('Actual JRaft leader identity differs from this node')
 if state.get('node_state')!='STATE_LEADER' or state.get('leader_id')!=state['raft_peer_id']:
  raise RuntimeError('JRaft state or sampled leader identity contradicts current leadership')
 return True

def verify_records(state,status,host,after_election=False):
 value=copy.deepcopy(state)
 if after_election and status==16:
  if value['records'][0].get('lockable') is not True:raise RuntimeError('Commit retry timeout residual lock was not released')
  # Original checker retains all identity/status/branch requirements; only the candidate lock policy differs.
  value['records'][0]['lockable']=False
 return original.verify_records(value,status,host,after_election)
