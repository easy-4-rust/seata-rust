"""核验迟到约束的真实 RED/GREEN、双工具链回归、原 DDL 和自有环境清理。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "late trigger was allowed to participate in compensation: Ok(Consumed" in red
assert "test result: FAILED. 0 passed; 1 failed;" in red
assert (evidence / "red-database.log").read_text().strip() == "0\t0\t1"
red_observation = json.loads((evidence / "red-observation.json").read_text())
assert red_observation["actual_metadata_waits"] > 0
assert "Ok(Consumed" in red_observation["result"]

versions = {}
for version in ("1.99", "1.85"):
    for suite, expected in (("live_undo_rollback", 16), ("live_rm_recovery", 11)):
        log = (evidence / f"{version}-{suite}.log").read_text()
        assert f"test result: ok. {expected} passed; 0 failed;" in log
        assert "test result: FAILED" not in log
    for case, feature in (("trigger", "table trigger"), ("inbound", "inbound/outbound foreign key"),
                          ("outbound", "inbound/outbound foreign key")):
        observation = json.loads((evidence / version / f"late-undo-{case}.json").read_text())
        assert observation["actual_metadata_waits"] > 0
        assert "Err(BeforeCommit" in observation["result"]
        assert feature in observation["result"]
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 633
    assert sum(int(ignored) for _, ignored in rows) == 186
    versions[version] = {"workspace_passed": 633, "ignored": 186, "undo_live_passed": 16,
                         "managed_live_passed": 11, "strict_clippy": True}
assert (evidence / "fmt.log").stat().st_size == 0
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
server = (evidence / "server.log").read_text()
assert "8.4.13" in server and "REPEATABLE-READ" in server
java_schema = root.parent / "seata-java-phase09-oracle/script/client/at/db/mysql.sql"
assert java_schema.read_text() == (evidence / "undo-schema.sql").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/undo/abstract_undo_log_manager.rs",
    "crates/seata_db_mysql/src/data_source_proxy.rs", "crates/seata_db_mysql/src/data_source_manager.rs",
    "crates/seata_db_mysql/src/current_table_constraints.rs",
    "crates/seata_db_mysql/tests/live_undo_rollback.rs",
    "docs/phase-2/mysql-undo-rollback-contract.md",
)]
manifest = {
    "status": "UNDO_CURRENT_CONSTRAINTS_CHECKED_BEFORE_COMPENSATION",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": {"consumed_business_rows": 0, "remaining_undo_rows": 0, "unexpected_trigger_rows": 1},
    "coverage": [
        "Server metadata_locks proves business MDL wait after original Undo dictionary snapshot",
        "Locker publishes late trigger/outgoing FK/incoming cascade FK before releasing business table",
        "Independent Undo rejects before control COMMIT with original identity, original Undo bytes and business/child/effect state intact",
        "Both independent and managed entry points use original connection options and original operation deadlines",
        "All document tables verified and bound before first compensating business DML",
        "Managed 11-test regression includes atomic terminal, dirty retry, cancellation and exact recovery child process",
    ],
    "limits": ["One temporary out-of-pool connection per active table check", "Stable single-primary routing required",
               "Late-constraint race exercised through independent entry; managed entry wiring reviewed and full existing suite run"],
    "remaining": ["Other WP05/WP06 gates and remaining original plan", "Other MySQL versions and routing profiles"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-undo-current-constraints.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified RED, late constraints, dual Rust 16 Undo + 11 managed tests, 633 workspace tests and cleanup")
