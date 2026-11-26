"""Check the retained bounded unknown COMMIT evidence without starting services."""

import hashlib
import json
from pathlib import Path


root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent


def read(name):
    return (evidence / name).read_text(encoding="utf-8")


for version in ("1.99", "1.85"):
    live = read(f"{version}-live.log")
    assert "test result: ok. 1 passed; 0 failed" in live
    assert "server COMMIT OK withheld; general_log delta=1; local result=bounded unknown" in live
    assert "original MySQL COMMIT returned OK; reply held" in read(
        f"{version}-server-ok.txt"
    )
    clippy = read(f"{version}-clippy.log")
    assert "Finished `dev` profile" in clippy and "error:" not in clippy

assert not read("fmt.log").strip()
cleanup = read("cleanup.log")
assert "Owned proxy and relay stopped" in cleanup and "Owned engine stopped" in cleanup
assert "8.4.13" in read("mysql-version.tsv")
assert "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec" in read(
    "container-identity.txt"
)

source_paths = [
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/src/mysql_commit_checked_transaction.rs",
    "crates/seata_db_mysql/tests/live_no_change_commit_loss.rs",
    "tools/formal-first-phase-commit/mysql_commit_response_proxy.py",
]
files = sorted(source_paths + [str(path.relative_to(root)) for path in evidence.iterdir() if path.is_file() and path.name != "verify.py"])
sha = {
    name: hashlib.sha256((root / name).read_bytes()).hexdigest()
    for name in files
}
out = {
    "status": "NO_CHANGE_COMMIT_SERVER_OK_LOSS_BOUNDED_UNKNOWN_VERIFIED",
    "scope": "one no-change original MySQL COMMIT per Rust toolchain; server OK held by owned proxy",
    "rust": ["1.99", "1.85"],
    "remaining": [
        "changed business and Undo COMMIT",
        "post-send recovery and old-attempt fence",
        "original Java TC branch report and full exclusive lock coverage",
        "WP05/WP06/full plan gates",
    ],
    "sha256": sha,
}
(root / "docs/phase-2/wp05-mysql-no-change-commit-loss.json").write_text(
    json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
print("Verified bounded unknown no-change COMMIT against retained server and Rust evidence")
