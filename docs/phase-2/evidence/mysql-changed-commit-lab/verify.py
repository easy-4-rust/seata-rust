"""Validate the gated changed-COMMIT experiment without closing production WP05."""

import hashlib
import json
from pathlib import Path


root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent


def read(name):
    return (evidence / name).read_text(encoding="utf-8")


assert "error[E0599]: no method named `commit_changed`" in read("changed-commit-red.log")
assert "test result: ok. 3 passed; 0 failed" in read("changed-commit-three.log")
for version in ("1.99", "1.85"):
    full = read(f"changed-commit-{version}-regression.log")
    assert "test result: ok. 20 passed; 0 failed" in full
    for case in (
        "experimental_changed_commit_driver_loss_retains_unknown_receipt",
        "experimental_changed_commit_persists_business_undo_and_witness_together",
        "experimental_changed_commit_rejects_legacy_witness_before_sending",
    ):
        assert f"test {case} ... ok" in full
    assert "test result: ok. 1 passed; 0 failed" in read(
        f"changed-commit-{version}-observer-loss.log"
    )
    for profile in ("default", "lab"):
        clippy = read(f"changed-commit-final-{version}-{profile}-clippy.log")
        assert "Finished `dev` profile" in clippy and "error:" not in clippy

assert not read("fmt.log").strip()
assert "MySQL 8.4.13" in read("runtime.txt")
assert "Owned engine stopped" in read("cleanup.log")
assert "No relay listener remains" in read("cleanup.log")

paths = [
    "crates/seata_db_mysql/Cargo.toml",
    "crates/seata_db_mysql/src/lib.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/src/mysql_commit_checked_transaction.rs",
    "crates/seata_db_mysql/src/mysql_changed_commit_error.rs",
    "crates/seata_db_mysql/src/mysql_prepared_transaction.rs",
    "crates/seata_db_mysql/src/data_source_manager.rs",
    "crates/seata_db_mysql/tests/live_commit_preconditions.rs",
]
paths.extend(
    str(path.relative_to(root))
    for path in evidence.iterdir()
    if path.is_file() and path.name != "verify.py"
)
sha = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in sorted(paths)}
result = {
    "status": "EXPERIMENTAL_CHANGED_COMMIT_LOCAL_ATOMICITY_VERIFIED_ONLY",
    "rust": ["1.99", "1.85"],
    "feature": "experimental_changed_commit",
    "default_enabled": False,
    "live_full_cases_per_toolchain": 20,
    "additional_observer_loss_case_per_toolchain": 1,
    "remaining": [
        "original Java TC complete exclusive lock coverage",
        "all old-attempt fencing and durable attempt ownership",
        "server COMMIT OK response loss and process cancellation matrix",
        "PhaseOne_Done/Failed report and loss classification",
        "business row image revalidation and production deployment gates",
        "WP05/WP06/full plan gates",
    ],
    "sha256": sha,
}
(root / "docs/phase-2/wp05-mysql-changed-commit-lab.json").write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
print("Verified gated changed-COMMIT local experiment and retained production gates")
