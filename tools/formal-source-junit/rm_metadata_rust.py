"""Verify case identities and assertion sequences, then exercise scoped production mutations."""
import argparse,copy,hashlib,json,os,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
PREFIX="org.apache.seata.rm.datasource.sql.struct."
BINDINGS={
"ColumnMetaTest#testColumnMeta":"original_rm_test_column_meta",
"IndexMetaTest#testIndexMeta":"original_rm_test_index_meta",
"IndexTypeTest#testIndexType":"original_rm_test_index_type",
"TableMetaTest#testTableMeta":"original_rm_test_table_meta",
"TableMetaTest#testGetColumnMeta":"original_rm_test_get_column_meta",
"TableMetaTest#testGetAutoIncreaseColumn":"original_rm_test_get_auto_increase_column",
"TableMetaTest#testGetPrimaryKeyMap":"original_rm_test_get_primary_key_map",
"TableMetaTest#testGetPrimaryKeyOnlyName":"original_rm_test_get_primary_key_only_name",
"TableMetaTest#testGetPkName":"original_rm_test_get_pk_name",
"TableMetaTest#testContainsPK":"original_rm_test_contains_pk",
}
def test_targets():
    return [arg for target in ("rm_column_meta_source","rm_index_meta_source","rm_index_type_source","rm_table_meta_source") for arg in ("--test",target)]
