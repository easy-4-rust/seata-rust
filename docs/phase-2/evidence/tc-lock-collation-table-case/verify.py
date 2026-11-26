import json,re,hashlib,sys
from pathlib import Path
sys.path.insert(0,str(Path("tools/formal-tc-lock-coverage").resolve()))
import collation_equivalence as c
e=Path("docs/phase-2/evidence/tc-lock-collation-table-case")
r={}
assert "match-contender: Err(Local" in (e/"java-tc-red-actual-1.99/client-stdout.log").read_text()
for v in ("1.99","1.85"):
 p=e/f"java-tc-verified-{v}"
 cases=[json.loads((p/f"{s}.json").read_bytes()) for s in c.CASES]
 result=c.verify(p,cases,True)
 assert result==json.loads((p/"result.json").read_bytes())
 assert cases[-1]["lock_key"]=="StOrAgE_MaTcH:café"
 assert result["rejected"]==1 and result["locally_rejected"]==2
 assert (p/"final-counts.tsv").read_text().split()==["0"]*5
 assert json.loads((p/"process-cleanup.json").read_bytes())["processes_stopped"]
 text=(e/f"{v}-workspace.log").read_text()
 rows=re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",text)
 assert sum(int(x) for x,y in rows)==633
 assert "Finished" in (e/f"{v}-clippy.log").read_text()
 assert sum(map(int,re.findall(r"test result: ok\. (\d+) passed; 0 failed",(e/f"{v}-doc.log").read_text())))==4
 r[v]={"workspace_passed":633,"doc_passed":4,"clippy":True,"java_tc":result}
paths=[*e.rglob("*")]
paths.extend(Path(p) for p in [
"crates/seata_db_mysql/src/java_tc_mysql_lock_collation.rs",
"crates/seata_db_mysql/tests/support/java_lock_collation_cases.rs",
"tools/formal-tc-lock-coverage/collation_equivalence.py",
"docs/phase-2/tc-lock-coverage-contract.md"])
manifest={"status":"PHYSICAL_COLUMN_COLLATION_AND_CASE_INSENSITIVE_TABLE_RESOLUTION_VERIFIED",
"rust":r,"mysql":"8.4.13","lower_case_table_names":1,
"scope":"Resolved physical column type in the original transaction, including empty tables and mixed table spelling",
"open":["Concurrent primary key membership/type DDL","Full WP05/WP06 and public COMMIT"],
"sha256":{p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file()}}
Path("docs/phase-2/wp05-tc-lock-collation-table-case.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print("Both Rust versions and original TC frames verified")
