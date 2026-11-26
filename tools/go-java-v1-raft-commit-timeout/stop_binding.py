"""Additional final certification gate, kept separate from the frozen runtime input epoch."""
def verify_stop_binding(receipt,before,after,process):
 if process['pid']!=receipt['pid'] or process['index']!=receipt['node'] or receipt['pid']==after['nodes'][str(after['leader'])]['pid']:raise RuntimeError('Stopped process identity differs')
 index=before['leader'];leader=before['nodes'][str(index)];sample=receipt['before'];identity=before['processes'][str(index)]
 if receipt['node']!=index or receipt['pid']!=leader['pid'] or receipt['process_file']!=identity['process_file'] or receipt['returncode'] is None:raise RuntimeError('Stopped process is not the previous confirmed leader')
 for key in ['pid','term','raft_peer_id','leader_id','reload_complete_term']:
  if sample[key]!=leader[key]:raise RuntimeError('Pre-stop sample does not belong to the previous current leader term')
 if sample['pid']!=receipt['pid'] or sample['leader'] is not True or leader['raft_peer_id']!=identity['peer_id']:raise RuntimeError('Pre-stop sample/process/peer identity differs')
 return True
