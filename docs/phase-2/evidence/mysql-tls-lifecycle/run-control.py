import os
import subprocess
from pathlib import Path

root = Path.cwd()
source = root / "crates/seata_db_mysql/src/mysql_options.rs"
evidence = root / "docs/phase-2/evidence/mysql-tls-lifecycle"
original = source.read_bytes()
assert original.count(b"MySqlSslMode::VerifyIdentity") == 1
changed = original.replace(b"MySqlSslMode::VerifyIdentity", b"MySqlSslMode::Required")
source.write_bytes(changed)
try:
    with (evidence / "mutation-no-server-verification.log").open("wb") as log:
        result = subprocess.run([
            "cargo", "test", "--locked", "--offline", "-p", "seata-db-mysql", "--test", "live_tls_lifecycle",
            "--", "expired_and_future_server_certificates_reject_new_and_auxiliary_sessions", "--ignored", "--exact", "--nocapture",
        ], cwd=root, env=os.environ.copy(), stdout=log, stderr=subprocess.STDOUT, timeout=60)
    text = (evidence / "mutation-no-server-verification.log").read_text()
    assert result.returncode != 0
    assert "invalid server certificate lifetime was accepted: server-expired.pem" in text
    assert "test result: FAILED. 0 passed; 1 failed;" in text
    print("Server-verification bypass failed the intended expired-server assertion")
finally:
    if source.read_bytes() != changed:
        raise RuntimeError("concurrent source edit detected; refusing to overwrite")
    source.write_bytes(original)
assert source.read_bytes() == original
print("Original production source restored exactly")
