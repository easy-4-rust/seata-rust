"""Diagnostic topology only: preserve raw context/status differences for defect observations."""
import copy,importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('context_reference_parent_checks',ROOT/'tools/go-java-v1-raft-commit-timeout/cluster_checks.py');parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
completion=parent.completion
verify_transition=parent.verify_transition

def verify_stage(bundle,phase,final=False,partial=False):
 # 仅诊断路径排除待测字段，不改原JSON、真实分支或生产实现，也不认证恢复通过。
 observed=copy.deepcopy(bundle)
 for node in list(observed['nodes'].values())+[observed['confirmation']]:
  for record in node['records']:
   for branch in record['branches']:
    branch['application_data']=None;branch['status']=2 if branch['type']==0 else 1
 parent.verify_stage(observed,phase,final,partial)
 return bundle
