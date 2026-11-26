"""Validate persisted mixed AT/TCC outcomes independently of HTTP success."""
import hashlib
def validate(value):
 try:
  phase=value['phase'];raw=bytes.fromhex(phase['undo_hex']);xid=phase['xid'];at=phase['branch_id'];tcc=phase['tcc_branch_id']
  if not xid or min(at,tcc)<=0 or at==tcc or phase['balance']!=900 or phase['undo_status']!=0 or len(raw)!=phase['undo_size'] or hashlib.sha256(raw).hexdigest()!=phase['undo_sha256']:raise RuntimeError('Original mixed phase-one proof differs')
  for name in ['timeout','restart','lost','second_restart','final']:
   row=value['snapshots'][name];before=name in ['timeout','restart'];final=name=='final'
   if row['xid']!=xid or row['balance']!=900 or row['locks']!=0:raise RuntimeError('Commit business/lock state differs')
   if (row['status'],row['at_branch'],row['tcc_branch'])!=((None,None,None) if final else (16,at,tcc) if before else (16,None,tcc)):raise RuntimeError('Persisted original commit branches differ')
   if before:
    if row['undo_status']!=0 or bytes.fromhex(row['undo_hex'])!=raw:raise RuntimeError('Original Undo changed before manual commit')
   elif row['undo_status'] is not None or row['undo_hex'] is not None:raise RuntimeError('Real AT commit did not clean Undo')
   if (row['tcc_xid'],row['tcc_effect_branch'])!=(xid,tcc):raise RuntimeError('Persistent TCC business identity differs')
   if (row['reserved'],row['confirmed'],row['effect_count'],row['tcc_phase'])!=((10,0,0,'prepared') if before else (0,10,1,'committed')):raise RuntimeError('TCC business was not committed exactly once')
  for key in ['sdk_pids','tc_pids','database_starts']:
   if len(value[key])!=3 or len(set(value[key]))!=3:raise RuntimeError('Three independent runtime identities required')
  if value['withheld']<2 or value['lost']!=1 or value['stages_registered']!=[True]*3 or not value['callback_correlations'] or not value['original_branches']:raise RuntimeError('Actual mixed registration/callback/drop evidence differs')
  if (value['tm_begins'],value['tm_commits'],value['tm_rollbacks'])!=(1,1,0):raise RuntimeError('TM target mutation replayed')
  if [tuple(row) for row in value['operator_results']]!=[(200,'500'),(200,'200')] or value['proxy_errors']:raise RuntimeError('Actual operator/transport result differs')
 except (KeyError,TypeError,ValueError) as error:raise RuntimeError('Incomplete mixed commit evidence') from error
 return value
