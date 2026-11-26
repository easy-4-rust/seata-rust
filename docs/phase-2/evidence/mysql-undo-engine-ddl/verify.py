"""核对 Undo 当前引擎的真实 RED/GREEN、原始建表字节与双工具链回归。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "nontransactional Undo was accepted after old dictionary snapshot: Ok(())" in red
assert "test result: FAILED. 0 passed; 1 failed;" in red
assert (evidence / "red-database.log").read_text().strip() == "0\t1\nMyISAM"
for case in range(3):
    snapshot = json.loads((evidence / f"ddl-current-definition-{case}.json").read_text())
    ddl = bytes(snapshot["bytes"]).decode("utf-8")
    engines = [line for line in ddl.splitlines() if line.startswith(") ENGINE=")]
    assert len(engines) == 1 and engines[0].startswith(") ENGINE=MyISAM ")
    if case:
        assert "fake\\n) ENGINE=InnoDB " in ddl
versions = {}
for version in ("1.99", "1.85"):
    for suite, count in (("live_undo_store", 11), ("live_undo_rollback", 16)):
        live = (evidence / f"{version}-{suite}.log").read_text()
        assert f"test result: ok. {count} passed; 0 failed;" in live
        assert "test result: FAILED" not in live
        if suite == "live_undo_store":
            assert live.count("old snapshot engine rejected and fresh InnoDB flush rolled back:") == 3
            assert "InnoDB '" in live
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 635
    assert sum(int(ignored) for _, ignored in rows) == 193
    versions[version] = {"workspace_passed": 635, "ignored": 193, "store_live_passed": 11,
                         "rollback_live_passed": 16, "strict_clippy": True}
    metadata = (evidence / f"{version}-business-engine.log").read_text()
    assert "test result: ok. 1 passed; 0 failed;" in metadata
assert (evidence / "fmt.log").stat().st_size == 0
assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs",
    "crates/seata_db_mysql/src/mysql/mysql_undo_log_manager.rs",
    "crates/seata_db_mysql/tests/live_undo_store.rs", "docs/phase-2/mysql-undo-store-contract.md",
)]
manifest = {
    "status": "UNDO_PHYSICAL_ENGINE_VERIFIED_UNDER_ORIGINAL_MDL",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": {"business_rows_after_rollback": 0, "nontransactional_orphan_undo_rows": 1, "engine": "MyISAM"},
    "coverage": [
        "Business statement creates dictionary snapshot before later Undo engine DDL",
        "Current SHOW CREATE engine is checked under original Undo MDL before writes",
        "Plain MyISAM and column/table comments with exact fake InnoDB token are rejected before flush",
        "Real current DDL bytes show escaped comment newlines and a single actual MyISAM engine line",
        "Rejected scope remains poisoned; original business and Undo counts are zero after rollback",
        "Restored InnoDB accepts fresh flush on same data source and rolls back both tables",
    ],
    "remaining": ["Other protected Undo/recovery schema snapshot dimensions, including columns, indexes and constraints",
                  "Original TC safety, public first-phase COMMIT and remaining WP05/WP06/full plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-undo-engine-ddl.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified atomicity RED, current Undo engine, comment bytes, dual Rust regressions and cleanup")
