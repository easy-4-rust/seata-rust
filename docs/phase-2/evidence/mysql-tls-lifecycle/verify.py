"""核对真实 MySQL 与双 TLS 前端的证书生命周期证据，不将两种边界混称。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
versions = {}
for version in ("1.99", "1.85"):
    live = (evidence / f"{version}-live.log").read_text()
    assert "test result: ok. 4 passed; 0 failed;" in live
    for marker in (
        "X509 rejected client lifetime: client-expired.pem",
        "X509 rejected client lifetime: client-future.pem",
        "server lifetime rejected new and auxiliary sessions: server-expired.pem",
        "server lifetime rejected new and auxiliary sessions: server-future.pem",
        "same CA renewed leaf accepted by new pool and auxiliary connections",
        "same overlap trust bundle accepted fresh sessions under both CAs",
        "MySQL rejected invalid reload with 3888 and kept previous context: server-expired.pem",
        "MySQL rejected invalid reload with 3888 and kept previous context: server-future.pem",
    ):
        assert marker in live
    assert "independent lifecycle business value=10" in live
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 635
    assert sum(int(ignored) for _, ignored in rows) == 192
    versions[version] = {"workspace_passed": 635, "ignored": 192, "lifecycle_live_passed": 4,
                         "test_threads": 4, "fixture_serialized_inside_process": True, "strict_clippy": True}
for identity in ("server", "client"):
    assert "certificate has expired" in (evidence / f"openssl-{identity}-expired.log").read_text()
    assert "certificate is not yet valid" in (evidence / f"openssl-{identity}-future.log").read_text()
mutation = (evidence / "mutation-no-server-verification.log").read_text()
assert "invalid server certificate lifetime was accepted: server-expired.pem" in mutation
assert "test result: FAILED. 0 passed; 1 failed;" in mutation
assert "Original production source restored exactly" in (evidence / "mutation-run.log").read_text()
previous = json.loads((root / "docs/phase-2/wp05-mysql-mtls.json").read_text())
for name in ("Cargo.lock", "crates/seata_db_mysql/src/mysql_options.rs",
             "crates/seata_db_mysql/tests/options_contract.rs", "crates/seata_db_mysql/tests/live_tls.rs"):
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == previous["sha256"][name]
observer = (evidence / "independent-server.log").read_text()
assert "8.4.13" in observer and "Ssl_version\tTLSv1.3" in observer
assert observer.strip().endswith("10")
assert (evidence / "fmt.log").stat().st_size == 0
cleanup = (evidence / "cleanup.log").read_text()
assert "Owned TLS bridge closed" in cleanup and "Owned relay closed" in cleanup and "Owned engine stopped" in cleanup
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert not any("PRIVATE KEY" in p.read_text() for p in evidence.glob("*.pem"))
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/mysql_options.rs",
    "crates/seata_db_mysql/tests/live_tls_lifecycle.rs", "docs/phase-2/mysql-foundation-contract.md",
)]
manifest = {
    "status": "SELECTED_TLS_LIFETIME_AND_ONLINE_ROTATION_BOUNDARIES_VERIFIED",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "production_change": "None; selected crypto options, prior TLS tests and lockfile SHA match preceding mTLS milestone",
    "coverage": [
        "Real MySQL X509 rejects expired/future client identities with valid identity control",
        "Real MySQL rejects invalid server reload with 3888 and preserves previous TLS context for fresh connections",
        "Real MySQL same-CA leaf/key reload succeeds for fresh pool and auxiliary handshakes",
        "CA reload rejects old trust for fresh pool and old transaction auxiliary handshakes; poison and rollback preserve original business value",
        "Same overlap CA bundle creates fresh metadata sessions under both CAs; replacement CA configuration also succeeds",
        "Controlled two-TLS frontend presents expired/future server certificates to SQLx; valid-before/after real MySQL authentication and business controls pass",
    ],
    "bridge_boundary": "Windows43319 terminates frontend TLS, forwards real MySQL protocol over backend identity-verified TLS via Windows43318 relay to WSL43317. MySQL itself does not present invalid certificates in this case.",
    "remaining": ["Further certificate/revocation/deployment profiles as required", "Original WP05/WP06 and remaining plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-mysql-tls-lifecycle.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified four lifecycle cases per toolchain, 635 workspace tests, exact prior crypto sources and owned cleanup")
