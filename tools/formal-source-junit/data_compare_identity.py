"""Run the pinned Java comparison tests and observe its duplicate-key behavior."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import re
from collections import defaultdict

from jdbc_driver_jvm import BASE, BASE_CATALOG, BASE_WORK, HERE, JAVA, PHASE, PIN, ROOT, sha
from validate import load

METHODS = ("DataCompareUtilsTest", "DataCompareUtilsBehaviorTest")
ROW_OBSERVATIONS = {
    "serializer_unset": "false:false",
    "serializer_different_case": "false:false",
    "serializer_fastjson": "true:true",
    "duplicate_then_date_failure": "java.lang.ArrayIndexOutOfBoundsException:true:true",
    "keys_before_conversion": "false:true",
}


RECORD_OBSERVATIONS = {
    "null_records": "true:false:true:false:false:true",
    "null_metadata": "java.lang.NullPointerException:java.lang.NullPointerException",
    "left_metadata_and_updates": "true:true:false",
}
JAVA_PACKAGE = "org/apache/seata/rm/datasource"


def comparison_observations(output, label, expected):
    rows = re.findall(rf"{label}\t([^\t\r\n]+)\t([^\r\n]+)", output)
    assert len(rows) == len(expected) and dict(rows) == expected, rows
    return dict(rows)


def run(rust_source=False):
    work = Path(tempfile.mkdtemp(prefix="seata_data_compare_", dir="D:/codex-builds"))
    family = "mysql-record-compare-source" if rust_source else "mysql-record-comparison"
    out = PHASE/"evidence"/family/work.name
    out.mkdir(parents=True)
    print(out, flush=True)
    for name in ("home", "tmp", "selected", "oracle", "assets"):
        (work/name).mkdir()
    env = os.environ | dict(TEMP="D:/Temp", TMP="D:/Temp")
    for key in ("CLASSPATH", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS"):
        env.pop(key, None)
    commands = []

    def command(args, label, *, environment=None):
        argv = list(map(str,args))
        result = subprocess.run(argv, cwd=work, env=env|(environment or {}), capture_output=True, timeout=120,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        (out/f"{label}.stdout").write_bytes(result.stdout)
        (out/f"{label}.stderr").write_bytes(result.stderr)
        commands.append(dict(label=label, argv=argv, exit=result.returncode))
        if result.returncode:
            raise RuntimeError(f"{label}: {result.stderr[-2000:].decode(errors='replace')}")
        return result.stdout

    assert sha(BASE/"catalog.json")==BASE_CATALOG
    baseline = {r["path"]:r["sha256"] for r in json.loads((BASE/"catalog.json").read_bytes())["members"]}
    for leaf in ("commands.json","classpath-inputs.json"):
        assert sha(BASE/leaf)==baseline[leaf]
    rows = json.loads((BASE/"classpath-inputs.json").read_bytes())
    expected = {row["path"] for row in rows}
    assert len(expected)==len(rows)
    assert {p.relative_to(BASE_WORK).as_posix() for p in (BASE_WORK/"cp").rglob("*") if p.is_file()}==expected
    for row in rows:
        assert sha(BASE_WORK/row["path"])==row["sha256"]
    original, = [r["argv"] for r in json.loads((BASE/"commands.json").read_bytes())
                 if r["name"]=="junit-original-exact"]
    cp = original[original.index("-cp")+1].split(os.pathsep,1)[1]
    oracle_repo = ROOT.parent/"seata-java-phase09-oracle"
    resource_root = "rm-datasource/src/test/resources"
    names = command(["git","-C",oracle_repo,"ls-tree","-r","--name-only",PIN,"--",resource_root],"resource-list")
    resources = names.decode("utf-8").splitlines()
    assert len(resources)==7 and all(name.startswith(resource_root+"/") for name in resources)
    for index, entry in enumerate(resources):
        relative = Path(entry.removeprefix(resource_root+"/"))
        assert not relative.is_absolute() and ".." not in relative.parts
        value = command(["git","-C",oracle_repo,"cat-file","blob",f"{PIN}:{entry}"],f"resource-{index}")
        dest = work/"assets"/relative
        dest.parent.mkdir(parents=True,exist_ok=True)
        dest.write_bytes(value)
    cp = os.pathsep.join([str(work/"assets"),cp])
    sources = []
    for name in METHODS:
        leaf = f"{name}.java"
        source = command(["git","-C",oracle_repo,"cat-file","blob",
                          f"{PIN}:rm-datasource/src/test/java/{JAVA_PACKAGE}/{leaf}"],f"source-{name}")
        dest = work/leaf
        dest.write_bytes(source)
        sources.append(dest)
    command([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",cp,"-d",work/"selected",*sources],"javac-original")
    events_path = out/"original-junit.jsonl"
    header = [JAVA,"-Duser.home="+str(work/"home"),"-Djava.io.tmpdir="+str(work/"tmp"),
              "-Duser.language=en","-Duser.country=US","-Duser.timezone=UTC","-XX:+EnableDynamicAgentLoading"]
    command([*header,"-cp",os.pathsep.join([str(work/"selected"),cp]),"SourceJUnitRunner",
             work/"selected",events_path,"execute"],"original-junit")
    events = load(events_path)
    finished = [r for r in events if r["kind"]=="finished" and r["test"]]
    assert len(finished)==12 and len({r["id"] for r in finished})==12
    assert {r["class"].rsplit(".",1)[-1] for r in finished}==set(METHODS)
    assert all(r["status"]=="SUCCESSFUL" for r in finished)
    assert events[-1]["passed"]==12 and events[-1]["failed"]==events[-1]["guard_denials"]==0
    probe = HERE/"DataCompareIdentityOracle.java"
    command([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",cp,"-d",work/"oracle",probe],"javac-probe")
    values = command([*header,"-cp",os.pathsep.join([str(work/"oracle"),cp]),"DataCompareIdentityOracle"],"java-probe")
    expected_proxy = dict(right_duplicate="false",left_duplicate="true",both_duplicate="true",
        colliding_key_reordered="false",different_composite_key="false",colliding_mapper_rows="1")
    expected_proxy.update({
        "decimal_key_0.0000010": "0.0000010",
        "decimal_key_0.0000001": "1E-7",
        "decimal_key_-0.0000001200": "-1.200E-7",
        "decimal_key_0.0000000": "0E-7",
        "decimal_key_12.0000000": "12.0000000",
        "space_only_date": "java.lang.ArrayIndexOutOfBoundsException:Index 0 out of bounds for length 0",
    })
    probe_output = values.decode("utf-8")
    row_observations = comparison_observations(probe_output, "SEATA_COMPARE_ROWS", ROW_OBSERVATIONS)
    record_observations = comparison_observations(probe_output, "SEATA_COMPARE_RECORDS", RECORD_OBSERVATIONS)
    observations = dict(line.split("\t") for line in probe_output.splitlines() if line.count("\t") == 1)
    assert observations==expected_proxy,observations
    (out/probe.name).write_bytes(probe.read_bytes())
    for path in (work/"selected").rglob("*.class"):
        (out/path.name).write_bytes(path.read_bytes())
    (out/"DataCompareIdentityOracle.class").write_bytes((work/"oracle/DataCompareIdentityOracle.class").read_bytes())
    matched = 0
    if rust_source:
        matched = compare_assertions(work,out,cp,header,finished,sources,observations,row_observations,record_observations,command)
    (out/"commands.json").write_text(json.dumps(commands,indent=2)+"\n",encoding="utf-8")
    result = dict(java_pin=PIN,original_cases_passed=12,original_cases_Rust_MATCH=matched,observations=observations,
                  row_observations=row_observations, record_observations=record_observations, probe_source_sha256=sha(probe), baseline_catalog_sha256=BASE_CATALOG)
    (out/"result.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    members = [dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir())]
    (out/"catalog.json").write_text(json.dumps(dict(members=members),indent=2)+"\n",encoding="utf-8")
    print(out,sha(out/"catalog.json"),flush=True)


def compare_assertions(work,out,cp,header,finished,sources,identity_observations,row_observations,record_observations,command):
    generated = work/"recorded-source"
    generated.mkdir()
    observed = work/"recorded-classes"
    observed.mkdir()
    recorder = HERE/"DataCompareAssertionRecorder.java"
    temporal = HERE/"DataCompareTemporalHost.java"
    command([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",cp,"-d",work/"oracle",recorder,temporal],
            "javac-assertion-fixtures")
    observed_cp = os.pathsep.join(map(str,[work/"oracle",cp]))
    expression = r"(?m)^(    (?:public )?void (\w+)\(([^\n]*)\) \{)"
    for source in sources:
        text = source.read_text("utf-8")
        text, calls = re.subn(r"\b(?:Assertions\.)?(assertTrue|assertFalse|assertEquals)\(",
                               r"DataCompareAssertionRecorder.\1(",text)
        assert calls=={"DataCompareUtilsTest":28,"DataCompareUtilsBehaviorTest":9}[source.stem]
        def injection(match):
            method = match[2]
            suffix = ' + ":" + type' if match[3] else ''
            return match[1]+f'\n        DataCompareAssertionRecorder.start("{source.stem}#{method}"{suffix});'
        text, methods = re.subn(expression,injection,text)
        assert methods in (3,5)
        (generated/source.name).write_text(text,encoding="utf-8")
        (out/("recorded-"+source.name)).write_text(text,encoding="utf-8")
    command([JAVA.with_name("javac.exe"),"-encoding","UTF-8","-cp",observed_cp,"-d",observed,
             *(generated/source.name for source in sources)],"javac-recorded-tests")
    raw = command([*header,"-cp",os.pathsep.join(map(str,[observed,observed_cp])),"SourceJUnitRunner",observed,
                   out/"recorded-assertion-events.jsonl","execute"],"java-values")
    events = load(out/"recorded-assertion-events.jsonl")
    assert {row["id"] for row in events if row["kind"]=="finished" and row["test"] and row["status"]=="SUCCESSFUL"}=={row["id"] for row in finished}
    assert events[-1]["passed"]==12 and events[-1]["failed"]==0
    java = defaultdict(str)
    for name,value in re.findall(r"^SEATA_COMPARE_JAVA\t([^\t\r\n]+)\t([^\r\n]+)",raw.decode("utf-8"),re.M):
        java[name] += value
    assert len(java)==12 and sum(len(x.replace(":","")) for x in java.values())==45
    outputs = {}
    jdbc_values = None
    local_values = None
    for version in ("1.99","1.85"):
        cargo = "cargo"
        target = "D:/cargo-target/seata-rust-load-driver"
        env = dict(CARGO_HOME=str(ROOT.parent/".seata-cargo-cache"),CARGO_INCREMENTAL="0")
        if version=="1.85":
            rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
            cargo = rust/"cargo.exe"
            target += "-msrv"
            env.update(RUSTC=str(rust/"rustc.exe"),RUSTDOC=str(rust/"rustdoc.exe"))
        env["CARGO_TARGET_DIR"] = target
        command([cargo,"--version"],f"{version}-cargo")
        trace = command([cargo,"test","--manifest-path",ROOT/"Cargo.toml","--locked","--offline",
            "-p","seata-db-mysql","--test","data_compare_source","--","--include-ignored","--nocapture","--test-threads=1"],
            f"{version}-source",environment=env)
        rows = re.findall(r"SEATA_DATA_COMPARE\t([^\t\r\n]+)\t([^\r\n]+)",trace.decode("utf-8"))
        assert len(rows)==12 and dict(rows)==java,(rows,java)
        additional = re.findall(r"SEATA_COMPARE_EXTRA\t([^\t\r\n]+)\t([^\r\n]+)",trace.decode("utf-8"))
        assert len(additional)==len(identity_observations) and dict(additional)==identity_observations
        assert comparison_observations(trace.decode("utf-8"), "SEATA_COMPARE_ROWS", ROW_OBSERVATIONS) == row_observations
        assert comparison_observations(trace.decode("utf-8"), "SEATA_COMPARE_RECORDS", RECORD_OBSERVATIONS) == record_observations
        jdbc_rows = re.findall(
            r"SEATA_COMPARE_JDBC_UTC\t([^\t\r\n]+)\t([^\t\r\n]+)\t([^\r\n]+)", trace.decode("utf-8")
        )
        assert len(jdbc_rows) == len({(kind, value) for kind, value, _ in jdbc_rows}) == 24
        if jdbc_values is None:
            jdbc_values = []
            for index, (kind, value, _) in enumerate(jdbc_rows):
                units = value.encode("utf-16-be", errors="surrogatepass").hex()
                observed_value = command(
                    [*header, "-cp", work/"oracle", "DataCompareTemporalHost", "--jdbc", kind, units],
                    f"jdbc-utc-java-{index:02d}",
                ).decode("ascii").strip()
                jdbc_values.append((kind, value, observed_value))
            (out/"jdbc-utc-java.tsv").write_text(
                "".join("\t".join(row)+"\n" for row in jdbc_values), encoding="utf-8"
            )
        assert jdbc_rows == jdbc_values, (jdbc_rows, jdbc_values)
        local_rows = re.findall(
            r"SEATA_COMPARE_LOCAL_TIME\t([^\t\r\n]+)\t([^\r\n]+)", trace.decode("utf-8")
        )
        assert len(local_rows) == len({value for value, _ in local_rows}) == 29
        if local_values is None:
            local_values = []
            for index, (value, _) in enumerate(local_rows):
                units = value.encode("utf-16-be", errors="surrogatepass").hex()
                observed_value = command(
                    [*header, "-cp", work/"oracle", "DataCompareTemporalHost", "--local", units],
                    f"local-time-java-{index:02d}",
                ).decode("ascii").strip()
                local_values.append((value, observed_value))
            (out/"local-time-java.tsv").write_text(
                "".join("\t".join(row)+"\n" for row in local_values), encoding="utf-8"
            )
        assert local_rows == local_values, (local_rows, local_values)
        outputs[version] = dict(rows)
    cases = []
    for row in finished:
        key = row["class"].rsplit(".",1)[-1]+"#"+row["method"]
        argument = re.match(r"^\[\d+\] (-?\d+),",row["display"])
        if argument:
            key += ":"+argument[1]
        cases.append(dict(case_id=row["id"],case=key,java=java[key],rust={v:x[key] for v,x in outputs.items()},status="MATCH"))
    assert len({r["case"] for r in cases})==12
    (out/"case-bindings.json").write_text(json.dumps(cases,indent=2)+"\n",encoding="utf-8")
    for fixture in (temporal,recorder):
        (out/fixture.name).write_bytes(fixture.read_bytes())
    for artifact in (work/"oracle").rglob("*.class"):
        (out/artifact.name).write_bytes(artifact.read_bytes())
    return len(cases)


if __name__=="__main__":
    cli = argparse.ArgumentParser()
    cli.add_argument("--rust-source",action="store_true")
    run(cli.parse_args().rust_source)
