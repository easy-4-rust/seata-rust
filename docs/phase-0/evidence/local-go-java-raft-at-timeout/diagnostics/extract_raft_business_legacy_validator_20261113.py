import hashlib,json
from pathlib import Path
root=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');run=root/'docs/phase-0/evidence/local-go-java-raft-at-timeout/run_20261113_051200_20f60e07';target=root/'tools/go-java-v1-raft-at-timeout/evidence.py'
row=next(row for row in json.loads((run/'source-input-audit.json').read_bytes()) if Path(row['path'])==target);raw=(run/row['snapshot']).read_bytes();assert hashlib.sha256(raw).hexdigest()==row['sha256'];out=Path('D:/codex-builds/legacy_raft_business_evidence_20261113.py')
if out.exists():assert out.read_bytes()==raw
else:out.write_bytes(raw)
print(row['sha256'])
