import os
import subprocess
from pathlib import Path

root = Path.cwd()
source = root / "crates/seata_db_mysql/src/mysql_options.rs"
evidence = root / "docs/phase-2/evidence/mysql-mtls"
original = source.read_bytes()
needle = b"Some((certificate, private_key)) => options\n                .ssl_client_cert_from_pem(certificate)\n                .ssl_client_key_from_pem(private_key),"
assert original.count(needle) == 1
changed = original.replace(needle, b"Some((_certificate, _private_key)) => options,")
source.write_bytes(changed)
try:
    with (evidence / "mutation-drop-client-identity.log").open("wb") as log:
        result = subprocess.run([
            "cargo", "test", "--locked", "--offline", "-p", "seata-db-mysql", "--test", "live_tls",
            "--", "x509_account_requires_client_identity_for_pool_and_auxiliary_sessions", "--ignored", "--exact", "--nocapture",
        ], cwd=root, env=os.environ.copy(), stdout=log, stderr=subprocess.STDOUT, timeout=60)
    text = (evidence / "mutation-drop-client-identity.log").read_text()
    assert result.returncode != 0
    assert "valid X509 identity must authenticate data source" in text
    assert "test result: FAILED. 0 passed; 1 failed;" in text
    print("Dropping the client identity failed the intended positive authentication assertion")
finally:
    if source.read_bytes() != changed:
        raise RuntimeError("concurrent edit detected; refusing to overwrite")
    source.write_bytes(original)
assert source.read_bytes() == original
print("Original production source restored exactly")
