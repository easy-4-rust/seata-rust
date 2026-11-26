"""核对恢复表伪提交见证 RED、当前定义拒绝、双工具链回归及运行环境清理。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "late recovery definition obtained commit arbitration" in red
assert "FAILED. 0 passed; 1 failed" in red
assert (evidence / "red-database.log").read_text().strip() == "0\t0\t1\t0\t0\nMyISAM"
versions = {}
for version in ("1.99", "1.85"):
    counts = {"live_commit_preconditions": 13, "live_first_phase_preparation": 11,
              "live_rm_recovery": 11, "live_commit_direction": 1}
    for suite, count in counts.items():
        log = (evidence / f"{version}-{suite}.log").read_text()
        assert f"test result: ok. {count} passed; 0 failed;" in log
        assert "test result: FAILED" not in log
        if suite == "live_commit_preconditions":
            assert log.count("late recovery schema rejected; receipt retained; witness and side effects zero:") == 7
            assert "valid late witness migration rejected; receipt retained; witness zero" in log
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(p) for p, _ in rows) == 635
    assert sum(int(i) for _, i in rows) == 196
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    versions[version] = {"workspace_passed": 635, "ignored": 196,
                         "live_passed": counts, "strict_clippy": True}
assert (evidence / "fmt.log").stat().st_size == 0
cleanup = (evidence / "cleanup.log").read_text()
assert "Owned relay closed" in cleanup and "Owned engine stopped" in cleanup
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert "8.4.13\tREPEATABLE-READ\t67108864" in (evidence / "independent-server.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/rm_branch_store.rs",
    "crates/seata_db_mysql/src/current_table_constraints.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/src/data_source_manager.rs",
    "crates/seata_db_mysql/tests/live_commit_preconditions.rs",
    "docs/phase-2/mysql-commit-preconditions-contract.md",
)]
manifest = {
    "status": "CURRENT_RECOVERY_SCHEMA_REVALIDATED_BEFORE_ARBITRATION_AND_CONTROL",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": {"business_after_rollback": 0, "undo_after_rollback": 0,
            "false_first_phase_witness": 1, "phase": 0, "terminal": 0, "engine": "MyISAM"},
    "coverage": [
        "Seven late unsafe recovery definitions rejected before writes; original receipt retained, business/Undo/witness/trigger side effect zero",
        "Valid migration 003 to 004 after old snapshot rejected instead of changing prepared capability",
        "Original MDL plus fresh same-authentication/TLS metadata transaction, UUID/schema equality, no user-data writes",
        "Central commit precondition, recovery control and independent first-phase observation entrances guarded",
        "Existing cancellation, deadline, callback ordering, recovery atomicity and child-process regressions retained",
    ],
    "limits": ["One extra out-of-pool connection per active recovery validation",
               "Queued DDL can cause safe original-deadline failure", "Stable single-primary routing required",
               "Controlled protocol peer tests do not replace real Java TC validation"],
    "remaining": ["Original WP05 and WP06/full plan gates", "Public first-phase COMMIT and original TC lock safety"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-recovery-current-schema.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified false witness RED, eight schema cases, 36 live tests per Rust, 635 workspace tests and cleanup")
