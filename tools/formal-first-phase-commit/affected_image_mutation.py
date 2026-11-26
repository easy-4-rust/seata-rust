"""Remove the original production guard for an actual driver/TC RED, then restore exact bytes."""
from pathlib import Path
import subprocess,json,hashlib
root=Path(__file__).resolve().parents[2]
source=root/"crates/seata_db_mysql/src/connection_proxy.rs"
original=source.read_bytes()
needle=b"                Self::verify_affected_rows_have_image(count, &undo)?;"
assert original.count(needle)==1
mutated=original.replace(needle,b"                // controlled mutation: omit affected image guard")
out=root/"docs/phase-2/evidence/mysql-affected-image-native"
runs=root/"docs/phase-2/evidence/java-tc-changed-commit-report"
command=["pwsh","-NoProfile","-File","tools/formal-first-phase-commit/run_desktop.ps1","-RustVersion","1.99","-Scenario","affected_image_guard"]
before=set(runs.glob("run-affected_image_guard-1.99-*"))
try:
    source.write_bytes(mutated)
    with (out/"guard-mutation-red.log").open("wb") as f:
        result=subprocess.run(command,cwd=root,stdout=f,stderr=subprocess.STDOUT)
    assert result.returncode!=0,"missing production guard was not rejected"
    new,=set(runs.glob("run-affected_image_guard-1.99-*"))-before
    text=(new/"client-stdout.log").read_text(encoding="utf-8")
    assert "native affected count without any image must be rejected: 1" in text,text[-2000:]
    cleanup=json.loads((new/"process-cleanup.json").read_bytes())
    assert cleanup["stopped"] is True
    with (out/"guard-mutation-proof.json").open("x",encoding="utf-8") as f:
        json.dump(dict(run=new.name,expected_failure=True,original_sha256=hashlib.sha256(original).hexdigest(),mutated_sha256=hashlib.sha256(mutated).hexdigest(),source_restore_required=True),f,indent=2)
finally:
    source.write_bytes(original)
assert source.read_bytes()==original
with (out/"guard-restored-green.log").open("wb") as f:
    result=subprocess.run(command,cwd=root,stdout=f,stderr=subprocess.STDOUT)
assert result.returncode==0,"restored source must pass full original TC/MySQL gate"
print("PASS original driver guard mutation rejected; restored full fixture passed")
