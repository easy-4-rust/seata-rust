"""Execute the fixed Java Fastjson parser and compare physical Rust Undo restoration."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

from jdbc_driver_jvm import BASE, BASE_CATALOG, BASE_WORK, HERE, JAVA, PHASE, PIN, ROOT, sha


def run():
    assert sha(BASE / "catalog.json") == BASE_CATALOG
    baseline = {row["path"]: row["sha256"] for row in json.loads((BASE / "catalog.json").read_bytes())["members"]}
    for name in ("commands.json", "classpath-inputs.json"):
        assert sha(BASE / name) == baseline[name]
    classpath_inputs = json.loads((BASE / "classpath-inputs.json").read_bytes())
    expected = {row["path"] for row in classpath_inputs}
    assert len(expected) == len(classpath_inputs)
    assert {file.relative_to(BASE_WORK).as_posix() for file in (BASE_WORK / "cp").rglob("*") if file.is_file()} == expected
    for row in classpath_inputs:
        assert sha(BASE_WORK / row["path"]) == row["sha256"]
    original, = [row["argv"] for row in json.loads((BASE / "commands.json").read_bytes())
                 if row["name"] == "junit-original-exact"]
    cp = original[original.index("-cp") + 1].split(os.pathsep, 1)[1]
    work = Path(tempfile.mkdtemp(dir="D:/codex-builds", prefix="seata_fastjson_native_"))
    out = PHASE / "evidence/fastjson-undo-native" / work.name
    for folder in (out, work / "probe"):
        folder.mkdir(parents=True)
    print(out, flush=True)
    commands = []
    env = os.environ | dict(TEMP="D:/Temp", TMP="D:/Temp")
    for key in ("CLASSPATH", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS"):
        env.pop(key, None)

    def command(label, args, *, extra=None):
        argv = list(map(str, args))
        result = subprocess.run(argv, cwd=work, env=env | (extra or {}), capture_output=True,
                                timeout=120, creationflags=subprocess.CREATE_NO_WINDOW)
        commands.append(dict(label=label, argv=argv, exit=result.returncode))
        (out / f"{label}.stdout").write_bytes(result.stdout)
        (out / f"{label}.stderr").write_bytes(result.stderr)
        (out / "commands.json").write_text(json.dumps(commands, indent=2)+"\n", encoding="utf-8")
        if result.returncode:
            raise RuntimeError(f"{label}: {result.stderr[-1600:].decode(errors='replace')}")
        return result.stdout

    header = [JAVA, "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US",
              "-Duser.timezone=UTC", f"-Duser.home={work}", "-Djava.io.tmpdir=D:/Temp",
              "-XX:+EnableDynamicAgentLoading"]
    original_path = "rm-datasource/src/test/java/org/apache/seata/rm/datasource/undo/parser/FastjsonUndoLogParserTest.java"
    command("original-source", ["git", "-C", ROOT.parent / "seata-java-phase09-oracle",
                                                 "cat-file", "blob", f"{PIN}:{original_path}"])
    junit_baseline = PHASE / "evidence/source-junit-gate/run_20261118_013909_5a5c9888"
    junit_catalog = junit_baseline / "catalog.json"
    assert sha(junit_catalog) == "373012b628b4e74b683c6c2160cca550fdde19b4e48d8d914db8dfba3d37d5a9"
    junit_members = {row["path"]: row["sha256"] for row in json.loads(junit_catalog.read_bytes())["members"]}
    ledger = junit_baseline / "concrete-source-cases.json"
    assert sha(ledger) == junit_members[ledger.name]
    finished = [row for row in json.loads(ledger.read_bytes())
                if "[class:org.apache.seata.rm.datasource.undo.parser.FastjsonUndoLogParserTest]" in row["case_id"]]
    assert len(finished) == len({row["case_id"] for row in finished}) and finished
    assert all(row["Java_status"] == "PASSED" for row in finished)
    (out / "original-case-ledger.json").write_text(json.dumps(finished, indent=2)+"\n", encoding="utf-8")
    javac = JAVA.with_name("javac.exe")

    probe = HERE / "FastjsonUndoNativeOracle.java"
    (out / probe.name).write_bytes(probe.read_bytes())
    command("compile-wire-probe", [javac, "-encoding", "UTF-8", "-cp", cp, "-d", work / "probe", probe])
    wire = out / "native-wire.txt"
    java = command("java-native-values", [*header, "-cp", os.pathsep.join((str(work / "probe"), cp)),
                                         "FastjsonUndoNativeOracle", wire]).decode("utf-8").splitlines()
    assert sum(row.startswith("FIELD\t") for row in java) == 36
    assert sum(row.startswith("IMAGE\t") for row in java) == 6
    assert [row.split("\t")[1] for row in java if row.startswith("LOG\t")] == ["INSERT", "UPDATE", "DELETE"]
    fixture = ROOT / "crates/seata_at/tests/fixtures/fastjson_native_wire.txt"
    assert fixture.read_bytes() == wire.read_bytes()
    escaping = HERE / "FastjsonUndoEscapingOracle.java"
    (out / escaping.name).write_bytes(escaping.read_bytes())
    command("compile-escaping-probe", [javac, "-encoding", "UTF-8", "-cp", cp, "-d", work / "probe", escaping])
    observed_scalars = command("java-scalar-values", [*header, "-cp", os.pathsep.join((str(work / "probe"), cp)),
                                                       "FastjsonUndoEscapingOracle"]).decode("utf-8").splitlines()
    scalar_markers = ("SEATA_FASTJSON_ESCAPE\t", "SEATA_FASTJSON_LONG_OVERFLOW\t")
    assert len(observed_scalars) == len(scalar_markers)
    assert all(line.startswith(marker) for line, marker in zip(observed_scalars, scalar_markers))
    versions = {}
    for version in ("1.99", "1.85"):
        cargo = "cargo"
        target = "D:/cargo-target/seata-rust-load-driver"
        extra = dict(CARGO_HOME=str(ROOT.parent / ".seata-cargo-cache"), CARGO_INCREMENTAL="0")
        if version == "1.85":
            tools = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
            cargo = tools / "cargo.exe"
            target += "-msrv"
            extra.update(RUSTC=str(tools / "rustc.exe"), RUSTDOC=str(tools / "rustdoc.exe"))
        extra["CARGO_TARGET_DIR"] = target
        versions[version] = command(f"{version}-version", [cargo, "--version"], extra=extra).decode().strip()
        output = command(f"{version}-native", [cargo, "test", "--locked", "--offline", "--manifest-path",
                          ROOT / "Cargo.toml", "-p", "seata-at", "--test", "fastjson_undo_native",
                          "--", "--nocapture", "--test-threads=1"], extra=extra).decode("utf-8").splitlines()
        marker = "SEATA_FASTJSON_NATIVE\t"
        assert [line.split(marker, 1)[1] for line in output if marker in line] == java
        for marker, expected_value in zip(scalar_markers, observed_scalars):
            actual, = [line[line.index(marker):] for line in output if marker in line]
            assert actual == expected_value
        assert any("test result: ok. 11 passed; 0 failed; 2 ignored" in line for line in output)
    for artifact in (work / "probe").rglob("*.class"):
        (out / artifact.name).write_bytes(artifact.read_bytes())
    result = dict(java_pin=PIN, baseline_catalog_sha256=BASE_CATALOG, wire_sha256=sha(wire),
                  original_java_case_baseline_sha256=sha(ledger), original_java_reexecuted=False,
                  original_java_cases=[row["case_id"] for row in finished], original_rust_case_count=0,
                  observations_matched=len(java), toolchains=versions)
    (out / "result.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
    catalog = out / "catalog.json"
    members = [{"path": path.name, "sha256": sha(path)} for path in sorted(out.iterdir())]
    catalog.write_text(json.dumps({"members": members}, indent=2)+"\n", encoding="utf-8")
    print(out, sha(catalog), flush=True)


if __name__ == "__main__":
    run()
