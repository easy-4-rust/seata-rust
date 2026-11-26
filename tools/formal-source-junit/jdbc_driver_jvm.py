"""Execute the sealed Java suite and compare four real JVM-backed Rust cases."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

from validate import load

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
PHASE = ROOT/"docs/phase-2"
JAVA = Path("D:/Java/jdk-17.0.12/bin/java.exe")
PIN = "cba73e180fd8896a99df6415b859d275122feaa7"
BASE = PHASE/"evidence/sqlparser-rm-jdbc-source/seata_jdbc_source_5oazvawu"
TEST_CLASS = "org.apache.seata.rm.datasource.util.JdbcUtilsTest"
BASE_CATALOG = "42eb747c3ae8bbcff58739f5b73980cb19d0dfbf37e3bb290823082c0630193d"
BASE_WORK = Path("D:/codex-builds")/BASE.name


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(initialization=False):
    work = Path(tempfile.mkdtemp(prefix="seata_jdbc_driver_jvm_",dir="D:/codex-builds"))
    family = "sqlparser-rm-jdbc-init-source" if initialization else "sqlparser-rm-jdbc-driver-jvm"
    out = PHASE/"evidence"/family/work.name
    out.mkdir(parents=True)
    print(out,flush=True)
    for folder in ("home","tmp","host","oracle"):
        (work/folder).mkdir()
    env = os.environ | dict(TEMP="D:/Temp",TMP="D:/Temp",PYTHONUTF8="1",PYTHONDONTWRITEBYTECODE="1",
        CARGO_HOME=str(ROOT.parent/".seata-cargo-cache"),CARGO_INCREMENTAL="0")
    for key in ("CLASSPATH","JAVA_TOOL_OPTIONS","JDK_JAVA_OPTIONS","_JAVA_OPTIONS"):
        env.pop(key,None)
    commands = []

    def invoke(args,name,*,extra=None,expect=0,cwd=work):
        command = list(map(str,args))
        result = subprocess.run(command,cwd=cwd,env=env|(extra or {}),capture_output=True,timeout=300,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        (out/f"{name}.stdout").write_bytes(result.stdout)
        (out/f"{name}.stderr").write_bytes(result.stderr)
        commands.append(dict(name=name,argv=command,exit=result.returncode))
        (out/"commands.json").write_text(json.dumps(commands,indent=2)+"\n",encoding="utf-8")
        if result.returncode!=expect:
            raise RuntimeError(f"{name}: exit {result.returncode}; {result.stderr[-1500:].decode(errors='replace')}")
        return result.stdout.decode("utf-8")

    assert sha(BASE/"catalog.json")==BASE_CATALOG
    catalog = {m["path"]:m["sha256"] for m in json.loads((BASE/"catalog.json").read_bytes())["members"]}
    for leaf in ("classpath-inputs.json","commands.json","JdbcUtilsTest.original.class","JdbcUtilsTest.java",
                 "original-exact-events.jsonl"):
        assert sha(BASE/leaf)==catalog[leaf]
    input_rows = json.loads((BASE/"classpath-inputs.json").read_bytes())
    expected_classpath = {entry["path"] for entry in input_rows}
    actual_classpath = {path.relative_to(BASE_WORK).as_posix() for path in (BASE_WORK/"cp").rglob("*")
                        if path.is_file()}
    assert len(expected_classpath)==len(input_rows) and actual_classpath==expected_classpath
    for entry in input_rows:
        assert sha(BASE_WORK/entry["path"])==entry["sha256"], entry["path"]
    runtime = [x["argv"] for x in json.loads((BASE/"commands.json").read_bytes()) if x["name"]=="junit-original-exact"]
    original = runtime[0]
    cp = original[original.index("-cp")+1]
    selected = Path(cp.split(os.pathsep)[0])
    selected_class = selected/Path(*TEST_CLASS.split(".")).with_suffix(".class")
    assert {path for path in selected.rglob("*") if path.is_file()}=={selected_class}
    assert sha(selected_class)==catalog["JdbcUtilsTest.original.class"]
    oracle_repo = ROOT.parent/"seata-java-phase09-oracle"
    raw_source = invoke(["git","-C",oracle_repo,"cat-file","blob",
        PIN+":rm-datasource/src/test/java/org/apache/seata/rm/datasource/util/JdbcUtilsTest.java"],"git-source")
    assert raw_source.encode("utf-8")== (BASE/"JdbcUtilsTest.java").read_bytes()
    invoke([JAVA,"-version"],"java-version")
    header = [JAVA,"-Duser.home="+str(work/"home"),"-Djava.io.tmpdir="+str(work/"tmp"),
              "-Duser.language=en","-Duser.country=US"]
    events = out/"original-junit-events.jsonl"
    invoke([*header,"-XX:+EnableDynamicAgentLoading","-cp",cp,"SourceJUnitRunner",selected,events,"execute"],
           "original-junit")
    current = load(events)
    baseline = load(BASE/"original-exact-events.jsonl")
    assert all(current[0][field]==baseline[0][field] for field in
               ("mode","java_version","jupiter_version","jupiter_origin","launcher_origin","classpath"))
    assert [r["sequence"] for r in current]==list(range(1,len(current)+1))
    # Re-run the exact sealed class and preserve its entire selected JUnit lifecycle.
    assert current[1:]==baseline[1:]
    assert current[-1]["passed"]==21 and current[-1]["failed"]==current[-1]["guard_denials"]==0
    actual = [r for r in current if r["kind"]=="finished" and r["test"]]
    assert len(actual)==21 and all(r["status"]=="SUCCESSFUL" for r in actual)
    h2 = BASE_WORK/"cp/75/h2-1.4.181.jar"
    mysql = BASE_WORK/"cp/89/mysql-connector-java-5.1.42.jar"
    invoke([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",h2,"-d",work/"host",
            HERE/"JdbcDriverPrimitiveHost.java"],"javac-host")
    invoke([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",cp,"-d",work/"oracle",
            HERE/"JdbcDriverCaseOracle.java"],"javac-native-oracle")
    java_rows = invoke([*header,"-cp",os.pathsep.join([str(work/"oracle"),cp]),"JdbcDriverCaseOracle"],
                       "native-java-values")
    java_values = dict(re.findall(r"^(testLoadDriver[^\t\r\n]+)\t([^\r\n]+)",java_rows,re.M))
    assert len(java_values)==4
    init_values = {}
    if initialization:
        invoke([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",cp,"-d",work/"oracle",
                HERE/"JdbcInitCaseOracle.java"],"javac-init-oracle")
        init_rows = invoke([*header,"-XX:+EnableDynamicAgentLoading","-cp",
            os.pathsep.join([str(work/"oracle"),cp]),"JdbcInitCaseOracle"],"native-init-values")
        rows = re.findall(r"^INIT\t([^\t\r\n]+)\t([^\r\n]+)",init_rows,re.M)
        init_values = dict(rows)
        assert len(rows)==len(init_values)==7 and not set(java_values)&set(init_values)
        for artifact in (work/"oracle").glob("JdbcInitCaseOracle*.class"):
            (out/artifact.name).write_bytes(artifact.read_bytes())
    for key in ("JdbcDriverCaseOracle","JdbcDriverPrimitiveHost"):
        location = "oracle" if key=="JdbcDriverCaseOracle" else "host"
        (out/f"{key}.class").write_bytes((work/location/f"{key}.class").read_bytes())
    outputs = {}
    for version in ("1.99","1.85"):
        target = "D:/cargo-target/seata-rust-load-driver"
        cargo = "cargo"
        extra = {}
        if version=="1.85":
            rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
            cargo = rust/"cargo.exe"
            target += "-msrv"
            extra.update(RUSTC=str(rust/"rustc.exe"),RUSTDOC=str(rust/"rustdoc.exe"))
        classpath = os.pathsep.join(map(str,[work/"host",h2]))
        extra.update(CARGO_TARGET_DIR=target,SEATA_JDBC_JAVA=str(JAVA),SEATA_JDBC_PRIMITIVE_CLASSPATH=classpath,
                     SEATA_JDBC_JVM_HOME=str(work/"home"))
        invoke([cargo,"--version"],f"{version}-cargo-version",extra=extra,cwd=ROOT)
        prefix = [cargo,"test","--offline","--locked","-p","seata-db-mysql","--test","jdbc_driver_jvm_source"]
        stdout = invoke([*prefix,"--","--ignored","--nocapture","--test-threads=1"],f"{version}-jvm",
                        extra=extra,cwd=ROOT)
        rust_values = re.findall(r"SEATA_JDBC_JVM_SOURCE\t([^\t\r\n]+)\t([^\r\n]+)",stdout)
        assert len(rust_values)==4 and dict(rust_values)==java_values
        if version=="1.99":
            red = invoke([*prefix,"--","--ignored","--exact","test_load_driver_with_valid_h2_driver"],
                         "runtime-without-h2",extra=extra|{"SEATA_JDBC_PRIMITIVE_CLASSPATH":str(work/"host")},
                         cwd=ROOT,expect=101)
            assert "test result: FAILED." in red and "java.lang.ClassNotFoundException" in red
        if initialization:
            prefix = [cargo,"test","--offline","--locked","-p","seata-db-mysql","--test","jdbc_resource_init_source"]
            mysql_path = os.pathsep.join([classpath,str(mysql)])
            stdout = invoke([*prefix,"--","--ignored","--nocapture","--test-threads=1"],f"{version}-init-jvm",
                extra=extra|{"SEATA_JDBC_PRIMITIVE_CLASSPATH":mysql_path},cwd=ROOT)
            init_rows = re.findall(r"SEATA_JDBC_INIT_SOURCE\t([^\t\r\n]+)\t([^\r\n]+)",stdout)
            assert len(init_rows)==7 and dict(init_rows)==init_values, (init_rows,init_values)
            rust_values.extend(init_rows)
            if version=="1.99":
                red = invoke([*prefix,"--","--ignored","--exact","test_init_data_source_resource_success"],
                    "runtime-without-mysql-driver",extra=extra,cwd=ROOT,expect=101)
                assert "test result: FAILED." in red
        outputs[version] = dict(rust_values)
    all_values = java_values|init_values
    bindings = [dict(id=row["id"],method=row["method"],java=all_values[row["method"]],
        rust={v:x[row["method"]] for v,x in outputs.items()},status="MATCH")
        for row in actual if row["method"] in all_values]
    assert len(bindings)==len(all_values)
    (out/"case-bindings.json").write_text(json.dumps(bindings,indent=2)+"\n",encoding="utf-8")
    sources = [HERE/"JdbcDriverPrimitiveHost.java",HERE/"JdbcDriverCaseOracle.java",
               ROOT/"crates/seata_db_mysql/tests/jdbc_driver_jvm_source.rs",
               ROOT/"crates/seata_db_mysql/tests/support/jdbc_driver_primitive_host.rs",
               ROOT/"crates/seata_db_mysql/src/util/jdbc_utils.rs",ROOT/"Cargo.lock",Path(__file__)]
    if initialization:
        sources.extend([HERE/"JdbcInitCaseOracle.java",ROOT/"crates/seata_db_mysql/tests/jdbc_resource_init_source.rs"])
    result = dict(java_pin=PIN,original_junit_passed=21,driver_source_cases_MATCH=4,prior_jdbc_cases_MATCH=10,
                  initialization_source_cases_MATCH=len(init_values),rm_jdbc_cases_MATCH=10+len(all_values),
                  h2_sha256=sha(h2),mysql_fixture_driver_sha256=sha(mysql),java_sha256=sha(JAVA),
                  baseline_catalog_sha256=BASE_CATALOG,
                  verified_inputs={f.relative_to(ROOT).as_posix():sha(f) for f in sources},production_jvm_host=False)
    (out/"result.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    members = [dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir())]
    (out/"catalog.json").write_text(json.dumps(dict(members=members),indent=2)+"\n",encoding="utf-8")
    print(out,sha(out/"catalog.json"),flush=True)


if __name__=="__main__":
    cli = argparse.ArgumentParser()
    cli.add_argument("--initialization",action="store_true")
    run(cli.parse_args().initialization)
