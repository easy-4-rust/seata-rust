"""核对选定 TLS 配置的真实握手、有效削弱控制、双工具链结果及清理。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
assert "no method named `with_tls_ca_pem`" in (evidence / "red.log").read_text()
assert "malformed-pem-fixture" in (evidence / "pem-red.log").read_text()
assert "test result: FAILED" in (evidence / "pem-red.log").read_text()
versions = {}
for version in ("1.99", "1.85"):
    options = (evidence / f"{version}-options.log").read_text()
    assert "test result: ok. 4 passed; 0 failed;" in options
    live = (evidence / f"{version}-live.log").read_text()
    assert "test result: ok. 1 passed; 0 failed;" in live
    assert "cipher=TLS_" in live and "required=1, alias_instance_matches=true" in live
    assert "rollback independently observed v=10" in live
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 634
    assert sum(int(ignored) for _, ignored in rows) == 187
    versions[version] = {"workspace_passed": 634, "ignored": 187, "options_passed": 4,
                         "live_tls_passed": 1, "strict_clippy": True}
for name, host in (("Required", "127.0.0.1"), ("HostRewrite", "localhost")):
    mutation = (evidence / f"mutation-{name}.log").read_text()
    assert "invalid TLS identity/trust was accepted for " + host in mutation
    assert "test result: FAILED. 0 passed; 1 failed;" in mutation
assert "Original production source restored exactly" in (evidence / "mutation-run.log").read_text()
assert "Hostname mismatch" in (evidence / "openssl-hostname-rejection.log").read_text()
assert "unable to get local issuer certificate" in (evidence / "openssl-ca-rejection.log").read_text()
assert "Ssl_version\tTLSv1.3" in (evidence / "independent-server.log").read_text()
assert "8.4.13\t1" in (evidence / "independent-server.log").read_text()
assert (evidence / "fmt.log").stat().st_size == 0
assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
paths = [p for p in evidence.rglob("*") if p.is_file()]
assert not any("PRIVATE KEY" in p.read_text(errors="ignore") for p in evidence.glob("*.pem"))
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/Cargo.toml", "crates/seata_db_mysql/src/mysql_options.rs",
    "crates/seata_db_mysql/tests/options_contract.rs", "crates/seata_db_mysql/tests/live_tls.rs",
    "docs/phase-2/mysql-foundation-contract.md",
)]
manifest = {
    "status": "SELECTED_MYSQL_TLS_CA_AND_SERVER_IDENTITY_PROFILE_VERIFIED",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "effective_mutation_controls": ["Required bypasses trust verification", "HostRewrite canonicalizes original hostname to matching IP"],
    "inconclusive_control": "VerifyCa retained hostname rejection in current fixed driver/rustls behavior; excluded from effective controls",
    "coverage": [
        "Bounded PEM CA list with nonempty certificate parsing, mutual exclusion with disabled TLS, and redacted Debug",
        "Trusted CA + exact IP identity connects to TLS-only MySQL; same options survive pool and auxiliary metadata connections",
        "Missing CA, wrong CA, wrong hostname and plaintext are rejected",
        "Independent TLS alias connectivity and server UUID prevent DNS/IPv6 errors being mistaken for hostname rejection",
        "Native MySQL observer confirms secure transport, TLS 1.3 and restored original business value",
    ],
    "trust_semantics": "Configured CA certificates supplement fixed SQLx WebPKI roots; VerifyIdentity remains mandatory",
    "remaining": ["Other certificate chains, expiry and rotation/revocation profiles", "Client certificate and production authentication profiles",
                  "Complete original WP05/WP06 and all remaining plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-mysql-tls-ca.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified selected TLS profile, two effective controls, dual Rust 634 tests and owned cleanup")
