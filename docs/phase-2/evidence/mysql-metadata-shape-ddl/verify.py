import json,re,hashlib
from pathlib import Path
e=Path("docs/phase-2/evidence/mysql-metadata-shape-ddl")
assert "independent_column_shape_ddl" in (e/"red.log").read_text()
assert "FAILED" in (e/"red.log").read_text()
assert "Owned engine stopped" in (e/"cleanup.log").read_text()
rust={}
for v in ("1.99","1.85"):
 assert "test result: ok. 1 passed; 0 failed;" in (e/f"{v}-live.log").read_text()
 assert "test result: ok. 6 passed; 0 failed;" in (e/f"{v}-ddl-regression.log").read_text()
 assert "Finished" in (e/f"{v}-clippy.log").read_text()
 text=(e/f"{v}-workspace.log").read_text()
 rows=re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",text)
 assert sum(int(a) for a,b in rows)==633
 assert sum(int(b) for a,b in rows)==181
 rust[v]={"ordinary_passed":633,"ignored":181,"live":1,"strict_clippy":True}
paths=[p for p in e.rglob("*") if p.is_file()]
paths+=list(map(Path,["Cargo.lock","crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs","crates/seata_db_mysql/tests/live_metadata.rs","docs/phase-2/mysql-foundation-contract.md"]))
manifest={"status":"STALE_COLUMN_COUNT_ORDER_NULLABILITY_REJECTED","mysql":"8.4.13","rust":rust,
"scope":"Fresh SQLx description checks physical result count, order and nullability under original MDL. Three independent changes are refused, poisoned and rolled back.",
"remaining":["charset/nullability/order/count/engine/trigger independent changes","Complete WP05/WP06"],
"sha256":{p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}}
Path("docs/phase-2/wp05-metadata-shape-ddl.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print("Dual Rust live and workspace results verified")
