"""Route untouched original mixed Commit frames to the owned current RAFT leader."""
import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
parent=module('raft_commit_original_proxy',ROOT/'tools/go-java-v1-real-commit-timeout/proxy.py')
route=module('raft_commit_original_route',ROOT/'tools/go-java-v1-raft-at-timeout/proxy.py')
class RaftCommitProxy(parent.CommitProxy):
 def __init__(self,runtime,evidence,mode,port):
  if port not in [28291,28293,28295]:raise ValueError('Owned current native leader port required')
  super().__init__(runtime,evidence,mode);self.backend_port=port
 accept=route.RaftTimeoutProxy.accept
