import json,re,hashlib
from pathlib import Path
e=Path("docs/phase-2/evidence/mysql-metadata-cross-table-ddl")
assert 'left: ["id"]' in (e/"red.log").read_text(encoding="utf-8")
rust={}
for v in ("1.99","1.85"):
 assert "test result: ok. 1 passed; 0 failed;" in (e/f"{v}-live.log").read_text(encoding="utf-8")
 assert "Finished" in (e/f"{v}-clippy.log").read_text(encoding="utf-8")
 log=(e/f"{v}-workspace.log").read_text(encoding="utf-8")
 rows=re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",log)
 assert sum(int(a) for a,b in rows)==633
 assert sum(int(b) for a,b in rows)==177
 rust[v]={"ordinary_passed":633,"ignored":177,"live_regression":1,"strict_clippy":True}
assert "Owned engine stopped" in (e/"cleanup.log").read_text(encoding="utf-8-sig")
files=[p for p in e.rglob("*") if p.is_file()]
files += list(map(Path,["crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs","crates/seata_db_mysql/tests/live_metadata.rs","docs/phase-2/mysql-foundation-contract.md","Cargo.lock"]))
m={"status":"CURRENT_PHYSICAL_PRIMARY_DEFINITION_CHECKED_AFTER_CROSS_TABLE_DDL",
"rust":rust,"mysql":"8.4.13","scope":"Detect stale primary membership/order under RR and poison original transaction; new transaction reads altered unsigned domain and new primary key",
"pending":["Independent type/charset/engine/trigger/foreign-key changes","Complete WP05 and WP06"],
"sha256":{p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in files}}
Path("docs/phase-2/wp05-metadata-cross-table-ddl.json").write_text(json.dumps(m,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print("Dual Rust live and workspace regression verified")
