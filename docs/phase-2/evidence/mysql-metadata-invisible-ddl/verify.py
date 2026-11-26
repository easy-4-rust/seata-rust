import json,re,hashlib
from pathlib import Path
e=Path("docs/phase-2/evidence/mysql-metadata-invisible-ddl")
assert "independent_extra_ddl" in (e/"red.log").read_text()
assert "FAILED" in (e/"red.log").read_text()
assert "Owned engine stopped" in (e/"cleanup.log").read_text()
rust={}
for v in ("1.99","1.85"):
 assert "test result: ok. 1 passed; 0 failed;" in (e/f"{v}-live.log").read_text()
 assert "test result: ok. 7 passed; 0 failed;" in (e/f"{v}-final-ddl.log").read_text()
 assert "Finished" in (e/f"{v}-final-clippy.log").read_text()
 text=(e/f"{v}-final-workspace.log").read_text()
 rows=re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",text)
 assert sum(int(a) for a,b in rows)==633
 assert sum(int(b) for a,b in rows)==182
 rust[v]={"ordinary_passed":633,"ignored":182,"live":1,"strict_clippy":True}
paths=[p for p in e.rglob("*") if p.is_file()]
paths+=list(map(Path,["Cargo.lock","crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs","crates/seata_db_mysql/tests/live_metadata.rs","docs/phase-2/mysql-foundation-contract.md"]))
manifest={"status":"CURRENT_INVISIBLE_GENERATED_AND_AUTO_INCREMENT_ATTRIBUTES_VERIFIED","mysql":"8.4.13","rust":rust,
"scope":"Physical SHOW CREATE column count catches added invisible column; generated and auto_increment attributes are refused under original MDL. Four actual changes are exercised.",
"remaining":["trigger/foreign-key independent changes and general SHOW CREATE parser profile","Complete WP05/WP06"],
"sha256":{p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}}
Path("docs/phase-2/wp05-metadata-invisible-ddl.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print("Dual Rust live and workspace results verified")
