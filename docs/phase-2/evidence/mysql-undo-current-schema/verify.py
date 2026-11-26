"""核对当前 Undo 既有规范的真实拒绝、中央入口回归与自有环境清理。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "late Undo definition was accepted or misclassified: CREATE TRIGGER" in red
assert "Ok(())" in red and "test result: FAILED. 0 passed; 1 failed;" in red
assert (evidence / "red-database.log").read_text().strip() == "0\t0\t1"
versions = {}
for version in ("1.99", "1.85"):
    for suite, count in (("live_undo_store", 12), ("live_undo_rollback", 16),
                         ("live_undo_fence", 9), ("live_commit_direction", 1)):
        live = (evidence / f"{version}-{suite}.log").read_text()
        assert f"test result: ok. {count} passed; 0 failed;" in live
        assert "test result: FAILED" not in live
        if suite == "live_undo_store":
            assert live.count("late Undo schema rejected without writes or side effects:") == 7
            assert live.count("old snapshot engine rejected and fresh InnoDB flush rolled back:") == 3
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 635
    assert sum(int(ignored) for _, ignored in rows) == 194
    versions[version] = {"workspace_passed": 635, "ignored": 194, "store_live_passed": 12,
                         "rollback_live_passed": 16, "fence_live_passed": 9,
                         "controlled_commit_cleanup_live_passed": 1, "strict_clippy": True}
assert (evidence / "fmt.log").stat().st_size == 0
assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert "8.4.13\tREPEATABLE-READ\t67108864" in (evidence / "independent-server.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/current_table_constraints.rs",
    "crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs",
    "crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs", "crates/seata_db_mysql/src/data_source_proxy.rs",
    "crates/seata_db_mysql/src/data_source_manager.rs", "crates/seata_db_mysql/src/undo/abstract_undo_log_manager.rs",
    "crates/seata_db_mysql/tests/live_undo_store.rs", "docs/phase-2/mysql-undo-store-contract.md",
)]
manifest = {
    "status": "CURRENT_UNDO_POLICY_REVALIDATED_WITH_AUTHENTICATED_FRESH_METADATA_TRANSACTION",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": {"business_rows_after_rollback": 0, "undo_rows": 0, "nontransactional_trigger_side_effect_rows": 1},
    "coverage": [
        "Late trigger, dropped unique identity index, changed text width, invisible extra column, unsigned status, outgoing and incoming FK are rejected",
        "Original transaction holds MDL; same-identity/TLS fresh connection verifies server UUID and schema before independent metadata transaction",
        "Existing policy is re-run centrally for flush, fence preparation and authorized cleanup; original error priority retained",
        "Fresh metadata transaction rolls back and closes without user-data writes or branch publication",
        "Poison and original rollback preserve business, Undo and side-effect state",
    ],
    "limits": ["One temporary out-of-pool connection per active validation", "Queued DDL can cause safe deadline failure",
               "Stable single-primary routing remains required", "Current existing Undo policy only; arbitrary schema/deployment safety is not certified"],
    "remaining": ["Recovery-table metadata snapshots", "Original TC safety, public first-phase COMMIT and remaining WP05/WP06/full plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-undo-current-schema.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified trigger atomicity RED, seven current-policy cases, 38 live tests per Rust, 635 workspace tests and cleanup")
