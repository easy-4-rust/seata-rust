"""Additional post-run certification of original phase-one context against every replica."""
def verify_context_binding(proof,registrations):
 phase=proof['business']['phase'];reports={r['branch_type']:r for r in proof['business']['original_branch_reports']}
 if set(reports)!={0,1} or set(registrations)!={0,1}:raise RuntimeError('Original accepted AT/TCC reports and registrations required')
 for kind in [0,1]:
  report=reports[kind];registered=registrations[kind];branch=phase['branch_id' if kind==0 else 'tcc_branch_id']
  if report['accepted'] is not True or (report['xid'],report['branch_id'])!=(phase['xid'],branch) or (registered['xid'],registered['branch_id'])!=(phase['xid'],branch):raise RuntimeError('Original phase-one register/report identity differs')
  if registered['application_data']!=report['application_data']:raise RuntimeError('This selected original Prepare does not exercise changed application context replication')
 observed=0
 for bundle in proof['cluster'].values():
  for node in bundle['nodes'].values():
   for record in node['records']:
    for branch in record['branches']:
     report=reports.get(branch['type'])
     if report is None or (branch['xid'],branch['branch_id'],branch['status'],branch['application_data'])!=(report['xid'],report['branch_id'],report['status'],report['application_data']):raise RuntimeError('Replicated original branch status/context does not bind accepted phase-one report')
     observed+=1
 if observed==0:raise RuntimeError('Actual replica branch observations missing')
 return dict(replica_branch_observations=observed,registration_report_context_equal=True,changed_prepare_context_replication_certified=False)
