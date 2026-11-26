"""Original normalized business assertions remain unchanged for the real RAFT increment."""
import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('raft_original_business_guards',ROOT/'tools/go-java-v1-real-at-timeout/verify.py');parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
validate=parent.validate
