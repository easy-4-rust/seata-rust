import json,re,hashlib
from pathlib import Path
e=Path("docs/phase-2/evidence/mysql-metadata-constraint-ddl")
assert "later_trigger_and_foreign_keys" in (e/"red.log").read_text()
assert "FAILED" in (e/"red.log").read_text()
assert "Owned engine stopped" in (e/"cleanup.log").read_text()
rust={}
for v in ("1.99","1.85"):
 assert "test result: ok. 1 passed; 0 failed;" in (e/f"{v}-live.log").read_text()
 assert "test result: ok. 7 passed; 0 failed;" in (e/f"{v}-ddl-regression.log").read_text()
 assert "Finished" in (e/f"{v}-clippy.log").read_text()
 text=(e/f"{v}-workspace.log").read_text()
 rows=re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",text)
 assert sum(int(a) for a,b in rows)==633
 assert sum(int(b) for a,b in rows)==183
 rust[v]={"ordinary_passed":633,"ignored":183,"live":1,"strict_clippy":True}
paths=[p for p in e.rglob("*") if p.is_file()]
paths+=list(map(Path,["Cargo.lock","crates/seata_db_mysql/src/current_table_constraints.rs","crates/seata_db_mysql/src/connection_proxy.rs","crates/seata_db_mysql/src/data_source_proxy.rs","crates/seata_db_mysql/src/lib.rs","crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs","crates/seata_db_mysql/tests/live_metadata.rs","docs/phase-2/mysql-foundation-contract.md"]))
manifest={"status":"CURRENT_TRIGGER_AND_FOREIGN_KEY_CHECKED_WITH_FRESH_SESSION","mysql":"8.4.13","rust":rust,
"scope":"Original business MDL plus fresh authenticated session and server UUID/schema identity check reject current triggers and incoming/outgoing foreign keys. Adds one temporary connection per active check; independent Undo compensation is outside this change.",
"remaining":["Post-check FK DDL locking schedule and independent Undo compensation constraint reads","Complete WP05/WP06"],
"sha256":{p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}}
Path("docs/phase-2/wp05-metadata-constraint-ddl.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print("Dual Rust live and workspace results verified")

