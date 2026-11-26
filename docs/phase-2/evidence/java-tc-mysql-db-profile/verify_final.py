"""核对组合配置的固定原 Java TC 现场结果及其清理范围。"""
import hashlib
import json
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
failed = (evidence / "1.99-original-java-tc-run.log").read_text()
assert "AssertionError" in failed
old_client = (evidence / "java-tc-verified-1.99/client-stdout.log").read_text()
assert 'business text primary key collation differs from Java TC MySQL lock storage' in old_client
versions = {}
for version in ("1.99", "1.85"):
    folder = evidence / f"java-tc-final-{version}"
    result = json.loads((folder / "result.json").read_text())
    assert result["branch_registrations"] == 3
    assert result["successful"] == 2 and result["rejected"] == 1
    assert result["lock_queries"] == 2 and result["global_rollbacks"] == 6
    assert result["locally_rejected"] == 3
    assert result["original_rollback_callbacks"] >= 3
    assert result["original_rollback_retries"] >= 1
    assert "test result: ok. 1 passed; 0 failed" in (folder / "client-stdout.log").read_text()
    assert json.loads((folder / "process-cleanup.json").read_text())["processes_stopped"]
    counts = (folder / "final-counts.tsv").read_text().strip().split("\t")
    assert len(counts) == 8 and counts[2:] == ["43317", "0", "0", "0", "0", "0"]
    inputs = json.loads((folder / "inputs.json").read_text())
    assert inputs["java_pin"] and inputs["java_jar"] and inputs["connector_j"]
    assert inputs["artifact_sha256"]
    assert (folder / "RM-frames.jsonl").stat().st_size > 0
    assert (folder / "TM-frames.jsonl").stat().st_size > 0
    assert "Finished" in (evidence / f"{version}-full-clippy.log").read_text()
    versions[version] = result
assert (evidence / "full-fmt.log").stat().st_size == 0
assert "Owned engine stopped" in (evidence / "final-cleanup.log").read_text()
assert "Both owned relays closed" in (evidence / "final-cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "final-image.log").read_text()
assert "8.4.13\tREPEATABLE-READ\t67108864" in (evidence / "full-independent-server.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "crates/seata_db_mysql/src/mysql_options.rs",
    "crates/seata_db_mysql/tests/support/java_lock_guard_cases.rs",
    "crates/seata_db_mysql/tests/support/java_lock_identity_cases.rs",
    "tools/formal-tc-lock-coverage/live_guard.py",
    "docs/phase-2/tc-lock-coverage-contract.md",
)]
manifest = {
    "status": "FIXED_JAVA_TC_COMBINED_MYSQL_DB_PROFILE_SELECTED_SCENARIOS_VERIFIED",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "red": "Historical as_cs identity fixture was safely rejected by combined collation guard; original failure retained",
    "scope": "Six identity/alias scenarios, original Java TC raw RM/TM frames, independent DB checkpoints, original callbacks and full cleanup",
    "remaining": ["Full exclusive lock coverage including external clients", "Public phase-one business COMMIT", "WP05/WP06/full plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-java-tc-mysql-db-profile.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified fixed Java TC combined profile on both Rust toolchains, original wire and DB state, and cleanup")
