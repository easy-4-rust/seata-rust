"""核对实际双工具链日志及本轮证据摘要，不替代真实数据库执行。"""
import hashlib
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent
rust = {}
for version in ("1.99", "1.85"):
    live = (evidence / f"{version}-live.log").read_text(encoding="utf-8")
    assert "checked_metadata_blocks_later_trigger_and_foreign_key_ddl_until_rollback ... ok" in live
    assert "test result: ok. 1 passed; 0 failed;" in live
    regression = (evidence / f"{version}-ddl-regression.log").read_text(encoding="utf-8")
    assert "test result: ok. 8 passed; 0 failed;" in regression
    clippy = (evidence / f"{version}-clippy.log").read_text(encoding="utf-8")
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text(encoding="utf-8")
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 633
    assert sum(int(ignored) for _, ignored in rows) == 184
    rust[version] = {"ordinary_passed": 633, "ignored": 184, "live_schedule": 1,
                     "ddl_regression": 8, "strict_clippy": True}

assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert "8.4.13" in (evidence / "runtime.log").read_text(errors="replace")
assert (evidence / "fmt.log").stat().st_size == 0
paths = [p for p in evidence.iterdir() if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/tests/live_metadata.rs",
    "crates/seata_db_mysql/src/current_table_constraints.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "docs/phase-2/mysql-foundation-contract.md",
)]
manifest = {
    "status": "POSTCHECK_CONSTRAINT_DDL_BLOCKED_UNTIL_ORIGINAL_ROLLBACK",
    "mysql": "8.4.13", "image": image, "rust": rust,
    "coverage": [
        "CREATE TRIGGER after successful public metadata check returns MySQL 1205",
        "ADD outgoing foreign key after successful metadata check returns MySQL 1205",
        "ADD incoming ON DELETE CASCADE foreign key after successful metadata check returns MySQL 1205",
        "Original transaction still executes UPDATE and DELETE and rolls back all business changes",
        "Independent query observes original parent value, retained child, and empty effect table",
        "Same DDL succeeds after original rollback; fresh transaction rejects and becomes poisoned",
    ],
    "method": "Regression verification of existing production locks; no production behavior changed in this milestone.",
    "remaining": ["Independent Undo compensation current constraint checks and tests", "Complete WP05/WP06 and remaining original plan"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-metadata-postcheck-ddl.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified two toolchains, three post-check constraints, eight DDL tests, workspace and cleanup")
