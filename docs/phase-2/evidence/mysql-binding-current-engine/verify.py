"""核对资源绑定的真实失锁 RED、登记前拒绝与双工具链回归。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "late unsafe binding registered and its claimed shared lock allowed administrative update" in red
assert "FAILED. 0 passed; 1 failed" in red
assert (evidence / "red-database.log").read_text().strip() == "0\t0\t1\t00\nMyISAM"
observation = json.loads((evidence / "red-observation.json").read_text())
assert observation["registered"] and observation["binding_changed_while_prepared"] == 1
versions = {}
for version in ("1.99", "1.85"):
    for suite, count in (("live_first_phase_preparation", 12), ("live_commit_preconditions", 13)):
        log = (evidence / f"{version}-{suite}.log").read_text()
        assert f"test result: ok. {count} passed; 0 failed;" in log
        assert "test result: FAILED" not in log
        if suite == "live_first_phase_preparation":
            assert log.count("late unsafe binding rejected before registration:") == 2
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(p) for p, _ in rows) == 635
    assert sum(int(i) for _, i in rows) == 197
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    observed = json.loads((evidence / f"{version}-late-binding-engine.json").read_text())
    assert len(observed) == 2
    for entry in observed:
        assert entry["registered"] is False
        assert entry["business"] == entry["undo"] == entry["recovery"] == 0
    versions[version] = {"workspace_passed": 635, "ignored": 197,
                         "preparation_live_passed": 12, "preconditions_live_passed": 13,
                         "strict_clippy": True}
assert (evidence / "fmt.log").stat().st_size == 0
cleanup = (evidence / "cleanup.log").read_text()
assert "Owned relay closed" in cleanup and "Owned engine stopped" in cleanup
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert "8.4.13\tREPEATABLE-READ\t67108864" in (evidence / "independent-server.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/mysql_resource_binding.rs",
    "crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/src/mysql_transaction_preparer.rs",
    "crates/seata_db_mysql/tests/live_first_phase_preparation.rs",
    "crates/seata_db_mysql/tests/live_commit_preconditions.rs",
    "crates/seata_db_mysql/tests/support/first_phase_fixture.rs",
    "crates/seata_db_mysql/tests/support/registration_guard_cases.rs",
    "crates/seata_db_mysql/tests/support/owned_mysql.rs",
    "crates/seata_db_mysql/tests/support/undo_store_fixture.rs",
    "crates/seata_db_mysql/migrations/005_seata_rs_at_resource_binding.sql",
    "docs/phase-2/tc-lock-coverage-contract.md",
)]
manifest = {
    "status": "CURRENT_RESOURCE_BINDING_ENGINE_REQUIRED_BEFORE_REGISTRATION",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": {"registered_before_unsafe_engine_rejection": True, "binding_changed_while_prepared": 1,
            "business_after_rollback": 0, "undo_after_rollback": 0, "recovery_registration": 1},
    "coverage": ["Late MyISAM and view replacement rejected with exact current-engine error before BranchRegister",
                 "No receipt or unknown registration; business, Undo and recovery state zero",
                 "Original MDL and shared row lock retained for supported InnoDB; same connection/deadline",
                 "Existing correct binding, held-lock and full preparation/precondition regressions retained"],
    "limits": ["Existing opt-in binding policy; full binding schema certification not claimed",
               "Stable single-primary routing and unified original JDBC resource ID remain deployment requirements"],
    "remaining": ["Original TC lock safety and public first-phase COMMIT", "WP05/WP06 and all remaining full-plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-binding-current-engine.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified real binding lock-loss RED, two rejections, 25 live tests per Rust, workspace checks and cleanup")
