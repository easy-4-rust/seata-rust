import importlib.util
from pathlib import Path
s=importlib.util.spec_from_file_location("actual_guard",Path('E:\\workspaces\\workspace-github-easy-4-rust\\seata-rust-phase09-validation\\tools\\go-java-v1-raft-context-update\\context_evidence.py'))
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
verify_updated_context=m.verify_updated_context
