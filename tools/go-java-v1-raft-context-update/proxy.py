"""Inherited actual mixed-mode recorder and owned current-leader route; no context rewriting."""
import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('updated_context_original_proxy',ROOT/'tools/go-java-v1-raft-commit-timeout/proxy.py');parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
RaftCommitProxy=parent.RaftCommitProxy
