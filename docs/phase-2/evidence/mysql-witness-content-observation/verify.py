"""Validate retained witness/Undo content evidence and emit scoped WP05 status."""

import hashlib
import json
from pathlib import Path


root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent


def read(name):
    return (evidence / name).read_text(encoding="utf-8")


assert "test result: FAILED. 0 passed; 1 failed" in read("1.99-red.log")
assert "Err(DbError::UndoCodec(_))" in read("1.99-red.log")
assert "test result: ok. 1 passed; 0 failed" in read("1.99-green.log")
for version in ("1.99", "1.85"):
    run = read(f"{version}-regression.log")
    assert "test result: ok. 17 passed; 0 failed" in run
    assert "witness_cases::first_phase_witness_observation_preserves_legacy_and_survives_real_callback_cleanup ... ok" in run
    clippy = read(f"{version}-clippy.log")
    assert "Finished `dev` profile" in clippy and "error:" not in clippy

assert not read("fmt.log").strip()
assert "MySQL 8.4.13" in read("runtime.txt")
assert "Owned engine stopped" in read("cleanup.log")
assert "No relay listener remains" in read("cleanup.log")

paths = [
    "crates/seata_db_mysql/src/rm_branch_store.rs",
    "crates/seata_db_mysql/tests/support/first_phase_witness_cases.rs",
]
paths.extend(
    str(path.relative_to(root))
    for path in evidence.iterdir()
    if path.is_file() and path.name != "verify.py"
)
sha = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in sorted(paths)}
result = {
    "status": "PENDING_WITNESS_REQUIRES_DECODED_IDENTITY_MATCHED_UNDO_VERIFIED",
    "rust": ["1.99", "1.85"],
    "live_regression_cases_per_toolchain": 17,
    "remaining": [
        "current physical table and business row image revalidation",
        "changed first-phase COMMIT and response-loss classification",
        "old attempt fence and original Java TC complete lock coverage",
        "WP05/WP06/full plan gates",
    ],
    "sha256": sha,
}
(root / "docs/phase-2/wp05-mysql-witness-content-observation.json").write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
print("Verified Undo content RED/GREEN and dual live regressions")
