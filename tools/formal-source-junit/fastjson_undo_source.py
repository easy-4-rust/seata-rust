"""Run pinned Undo JUnit classes and bind migrated Rust cases."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from jdbc_driver_jvm import BASE, BASE_CATALOG, BASE_WORK, HERE, JAVA, PHASE, PIN, ROOT, sha

CLASS = "org.apache.seata.rm.datasource.undo.parser.FastjsonUndoLogParserTest"
BASE_CLASS = "org.apache.seata.rm.datasource.undo.BaseUndoLogParserTest"
OBSERVATIONS = {
    "testDecodeDefaultContent": (BASE_CLASS, "SEATA_FASTJSON_DEFAULT\t"),
    "testWriteClassName": (CLASS, "SEATA_FASTJSON_WRITE_CLASS\t"),
    "testDirectSerialArraySerialization": (CLASS, "SEATA_FASTJSON_ARRAY_DIRECT\t"),
    "testSerializeAndDeserializeSerialArray": (CLASS, "SEATA_FASTJSON_ARRAY_EMBEDDED\t"),
    "testSerializeAndDeserializeSerialArrayWithNulls": (CLASS, "SEATA_FASTJSON_ARRAY_NULLS\t"),
    "testEncodeAndDecode": (BASE_CLASS, "SEATA_FASTJSON_H2_ROUNDTRIP\t"),
    "testPerformance": (BASE_CLASS, "SEATA_FASTJSON_H2_PERFORMANCE\t"),
}


def run(codec="fastjson", capture_only=False, jackson_arrays=False):
    jsonb = codec == "fastjson2"
    inherited_binary = codec in ("protostuff", "kryo")
    binary = jsonb or inherited_binary or codec == "fory"
    classes = {"fastjson": "Fastjson", "fastjson2": "Fastjson2", "jackson": "Jackson", "protostuff": "Protostuff", "kryo": "Kryo", "fory": "Fory"}
    class_name = CLASS.replace("Fastjson", classes[codec])
    expected_cases = {"jackson": 9, "fastjson": 7, "fastjson2": 5, "protostuff": 4, "kryo": 4, "fory": 3}[codec]
    inherited = {name: spec for name, spec in OBSERVATIONS.items() if spec[0] == BASE_CLASS}
    observations = OBSERVATIONS if codec == "fastjson" else inherited
    if jackson_arrays or jsonb:
        array_marker = "SEATA_FASTJSON2" if jsonb else "SEATA_JACKSON"
        arrays = {
            "testSerializeAndDeserializeSerialArray": (class_name, f"{array_marker}_ARRAY_VALUES\t"),
            "testSerializeAndDeserializeSerialArrayWithNulls": (class_name, f"{array_marker}_ARRAY_NULLS\t"),
        }
        observations = inherited | arrays if jsonb else arrays
    if inherited_binary:
        timestamp_marker = f"SEATA_{codec.upper()}_TIMESTAMP"
        observations = inherited | {
            "testTimestampEncodeAndDecode": (BASE_CLASS, f"{timestamp_marker}\t"),
        }
    assert sha(BASE / "catalog.json") == BASE_CATALOG
    members = {row["path"]: row["sha256"] for row in json.loads((BASE / "catalog.json").read_bytes())["members"]}
    for name in ("classpath-inputs.json", "commands.json"):
        assert sha(BASE / name) == members[name]
    classpath_inputs = json.loads((BASE / "classpath-inputs.json").read_bytes())
    expected = {row["path"] for row in classpath_inputs}
    assert len(expected) == len(classpath_inputs)
    assert {p.relative_to(BASE_WORK).as_posix() for p in (BASE_WORK / "cp").rglob("*") if p.is_file()} == expected
    for row in classpath_inputs:
        assert sha(BASE_WORK / row["path"]) == row["sha256"]
    original, = [row["argv"] for row in json.loads((BASE / "commands.json").read_bytes())
                 if row["name"] == "junit-original-exact"]
    cp = original[original.index("-cp") + 1].split(os.pathsep, 1)[1]
    work = Path(tempfile.mkdtemp(dir="D:/codex-builds", prefix=f"seata_{codec}_junit_"))
    out = PHASE / "evidence/fastjson-undo-native" / work.name
    out.mkdir(parents=True)
    print(out, flush=True)
    for leaf in ("selected", "deps", "src", "assets", "home", "tmp", "recorded"):
        (work / leaf).mkdir()
    env = os.environ | dict(TEMP=str(work / "tmp"), TMP=str(work / "tmp"))
    for key in ("CLASSPATH", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS"):
        env.pop(key, None)
    commands = []

    def command(label, args, extra=None):
        argv = list(map(str, args))
        result = subprocess.run(argv, cwd=work, env=env | (extra or {}), capture_output=True,
                                timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / f"{label}.stdout").write_bytes(result.stdout)
        (out / f"{label}.stderr").write_bytes(result.stderr)
        commands.append(dict(label=label, argv=argv, exit=result.returncode))
        if result.returncode:
            raise RuntimeError(f"{label}: {result.stderr[-3000:].decode(errors='replace')}")
        return result.stdout

    repo = ROOT.parent / "seata-java-phase09-oracle"
    prefix = "rm-datasource/src/test/java/org/apache/seata/rm/datasource/"
    test = prefix + "undo/parser/" + class_name.rsplit(".", 1)[-1] + ".java"
    support = [prefix + "undo/BaseH2Test.java", prefix + "undo/BaseUndoLogParserTest.java",
               prefix + "DataSourceProxyTest.java"]
    support += command("mock-list", ["git", "-C", repo, "ls-tree", "-r", "--name-only", PIN,
                                    "--", prefix + "mock"]).decode().splitlines()
    source_inputs = []
    for index, path in enumerate([*support, test]):
        original = command(f"source-{index:02d}", ["git", "-C", repo, "cat-file", "blob", f"{PIN}:{path}"])
        destination = work / "src" / Path(path).name
        destination.write_bytes(original)
        source_inputs.append(dict(java_path=path, sha256=sha(destination), file=f"source-{index:02d}.stdout"))
    resource = "rm-datasource/src/test/resources/"
    assets = command("asset-list", ["git", "-C", repo, "ls-tree", "-r", "--name-only", PIN,
                                    "--", resource]).decode().splitlines()
    assert len(assets) == 7
    for index, path in enumerate(assets):
        target = work / "assets" / path.removeprefix(resource)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(command(f"asset-{index}", ["git", "-C", repo, "cat-file", "blob", f"{PIN}:{path}"]))
        source_inputs.append(dict(java_path=path, sha256=sha(target), file=f"asset-{index}.stdout"))
    javac = JAVA.with_name("javac.exe")
    command("compile-support", [javac, "-encoding", "UTF-8", "-cp", cp, "-d", work / "deps",
                                *(work / "src" / Path(path).name for path in support)])
    cp = os.pathsep.join((str(work / "deps"), str(work / "assets"), cp))
    command("compile-original", [javac, "-encoding", "UTF-8", "-cp", cp, "-d", work / "selected",
                                 work / "src" / Path(test).name])
    java = [JAVA, "-Dfile.encoding=UTF-8", f"-Duser.home={work / 'home'}",
            f"-Djava.io.tmpdir={work / 'tmp'}", "-Duser.language=en", "-Duser.country=US",
            "-Duser.timezone=UTC", "-XX:+EnableDynamicAgentLoading"]

    if binary:
        java.append("-Dseata.undo.source.binary=true")
    if codec == "kryo":
        java.extend(("--add-opens=java.base/java.util=ALL-UNNAMED",
                     "--add-opens=java.base/java.text=ALL-UNNAMED"))

    def execute(label, classpath):
        events = out / f"{label}.jsonl"
        stdout = command(label, [*java, "-cp", classpath, "SourceJUnitRunner", work / "selected", events, "execute"])
        rows = [json.loads(line) for line in events.read_text(encoding="utf-8").splitlines()]
        leaves = [row for row in rows if row["kind"] == "finished" and row["test"]]
        assert len(leaves) == len({row["id"] for row in leaves}) == expected_cases
        assert all(row["class"] == class_name and row["status"] == "SUCCESSFUL" for row in leaves)
        assert rows[-1]["passed"] == expected_cases
        assert rows[-1]["failed"] == rows[-1]["container_failed"] == rows[-1]["guard_denials"] == 0
        assert not (work / "db_store").exists()
        return stdout, leaves

    selected_cp = os.pathsep.join((str(work / "selected"), cp))
    raw, original_leaves = execute("original-junit", selected_cp)
    assert all(marker.encode() not in raw for _, marker in observations.values())
    governing = work / "src" / "BaseUndoLogParserTest.java"
    source = governing.read_text(encoding="utf-8")
    last = "        Assertions.assertNull(branchUndoLog.getSqlUndoLogs());"
    assert source.count(last) == 1
    source = source.replace(last, last + """
        System.out.printf("SEATA_FASTJSON_DEFAULT\\t%s\\t%b\\t%b\\t%d\\t%b%n",
                FastjsonUndoH2Observer.wireText(defaultContent),
                java.util.Objects.nonNull(branchUndoLog), java.util.Objects.isNull(branchUndoLog.getXid()),
                branchUndoLog.getBranchId(), java.util.Objects.isNull(branchUndoLog.getSqlUndoLogs()));""")
    start = source.index("    void testEncodeAndDecode()")
    end = source.index("\n    }", start)
    roundtrip = source[start:end]
    assert roundtrip.count("        // start test") == 1
    roundtrip = roundtrip.replace(
        "        // start test",
        '        FastjsonUndoH2Observer.input("testEncodeAndDecode", originLog);\n        // start test',
    )
    roundtrip += "\n        FastjsonUndoH2Observer.roundtrip(bs, originLog, dstLog);"
    source = source[:start] + roundtrip + source[end:]
    start = source.index("    void testPerformance()")
    end = source.index("\n    }", start)
    performance = source[start:end]
    anchors = (
        ("        long start = System.currentTimeMillis();",
         '        FastjsonUndoH2Observer.input("testPerformance", originLog);\n'
         '        int observed = 0;\n        byte[] last = null;\n        long lastBranchId = 0;\n'
         '        long start = System.currentTimeMillis();'),
        ("            Assertions.assertEquals(originLog.getBranchId(), dstLog.getBranchId());",
         "            Assertions.assertEquals(originLog.getBranchId(), dstLog.getBranchId());\n"
         "            observed++;\n            last = bs;\n            lastBranchId = dstLog.getBranchId();"),
        ('        logger.info("elapsed time {} ms.", (end - start));',
         '        logger.info("elapsed time {} ms.", (end - start));\n'
         '        FastjsonUndoH2Observer.performance(observed, last, lastBranchId);'),
    )
    for anchor, observed in anchors:
        assert performance.count(anchor) == 1
        performance = performance.replace(anchor, observed)
    source = source[:start] + performance + source[end:]
    if inherited_binary:
        timestamp = "        Assertions.assertEquals(timestampEncode, timestampDecode);"
        assert source.count(timestamp) == 1
        source = source.replace(timestamp, timestamp + f"""
        System.out.printf("{timestamp_marker}\\t%d\\t%d\\t%s%n",
                timestampDecode.getTime(), timestampDecode.getNanos(),
                FastjsonUndoH2Observer.wireText(encode));""")

    recording_source = out / "recorded-BaseUndoLogParserTest.java"
    recording_source.write_text(source, encoding="utf-8")
    recorded = work / "recorded" / governing.name
    recorded.write_text(source, encoding="utf-8")
    recorded_subclasses = []
    if codec == "fastjson":
        original_subclass = work / "src" / Path(test).name
        subclass = original_subclass.read_text(encoding="utf-8")
        last_assertion = "        Assertions.assertTrue(value2 instanceof BigDecimal);"
        assert subclass.count(last_assertion) == 1
        subclass = subclass.replace(last_assertion, last_assertion + """
            System.out.printf("SEATA_FASTJSON_WRITE_CLASS\\t%b\\t%b\\t%b\\t%s%n",
                    s.contains("\\\"@type\\\""), value1 instanceof Long,
                    value2 instanceof BigDecimal, s);""")
        array_probes = {
            "testDirectSerialArraySerialization": (
                "        Assertions.assertArrayEquals(serialArray.getElements(), deserialized.getElements());",
                """
            System.out.printf("SEATA_FASTJSON_ARRAY_DIRECT\\t%d\\t%s\\t%s\\t%s%n",
                    deserialized.getBaseType(), deserialized.getBaseTypeName(),
                    java.util.Arrays.toString(deserialized.getElements()), json);"""),
            "testSerializeAndDeserializeSerialArray": (
                "        Assertions.assertArrayEquals(serialArray.getElements(), deserializedArray.getElements());",
                """
            System.out.printf("SEATA_FASTJSON_ARRAY_EMBEDDED\\t%b\\t%d\\t%s\\t%s\\t%s%n",
                    DataCompareUtils.isFieldEquals(field, decodedField).getResult(),
                    deserializedArray.getBaseType(), deserializedArray.getBaseTypeName(),
                    java.util.Arrays.toString(deserializedArray.getElements()), jsonString);"""),
            "testSerializeAndDeserializeSerialArrayWithNulls": (
                "        Assertions.assertEquals(3L, elements[2]);",
                """
            System.out.printf("SEATA_FASTJSON_ARRAY_NULLS\\t%b\\t%d\\t%s\\t%s%n",
                    DataCompareUtils.isFieldEquals(field, decodedField).getResult(), elements.length,
                    java.util.Arrays.toString(elements),
                    new String(bytes, java.nio.charset.StandardCharsets.UTF_8));"""),
        }
        for method, (anchor, observation) in array_probes.items():
            start = subclass.index(f"    public void {method}()")
            end = subclass.index("\n    }", start)
            body = subclass[start:end]
            assert body.count(anchor) == 1
            subclass = subclass[:start] + body.replace(anchor, anchor + observation) + subclass[end:]
        recorded_subclass = out / "recorded-FastjsonUndoLogParserTest.java"
        recorded_subclass.write_text(subclass, encoding="utf-8")
        original_name = work / "recorded" / original_subclass.name
        original_name.write_text(subclass, encoding="utf-8")
        recorded_subclasses.append(original_name)
    if jackson_arrays or jsonb:
        original_subclass = work / "src" / Path(test).name
        subclass = original_subclass.read_text(encoding="utf-8")
        array_probes = {
            "testSerializeAndDeserializeSerialArray": (
                "        Assertions.assertArrayEquals(serialArray.getElements(), deserializedArray.getElements());",
                """
            System.out.printf("SEATA_JACKSON_ARRAY_VALUES\\t%b\\t%d\\t%s\\t%s\\t%s%n",
                    DataCompareUtils.isFieldEquals(field, sameField).getResult(),
                    deserializedArray.getBaseType(), deserializedArray.getBaseTypeName(),
                    java.util.Arrays.toString(deserializedArray.getElements()),
                    new String(bytes, java.nio.charset.StandardCharsets.UTF_8));"""),
            "testSerializeAndDeserializeSerialArrayWithNulls": (
                "        Assertions.assertEquals(3L, elements[2]);",
                """
            System.out.printf("SEATA_JACKSON_ARRAY_NULLS\\t%b\\t%d\\t%s\\t%s%n",
                    DataCompareUtils.isFieldEquals(field, sameField).getResult(),
                    elements.length, java.util.Arrays.toString(elements),
                    new String(bytes, java.nio.charset.StandardCharsets.UTF_8));"""),
        }
        for method, (anchor, observation) in array_probes.items():
            start = subclass.index(f"    public void {method}()")
            end = subclass.index("\n    }", start)
            body = subclass[start:end]
            assert body.count(anchor) == 1
            subclass = subclass[:start] + body.replace(anchor, anchor + observation) + subclass[end:]
        if jsonb:
            subclass = (subclass.replace("SEATA_JACKSON_ARRAY_", "SEATA_FASTJSON2_ARRAY_")
                        .replace("isFieldEquals(field, sameField)", "isFieldEquals(field, decodedField)")
                        .replace("new String(bytes, java.nio.charset.StandardCharsets.UTF_8)",
                                 "java.util.HexFormat.of().formatHex(bytes)"))
        (out / f"recorded-{original_subclass.name}").write_text(subclass, encoding="utf-8")
        original_name = work / "recorded" / original_subclass.name
        original_name.write_text(subclass, encoding="utf-8")
        recorded_subclasses.append(original_name)
    h2_observer = HERE / "FastjsonUndoH2Observer.java"
    (out / h2_observer.name).write_bytes(h2_observer.read_bytes())
    command("compile-recorded", [javac, "-encoding", "UTF-8", "-cp", selected_cp, "-d", work / "recorded",
                                  recorded, *recorded_subclasses, h2_observer])
    raw, observed_leaves = execute("recorded-junit", os.pathsep.join((str(work / "recorded"), selected_cp)))
    assert {row["id"] for row in original_leaves} == {row["id"] for row in observed_leaves}
    java_output = raw.decode("utf-8").splitlines()
    h2_marker = "SEATA_FASTJSON_H2_INPUT\t"
    h2_input = [line.split(h2_marker, 1)[1] for line in java_output if line.startswith(h2_marker)]
    h2_path = out / "h2-source-input.tsv"
    for method in ("testEncodeAndDecode", "testPerformance"):
        observed = [line.split("\t") for line in h2_input if line.split("\t")[1] == method]
        assert len(observed) == 15
        assert [item[0] for item in observed] == ["BRANCH", *(["LOG", "IMAGE", "ROW", "ROW", "IMAGE", "ROW", "ROW"] * 2)]
    h2_path.write_text("".join(line+"\n" for line in h2_input), encoding="utf-8")
    bindings = {}
    for method, (declaring, marker) in observations.items():
        case, = [row for row in original_leaves if row["method"] == method and row["declaring_class"] == declaring]
        reference, = [line.split(marker, 1)[1] for line in java_output if line.startswith(marker)]
        bindings[method] = dict(case_id=case["id"], declaring_class=declaring, java=reference.split("\t"), rust={}, status="JAVA_CAPTURED")
    if not jackson_arrays:
        default = bindings["testDecodeDefaultContent"]["java"]
        assert default[1:] == ["true", "true", "0", "true"]
        if binary:
            assert bytes.fromhex(default[0]).hex() == default[0]
            assert default[0]
            if codec == "protostuff":
                assert default[0] == "1000"
            for name in observations:
                if name != "testDecodeDefaultContent":
                    assert bytes.fromhex(bindings[name]["java"][-1])
        else:
            assert default[0] == "{}"
    if codec == "fastjson":
        assert bindings["testWriteClassName"]["java"][:3] == ["true", "true", "true"]
    if capture_only:
        for name, value in (
            ("java-capture.json", bindings), ("commands.json", commands), ("source-inputs.json", source_inputs),
        ):
            (out / name).write_text(json.dumps(value, indent=2)+"\n", encoding="utf-8")
        index = {"status": "JAVA_CAPTURED", "members": [{"path": path.name, "sha256": sha(path)}
                                                         for path in sorted(out.iterdir())]}
        (out / "catalog.json").write_text(json.dumps(index, indent=2)+"\n", encoding="utf-8")
        print(out, sha(out / "catalog.json"), flush=True)
        return

    array_filter = "jackson_source_test_serialize_and_deserialize_serial_array"
    rust_filter = array_filter if jackson_arrays else f"{codec}_source_test_"
    rust_extra = ["--skip", array_filter] if codec == "jackson" and not jackson_arrays else []
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
        extra["SEATA_FASTJSON_H2_SOURCE"] = str(h2_path)
        command(f"{version}-cargo", [cargo, "--version"], extra)
        output = command(f"{version}-source", [cargo, "test", "--locked", "--offline", "--manifest-path",
                         ROOT / "Cargo.toml", "-p", "seata-at", "--test", ("fastjson_undo_native" if codec == "fastjson" else f"{codec}_undo_source"), rust_filter,
                         "--", "--include-ignored", "--nocapture", "--test-threads=1", *rust_extra], extra).decode("utf-8").splitlines()
        assert any(f"test result: ok. {len(observations)} passed; 0 failed" in line for line in output)
        for method, (_, marker) in observations.items():
            observed, = [line.split(marker, 1)[1].split("\t") for line in output if marker in line]
            assert observed == bindings[method]["java"]
            bindings[method]["rust"][version] = observed
    for binding in bindings.values():
        assert set(binding["rust"]) == {"1.99", "1.85"}
        binding["status"] = "MATCH"
    for label in ("deps", "selected", "recorded"):
        for artifact in (work / label).rglob("*.class"):
            (out / f"{label}-{artifact.name}").write_bytes(artifact.read_bytes())
    assertion_counts = {"fastjson": 33, "fastjson2": 27, "jackson": 18, "protostuff": 19, "kryo": 19, "fory": 18}
    original_assertions = 9 if jackson_arrays else assertion_counts[codec]
    for name, value in (
        ("commands.json", commands), ("source-inputs.json", source_inputs),
        ("case-bindings.json", list(bindings.values())),
        ("result.json", dict(java_pin=PIN, classpath_catalog_sha256=BASE_CATALOG,
                             original_java_cases=expected_cases, rust_source_cases=len(observations), original_assertions=original_assertions, performance_original_iterations=0 if jackson_arrays else 100000)),
    ):
        (out / name).write_text(json.dumps(value, indent=2)+"\n", encoding="utf-8")
    catalog = out / "catalog.json"
    catalog.write_text(json.dumps({"members": [{"path": path.name, "sha256": sha(path)}
                                               for path in sorted(out.iterdir())]}, indent=2)+"\n", encoding="utf-8")
    print(out, sha(catalog), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--codec", choices=("fastjson", "jackson", "fastjson2", "protostuff", "kryo", "fory"), default="fastjson")
    parser.add_argument("--capture-only", action="store_true")
    parser.add_argument("--jackson-arrays", action="store_true", help="bind the two original Jackson field array tests")
    args = parser.parse_args()
    if args.jackson_arrays and args.codec != "jackson":
        parser.error("--jackson-arrays requires --codec jackson")
    run(args.codec, args.capture_only, args.jackson_arrays)
