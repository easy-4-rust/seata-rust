"""Read-only cross-check of pinned source, portable bytes, original JUnit and Rust assertion ledgers."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
from rm_metadata_source import CLASSPATH_INPUTS_SHA256,PIN,safe_member
PREFIX="org.apache.seata.rm.datasource.sql.struct."
COUNTS={
"ColumnMetaTest#testColumnMeta":21,"IndexMetaTest#testIndexMeta":10,"IndexTypeTest#testIndexType":2,
"TableMetaTest#testTableMeta":4,"TableMetaTest#testGetColumnMeta":2,"TableMetaTest#testGetAutoIncreaseColumn":2,
"TableMetaTest#testGetPrimaryKeyMap":1,"TableMetaTest#testGetPrimaryKeyOnlyName":3,"TableMetaTest#testGetPkName":1,
"TableMetaTest#testContainsPK":5,
}
def digest(data):return hashlib.sha256(data).hexdigest()
def read_json(path):return json.loads(path.read_bytes())
def run(oracle,rust):
    root=Path(__file__).resolve().parents[2]
    source=read_json(oracle/"source-inputs.json")
    assert source["pin"]==PIN and len(source["files"])==8
    for row in source["files"]:
        data=(oracle/"sources"/row["path"]).read_bytes()
        assert digest(data)==row["sha256"]
        assert hashlib.sha1(b"blob "+str(len(data)).encode()+b"\0"+data).hexdigest()==row["git_blob"]
        actual=subprocess.check_output(["git","-C",str(root.parent/"seata-java-phase09-oracle"),"cat-file","blob",PIN+":"+row["path"]])
        assert data==actual
    java=read_json(oracle/"java-assertions.json")
    assert {k.removeprefix(PREFIX):len(v) for k,v in java.items()}==COUNTS
    for label in ("original-junit","observed-junit"):
        events=[json.loads(line) for line in (oracle/(label+"-events.jsonl")).read_text().splitlines()]
        assert events[-1]==dict(kind="summary",passed=10,failed=0,skipped=0)
        assert {e["class"]+"#"+e["method"] for e in events[:-1]}==set(java)
        assert all(e["status"]=="SUCCESSFUL" for e in events[:-1])
    authority=(oracle/"baseline-classpath-inputs.json").read_bytes()
    assert digest(authority)==CLASSPATH_INPUTS_SHA256
    expected={e["path"]:e["sha256"] for e in json.loads(authority)}
    portable=read_json(oracle/"portable-manifest.json")
    assert len({e["path"] for e in portable["members"]})==len(portable["members"])==13
    for e in portable["members"]:
        path=safe_member(oracle,e["snapshot"],"portable-inputs")
        assert e["sha256"]==expected[e["path"]]==digest(path.read_bytes())
    compiled=read_json(oracle/"compiled-bindings.json")
    for e in compiled:
        assert digest((oracle/e["snapshot"]).read_bytes())==e["sha256"]
    for role in ("main","original","observed"):
        assert sum(e["role"]==role for e in compiled)==4
    origins=read_json(oracle/"class-origins.json")
    assert len(origins)==16
    bootstrap=read_json(oracle/"bootstrap.json")
    assert bootstrap["mode"]=="portable"
    for version in ("1.99","1.85"):
        entries={}
        for line in (rust/(version+"-observations.tsv")).read_text().splitlines():
            key,value=line.split("\t")
            assert key not in entries
            entries[key]=bytes.fromhex(value).decode("utf-8").split("\x1f")
        assert entries==java
        controls=read_json(rust/(version+"-controls.json"))
        assert len(controls)==8 and all(c["rejected"] is True for c in controls)
    mutations=read_json(rust/"mutations.json")
    assert len(mutations)==3 and all(m["rejected"] is True for m in mutations)
    for name in ("table_meta.rs","index_type.rs"):
        path=root/"crates/seata_sqlparser/src/struct"/name
        assert path.read_bytes()==subprocess.check_output(["git","-C",str(root),"show","HEAD:"+path.relative_to(root).as_posix()])
    bindings=read_json(rust/"source-case-bindings.json")
    assert len(bindings)==10 and all(b["status"]=="MATCH" for b in bindings)
    assert sum(b["assertions"] for b in bindings)==51
    return dict(kind="read-only-source-and-ledger-cross-check",original_java_cases=10,original_assertions=51,
        literal_pin_verified=True,loaded_fresh_source_classes_verified=True,portable_authority_verified=True,
        rust_versions=["1.99","1.85"],mutation_controls=3,evidence_controls_per_version=8,
        production_sources_unchanged_from_HEAD=True,full_baseline_restored=False,global_totals_recomputed=False,
        independent_human_or_agent_review=False,full_WP05_complete=False)
if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("oracle",type=Path);parser.add_argument("rust",type=Path);parser.add_argument("output",type=Path)
    a=parser.parse_args();report=run(a.oracle,a.rust)
    with a.output.open("x",encoding="utf-8") as f:json.dump(report,f,indent=2)
    print("PASS readonly source / portable / original JUnit / Rust ledger audit")
