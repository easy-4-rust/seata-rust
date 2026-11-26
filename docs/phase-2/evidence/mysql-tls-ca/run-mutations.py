import os
import subprocess
from pathlib import Path

root = Path.cwd()
source = root / "crates/seata_db_mysql/src/mysql_options.rs"
evidence = root / "docs/phase-2/evidence/mysql-tls-ca"
original = source.read_bytes()
needle = b"MySqlSslMode::VerifyIdentity"
assert original.count(needle) == 1
for mode, changed, expected in (
    (b"Required", original.replace(needle, b"MySqlSslMode::Required"), "127.0.0.1"),
    (b"HostRewrite", original.replace(b".host(&self.host)", b'.host(if self.host == "localhost" { "127.0.0.1" } else { &self.host })'), "localhost"),
):
    assert source.read_bytes() == original
    source.write_bytes(changed)
    try:
        with (evidence / ("mutation-" + mode.decode() + ".log")).open("wb") as log:
            result = subprocess.run([
                "cargo", "test", "--locked", "--offline", "-p", "seata-db-mysql",
                "--test", "live_tls", "--", "--ignored", "--nocapture",
            ], cwd=root, env=os.environ.copy(), stdout=log, stderr=subprocess.STDOUT, timeout=60)
        text = (evidence / ("mutation-" + mode.decode() + ".log")).read_text()
        assert result.returncode != 0
        assert "invalid TLS identity/trust was accepted for " + expected in text
        print(mode.decode() + " was rejected by the intended live assertion")
    finally:
        if source.read_bytes() != changed:
            raise RuntimeError("concurrent source edit detected; refusing to overwrite")
        source.write_bytes(original)
assert source.read_bytes() == original
print("Original production source restored exactly")
