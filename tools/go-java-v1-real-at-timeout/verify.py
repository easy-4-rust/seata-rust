"""Strict normalized evidence guards; runtime parser binds these claims to SQL and wire bytes."""
import hashlib

def validate(value):
 try:
  phase=value['phase'];xid=phase['xid'];branch=phase['branch_id'];raw=bytes.fromhex(phase['undo_hex'])
  if phase['balance']!=900 or phase['undo_status']!=0 or branch<=0 or not xid or len(raw)!=phase['undo_size'] or hashlib.sha256(raw).hexdigest()!=phase['undo_sha256']:raise RuntimeError('Original real Undo proof differs')
  for name in ['timeout','restart','lost','second_restart','final']:
   row=value['snapshots'][name];final=name=='final';before=name in ['timeout','restart']
   if row['xid']!=xid or row['branch_id']!=branch or row['balance']!=(900 if before else 1000):raise RuntimeError('Business/branch identity differs at '+name)
   if (row['global_status'],row['branches'],row['locks'])!=((None,0,0) if final else (17,1,1)):raise RuntimeError('TC retained timeout/branch/lock state differs at '+name)
   if row['undo_status']!=(0 if before else 1 if final else None):raise RuntimeError('Persistent Undo/fence lifecycle differs at '+name)
   if before and bytes.fromhex(row['undo_hex'])!=raw:raise RuntimeError('Persisted original Undo bytes changed')
   if final and row['undo_hex']!='7B7D' and row['undo_hex']!='7b7d':raise RuntimeError('Real Go duplicate fence bytes differ')
  if len(value['processes'])!=3 or len(set(value['processes']))!=3 or any(p<=0 for p in value['processes']):raise RuntimeError('Three separate SDK processes are required')
  if len(value['database_starts'])!=3 or len(set(value['database_starts']))!=3:raise RuntimeError('Two actual database restarts are required')
  if value['request_withheld']<1 or value['successful_response_lost']!=1 or value['registered_stages']!=[True]*3 or not value['original_branch'] or not value['callback_correlations']:raise RuntimeError('Actual registered callback/drop evidence differs')
  if value['tm_begins']!=1 or value['tm_rollbacks']!=1:raise RuntimeError('Original TM target mutation was replayed')
  if value['operator_results']!=[500,200] or value['proxy_errors']:raise RuntimeError('Actual operator or transport evidence differs')
 except (KeyError,TypeError,ValueError) as error:raise RuntimeError('Incomplete normalized evidence') from error
 return value
