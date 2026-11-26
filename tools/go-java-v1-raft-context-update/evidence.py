"""Require full real recovery and genuine Prepare context replication together."""
import importlib.util
from pathlib import Path
HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
cluster=module('context_full_cluster_evidence',HERE/'cluster_evidence.py');context=module('context_updated_evidence',HERE/'context_evidence.py')
parse_metadata=cluster.parse_metadata
def derive(directory,archive_metadata=False):
 proof=cluster.derive(directory,archive_metadata);proof['updated_context']=context.derive(directory,proof);return proof
