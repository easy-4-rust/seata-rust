"""Validate retained affected-row/Undo-image evidence and write scoped WP05 status."""

import hashlib
import json
from pathlib import Path


root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent


def read(name):
    return (evidence / name).read_text(encoding="utf-8")


for version in ("1.99", "1.85"):
    live = read(f"{version}-live-execution-verified.log")
    assert "test result: ok. 17 passed; 0 failed" in live
    assert "insert_uses_explicit_composite_keys_and_native_exact_values_without_committing ... ok" in live
    assert "no_match_and_noop_do_not_invent_rows_or_require_a_changed_row_count ... ok" in live
    assert "test result: ok. 1 passed; 0 failed" in read(f"{version}-no-change-commit.log")
    assert "test result: ok. 1 passed; 0 failed" in read(f"{version}-unit.log")
    clippy = read(f"{version}-clippy.log")
    assert "Finished `dev` profile" in clippy and "error:" not in clippy

assert "17 failed" in read("1.99-live-execution.log")
assert "undo_log' doesn't exist" in read("1.99-live-execution-final.log")
assert "original business SQL executes once" in read("1.99-live-execution-final2.log")
assert "16 passed; 1 failed" in read("1.99-live-execution-final2.log")
assert not read("fmt.log").strip()
assert "Owned relay stopped" in read("cleanup.log")
assert "Owned engine stopped" in read("cleanup.log")

paths = [
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/tests/live_execution.rs",
    "crates/seata_db_mysql/tests/live_commit_preconditions.rs",
]
paths.extend(
    str(path.relative_to(root))
    for path in evidence.iterdir()
    if path.is_file() and path.name != "verify.py"
)
sha = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in sorted(paths)}
result = {
    "status": "AFFECTED_ROWS_REQUIRE_UNDO_IMAGE_GUARD_VERIFIED",
    "scope": "pure invalid image shape and 17 live valid DML cases per Rust toolchain",
    "rust": ["1.99", "1.85"],
    "remaining": [
        "live fault injection of positive affected rows with empty Undo image",
        "changed business and Undo COMMIT",
        "old-attempt fence and original Java TC report",
        "WP05/WP06/full plan gates",
    ],
    "sha256": sha,
}
(root / "docs/phase-2/wp05-mysql-affected-image-guard.json").write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
print("Verified affected-row image guard and both live DML cohorts")
