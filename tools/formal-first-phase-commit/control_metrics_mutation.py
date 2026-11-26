"""Execute a real MySQL test with missing result settlement, then restore original bytes."""
from pathlib import Path
import subprocess
source=Path("crates/seata_db_mysql/src/data_source_manager.rs")
original=source.read_bytes()
needle=b"guard.finish(&result);"
assert original.count(needle)==1
command=["cargo","+1.99.0","test","-p","seata-db-mysql","--test","live_recovery_metrics","--all-features","--offline","--locked","shared_control_counts_follow_durable_results_and_duplicate_terminal_reads","--","--ignored","--exact"]
out=Path("docs/phase-2/evidence/mysql-control-metrics")
try:
    source.write_bytes(original.replace(needle,b"drop(guard);"))
    with (out/"settlement-mutation-red.log").open("wb") as f:
        result=subprocess.run(command,stdout=f,stderr=subprocess.STDOUT)
    assert result.returncode!=0,"missing control settlement was not rejected"
    log=(out/"settlement-mutation-red.log").read_text(encoding="utf-8")
    assert "left: (0, 0, 0, 0)" in log and "right: (1, 1, 0, 0)" in log,"test did not fail at exact target"
finally:
    source.write_bytes(original)
with (out/"settlement-restored-green.log").open("wb") as f:
    result=subprocess.run(command,stdout=f,stderr=subprocess.STDOUT)
assert result.returncode==0
print("PASS missing settlement mutation and restored real MySQL test")
