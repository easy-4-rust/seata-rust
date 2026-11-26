import hashlib
import json
import re
import sys
from copy import deepcopy
from pathlib import Path

evidence = Path("docs/phase-2/evidence/tc-lock-collation-guard")
sys.path.insert(0, str(Path("tools/formal-tc-lock-coverage").resolve()))
import collation_equivalence as proof

def load(path):
    return json.loads(path.read_bytes())

def reject(check):
    try:
        check()
    except ValueError:
        return
    raise AssertionError("changed observation passed verification")

files = [evidence / name for name in (
    "api-red.log", "baseline.log", "behavior-red.log", "green.log", "runtime.log", "relay-green.log",
    "cleanup.log", "fmt.log", "owned-wsl-java-runner.py", "recheck.py",
    "java-tc-red-1.99/client-stdout.log", "java-tc-red-1.99/RM-frames.jsonl",
    "java-tc-red-1.99/process-cleanup.json",
)]
assert "error[E0599]" in (evidence/"api-red.log").read_text(encoding="utf-8")
before = (evidence/"java-tc-red-1.99/client-stdout.log").read_text(encoding="utf-8")
assert "unexpected registration result for pad-owner: Ok(" in before
assert "isolated Docker engine stopped" in (evidence/"cleanup.log").read_text(encoding="utf-8-sig")
rust = {}
for version in ("1.99","1.85"):
    totals = {}
    for mode in ("legacy","verified"):
        guarded = mode == "verified"
        run = evidence/f"java-tc-{mode}-{version}"
        cases = [load(run/f"{stage}.json") for stage in proof.CASES]
        profile = (run/"business-and-tc-collations.tsv").read_bytes()
        proof.verify_profile(profile)
        result = proof.verify(run, cases, guarded)
        assert result == load(run/"result.json")
        assert result["branch_registrations"] == (2 if guarded else 4)
        assert result["locally_rejected"] == (2 if guarded else 0)
        assert (run/"final-counts.tsv").read_text().split() == ["0"]*5
        assert load(run/"process-cleanup.json")["processes_stopped"]
        rows = [line.split("\t") for line in (run/"pad-contender-database.tsv").read_text(
            encoding="utf-8").splitlines()]
        if guarded:
            assert not rows
        else:
            assert [row[4] for row in rows] == ["key", "key "]
            assert [row[0] for row in rows] == [row["xid"] for row in cases[:2]]
        reject(lambda: proof.verify(run, cases, not guarded))
        changed = deepcopy(cases)
        changed[1]["locally_rejected"] = not guarded
        reject(lambda: proof.verify(run, changed, guarded))
        reject(lambda: proof.verify_profile(profile.replace(b"PAD SPACE", b"NO PAD")))
        totals[mode] = result
        files.extend(path for path in run.iterdir() if path.is_file())
    for role in ("workspace","doc","clippy"):
        files.append(evidence/f"{version}-{role}.log")
    workspace = (evidence/f"{version}-workspace.log").read_text(encoding="utf-8")
    rows = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;",workspace,re.M)
    passed, ignored = (sum(map(int, column)) for column in zip(*rows))
    assert (passed,ignored,len(rows)) == (633,176,121)
    docs = (evidence/f"{version}-doc.log").read_text(encoding="utf-8")
    doc_count = sum(map(int,re.findall(r"^test result: ok\. (\d+) passed; 0 failed;",docs,re.M)))
    assert doc_count == 4
    assert "Finished" in (evidence/f"{version}-clippy.log").read_text(encoding="utf-8")
    rust[version] = dict(workspace_passed=passed,workspace_ignored=ignored,docs_passed=doc_count,
                         strict_clippy=True,java_tc=totals)
    print(version, "wire, stored keys, termination, workspace and negative controls verified")
spec = "docs/phase-2/tc-lock-coverage-contract.md"
files.extend(map(Path,[
    spec,"Cargo.lock",
    "crates/seata_db_mysql/src/java_tc_mysql_lock_collation.rs",
    "crates/seata_db_mysql/src/mysql_options.rs",
    "crates/seata_db_mysql/src/data_source_manager.rs",
    "crates/seata_db_mysql/src/data_source_proxy.rs",
    "crates/seata_db_mysql/src/data_source_proxy_rollback_sqlx_tests.rs",
    "crates/seata_db_mysql/src/lib.rs",
    "crates/seata_db_mysql/src/mysql_transaction_preparer.rs",
    "crates/seata_db_mysql/src/connection_proxy.rs",
    "crates/seata_db_mysql/tests/live_commit_direction_java.rs",
    "crates/seata_db_mysql/tests/support/java_lock_collation_cases.rs",
    "crates/seata_db_mysql/tests/support/java_lock_guard_cases.rs",
    "tools/formal-tc-lock-coverage/live_guard.py",
    "tools/formal-tc-lock-coverage/collation_equivalence.py",
]))
sha256 = {path.as_posix(): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(set(files))}
manifest = dict(
    status="OPTIONAL_JAVA_TC_MYSQL_COLLATION_GUARD_VERIFIED",
    spec=spec,
    evidence=evidence.as_posix(),
    prior="docs/phase-2/wp05-tc-lock-db-length-guard.json",
    mysql="8.4.13",
    java_pin="cba73e180fd8896a99df6415b859d275122feaa7",
    local_scope="Explicit manager option, expected TC lock collation utf8mb4_0900_ai_ci, exact business PK metadata under the original transaction MDL",
    fixture_scope="Actual local DML with rollback and explicit administrative deletion to emulate release of the business-row lock; original TC DB lock rows and wire requests",
    open_items=["Actual TC deployment profile and other clients", "Complete lock equivalence", "Public AT business COMMIT", "WP05/WP06"],
    rust=rust,
    sha256=sha256,
)
dest=Path("docs/phase-2/wp05-tc-lock-collation-guard.json")
dest.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest()==digest for path,digest in load(dest)["sha256"].items())
print(len(sha256),"hashes verified")