def sha(data):return hashlib.sha256(data).hexdigest()
def dump(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
def parse(raw):
    values={}
    for row in raw.decode("utf-8").splitlines():
        name,hex_value=row.split("\t")
        assert name not in values,"duplicate original case"
        values[name]=bytes.fromhex(hex_value).decode("utf-8").split("\x1f")
    return values
def verify(java,rust,events):
    expected={PREFIX+name for name in BINDINGS}
    assert set(java)==set(rust)==expected
    assert len(events)==11 and events[-1]==dict(kind="summary",passed=10,failed=0,skipped=0)
    cases=events[:-1]
    assert len({r["case_id"] for r in cases})==10
    for r in cases:
        assert r["kind"]=="case" and r["status"]=="SUCCESSFUL"
        key=r["class"]+"#"+r["method"]
        assert key in expected
        assert r["case_id"]==f'[engine:junit-jupiter]/[class:{r["class"]}]/[method:{r["method"]}()]'
    assert {r["class"]+"#"+r["method"] for r in cases}==expected
    assert sum(map(len,java.values()))==51
    assert rust==java,"original assertion arguments/order differ"
def negative_controls(java,rust,events,raw):
    rejected=[]
    for name in ("missing_case","duplicate_row","wrong_exception","wrong_message","wrong_order","missing_assertion","wrong_namespace","failed_java_case"):
        j=copy.deepcopy(java);r=copy.deepcopy(rust);e=copy.deepcopy(events)
        key=PREFIX+"TableMetaTest#testContainsPK"
        if name=="missing_case":r.pop(key)
        elif name=="duplicate_row":
            try:parse(raw+raw.splitlines(keepends=True)[-1])
            except (AssertionError,ValueError):rejected.append(dict(control=name,rejected=True));continue
            raise AssertionError("duplicate observation accepted")
        elif name=="wrong_exception":r[PREFIX+"IndexTypeTest#testIndexType"][-1]="throws:java.lang.RuntimeException"
        elif name=="wrong_message":r[key][2]="equals:needs to contain the primary key."
        elif name=="wrong_order":r[key][0],r[key][-1]=r[key][-1],r[key][0]
        elif name=="missing_assertion":j[key].pop();r[key].pop()
        elif name=="wrong_namespace":r[key.replace("rm.datasource.sql.","")]=r.pop(key)
        else:e[0]["status"]="FAILED"
        try:verify(j,r,e)
        except AssertionError:rejected.append(dict(control=name,rejected=True))
        else:raise AssertionError("accepted weakening "+name)
    return rejected
def run(oracle):
    oracle=oracle.resolve()
    work=Path(tempfile.mkdtemp(dir="D:/codex-builds",prefix="seata_rm_metadata_rust_"))
    out=ROOT/"docs/phase-2/evidence/rm-metadata-source"/work.name;out.mkdir(parents=True)
    print("RM_METADATA_RUST "+str(out),flush=True)
    java=json.loads((oracle/"java-assertions.json").read_bytes())
    events=[json.loads(line) for line in (oracle/"original-junit-events.jsonl").read_text().splitlines()]
    env=os.environ|dict(TEMP="D:/Temp",TMP="D:/Temp",CARGO_HOME=str(ROOT.parent/".seata-cargo-cache"),CARGO_INCREMENTAL="0",CARGO_PROFILE_TEST_DEBUG="0")
    commands=[]
    def command(label,args,extra=None,expect=0):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env|(extra or {}),capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+".stdout")).write_bytes(v.stdout);(out/(label+".stderr")).write_bytes(v.stderr)
        commands.append(dict(label=label,argv=list(map(str,args)),exit=v.returncode));dump(out/"commands.json",commands)
        assert v.returncode==expect,(label,v.stderr[-1000:].decode(errors="replace"))
        return v
    verified={}
    for version in ("1.99","1.85"):
        target=ROOT.parent/(".seata-target-nodebug" if version=="1.99" else ".seata-target-nodebug-185")
        capture=work/(version+".tsv")
        command(version+"-source",["cargo","+"+version+".0","test","-p","seata-test",*test_targets(),"--offline","--locked"],dict(CARGO_TARGET_DIR=str(target),SEATA_RM_METADATA_OBSERVATIONS=str(capture)))
        raw=capture.read_bytes();(out/(version+"-observations.tsv")).write_bytes(raw)
        rust=parse(raw);verify(java,rust,events)
        controls=negative_controls(java,rust,events,raw)
        dump(out/(version+"-controls.json"),controls)
        verified[version]=dict(cases=10,assertions=51,controls=len(controls))
    common=["cargo","+1.99.0","test","-p","seata-test",*test_targets(),"--offline","--locked"]
    env["CARGO_TARGET_DIR"]=str(ROOT.parent/".seata-target-nodebug")
    table=ROOT/"crates/seata_sqlparser/src/struct/table_meta.rs"
    enum=ROOT/"crates/seata_sqlparser/src/struct/index_type.rs"
    mutations=[
        ("case-folding",table,b"Ok(primary.iter().all(|name| columns.contains(name)))",b"Ok(primary.iter().all(|name| columns.contains(name)) && false)","original_rm_test_contains_pk"),
        ("invalid-enum",enum,b'_ => Err(InvalidEnumValue::new("IndexType", i)),',b'4 => Ok(Self::NORMAL), _ => Err(InvalidEnumValue::new("IndexType", i)),',"original_rm_test_index_type"),
        ("strip-quotes",table,b"self.all_columns.get(col_name)",b"let clean = col_name.map(|name| JavaString::from_utf16(name.as_utf16().iter().copied().filter(|unit| *unit != 96).collect())); self.all_columns.get(clean.as_ref())","original_rm_test_get_column_meta"),
    ]
    mutation_results=[]
    for name,p,needle,replacement,test in mutations:
        original=p.read_bytes()
        assert original.count(needle)==1,name
        try:
            p.write_bytes(original.replace(needle,replacement))
            v=subprocess.run(common+[test,"--","--exact"],cwd=ROOT,env=env,capture_output=True,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW)
            (out/("mutation-"+name+".stdout")).write_bytes(v.stdout);(out/("mutation-"+name+".stderr")).write_bytes(v.stderr)
            assert v.returncode!=0 and b"test result: FAILED" in v.stdout and test.encode() in v.stdout,(name,v.stdout,v.stderr)
            mutation_results.append(dict(mutation=name,target=test,rejected=True,original_sha256=sha(original)))
        finally:p.write_bytes(original)
    capture=work/"restored.tsv"
    command("restored-source",common,dict(SEATA_RM_METADATA_OBSERVATIONS=str(capture)))
    raw=capture.read_bytes();(out/"restored-observations.tsv").write_bytes(raw);verify(java,parse(raw),events)
    dump(out/"mutations.json",mutation_results)
    bindings=[]
    for event in events[:-1]:
        key=event["class"]+"#"+event["method"]
        bindings.append(dict(case_id=event["case_id"],rust_test=BINDINGS[key.removeprefix(PREFIX)],status="MATCH",assertions=len(java[key]),rust_class_file={"ColumnMetaTest":"column_meta_test.rs","IndexMetaTest":"index_meta_test.rs","IndexTypeTest":"index_type_test.rs","TableMetaTest":"table_meta_test.rs"}[event["class"].rsplit(".",1)[1]]))
    dump(out/"source-case-bindings.json",bindings)
    dump(out/"result.json",dict(oracle=str(oracle),oracle_assertions_sha256=sha((oracle/"java-assertions.json").read_bytes()),rust=verified,mutations=3,original_methods=10,global_MATCH_total_not_recomputed=True,WP05_complete=False))
    print("PASS Rust dual-toolchain ten original cases / 51 assertions and three source mutations; "+str(out),flush=True)
    return out
if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("oracle",type=Path)
    args=parser.parse_args();run(args.oracle)
