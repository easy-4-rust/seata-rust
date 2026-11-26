"""核对选定 X509 账号的真实双向认证、双工具链回归及自有环境清理。"""
import hashlib
import json
import re
from pathlib import Path

evidence = Path(__file__).resolve().parent
root = evidence.parents[3]
red = (evidence / "red.log").read_text()
assert "no method named `with_tls_client_identity_pem`" in red
versions = {}
for version in ("1.99", "1.85"):
    assert "test result: ok. 5 passed; 0 failed;" in (evidence / f"{version}-options.log").read_text()
    live = (evidence / f"{version}-live.log").read_text()
    assert "test result: ok. 2 passed; 0 failed;" in live
    assert "X509 authenticated pool and auxiliary sessions; independently restored v=10" in live
    assert "cipher=TLS_" in live
    clippy = (evidence / f"{version}-clippy.log").read_text()
    assert "Finished" in clippy and "error:" not in clippy
    workspace = (evidence / f"{version}-workspace.log").read_text()
    assert "test result: FAILED" not in workspace and "error:" not in workspace
    rows = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored", workspace)
    assert sum(int(passed) for passed, _ in rows) == 635
    assert sum(int(ignored) for _, ignored in rows) == 188
    versions[version] = {"workspace_passed": 635, "ignored": 188, "options_passed": 5,
                         "live_tls_passed": 2, "strict_clippy": True}
mutation = (evidence / "mutation-drop-client-identity.log").read_text()
assert "valid X509 identity must authenticate data source" in mutation
assert "test result: FAILED. 0 passed; 1 failed;" in mutation
assert "Original production source restored exactly" in (evidence / "mutation-run.log").read_text()
runtime = (evidence / "runtime.log").read_text()
assert "mtls_fixture\tX509\tcaching_sha2_password" in runtime
observer = (evidence / "independent-client.log").read_text()
assert "mtls_fixture@%\t1" in observer
assert "Ssl_version\tTLSv1.3" in observer and observer.strip().endswith("10")
assert "ERROR 1045 (28000)" in (evidence / "independent-missing-client.log").read_text()
assert (evidence / "fmt.log").stat().st_size == 0
assert "Owned relay closed" in (evidence / "cleanup.log").read_text()
assert "Owned engine stopped" in (evidence / "cleanup.log").read_text()
image = "sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec"
assert image in (evidence / "image.log").read_text()
assert not any("PRIVATE KEY" in p.read_text() for p in evidence.glob("*.pem"))
paths = [p for p in evidence.rglob("*") if p.is_file()]
paths += [root / name for name in (
    "Cargo.lock", "crates/seata_db_mysql/src/mysql_options.rs",
    "crates/seata_db_mysql/tests/options_contract.rs", "crates/seata_db_mysql/tests/live_tls.rs",
    "docs/phase-2/mysql-foundation-contract.md",
)]
manifest = {
    "status": "SELECTED_MYSQL_X509_CLIENT_AUTH_PROFILE_VERIFIED",
    "mysql": "8.4.13", "image": image, "rust": versions,
    "coverage": [
        "Bounded certificate/key PEM pair; disabled TLS mutually excluded; server VerifyIdentity retained",
        "X509 account rejects missing client certificate, other CA client, mismatched private key and wrong password",
        "Correct identity is retained by pool and auxiliary metadata sessions; business rollback independently observed",
        "Native MySQL client verifies server identity, authenticates X509 account and records TLS 1.3; missing identity rejected with 1045",
        "Dropping production identity settings fails intended positive authentication assertion; exact source restored",
    ],
    "limits": ["Selected RSA/PEM profile only", "First recognized private-key block follows fixed driver semantics",
               "Raw key and driver copies have no zeroization guarantee", "Encrypted private-key password not supported by this entry"],
    "remaining": ["Certificate lifecycle, rotation/revocation and further algorithm/chain profiles",
                  "Production least-privilege authentication and remaining original WP05/WP06/V1 plan gates"],
    "sha256": {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in sorted(paths)},
}
(root / "docs/phase-2/wp05-mysql-mtls.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("Verified selected X509 profile, effective control, dual Rust 635 tests and owned cleanup")
