"""Run the isolated rbatis compatibility baseline; this does not run a database or TC."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import sys
import uuid
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
PROBE = ROOT / "tools/rbatis-compatibility"
DIRECTORIES = ("rbatis", "rbdc", "scraper", "rbatis_macro_driver", "time")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def inputs():
    files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock",
             ROOT / "docs/phase-2/rbatis-mysql-contract.md", Path(__file__).resolve(),
             ROOT / "tools/formal-source-junit/validate_rbatis_compatibility.py"]
    for base in [PROBE, *(ROOT / "third_party" / d for d in DIRECTORIES)]:
        files.extend(p for p in base.rglob("*") if p.is_file())
    assert all("__pycache__" not in p.parts for p in files)
    return {p.relative_to(ROOT).as_posix(): sha(p) for p in sorted(set(files))}


def run():
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d_%H%M%S_") + uuid.uuid4().hex[:8]
    out = ROOT / "docs/phase-2/evidence/rbatis-compatibility" / stamp
    out.mkdir(parents=True, exist_ok=False)
    env = os.environ | {"CARGO_HOME": str(ROOT.parent / ".seata-cargo-cache"),
                        "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2",
                        "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0",
                        "TEMP": "D:/Temp", "TMP": "D:/Temp", "PYTHONUTF8": "1",
                        "PYTHONDONTWRITEBYTECODE": "1", "PYTHONIOENCODING": "utf-8"}
    source = inputs()
    write(out / "source-inputs.json", source)
    write(out / "scope.json", {"database": False, "tc": False, "formal_adapter": False,
                               "production_lock_sha256": source["Cargo.lock"],
                               "versions": ["1.85.0", "1.99.0"]})
    records = []

    def command(label, argv, allowed=(0,), timeout=600):
        print(label, flush=True)
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env,
                                capture_output=True, timeout=timeout,
                                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "argv": list(map(str, argv)),
                        "returncode": result.returncode})
        write(out / "commands.json", records)
        assert result.returncode in allowed, (label, result.returncode, result.stderr[-3000:])
        return result.stdout

    command("sources", [sys.executable, "-B", PROBE / "verify_sources.py"])
    # Bind the original archive checksums to crates.io publication metadata as well.
    for directory in DIRECTORIES:
        info = json.loads((ROOT / "third_party" / directory / "SEATA_PROVENANCE.json").read_bytes())
        request = urllib.request.Request(
            "https://crates.io/api/v1/crates/" + info["crate"] + "/" + info["version"],
            headers={"User-Agent": "seata-rust-rbatis-compatibility"})
        metadata = json.load(urllib.request.urlopen(request, timeout=30))
        assert metadata["version"]["checksum"] == info["archive_sha256"]
        assert not metadata["version"]["yanked"]
        write(out / (directory + ".publication.json"), metadata)
    binaries = {}
    for version, target in [("1.85.0", ".seata-target-nodebug-185"),
                            ("1.99.0", ".seata-target-nodebug")]:
        env["CARGO_TARGET_DIR"] = str(ROOT.parent / target)
        command("rustc-" + version, ["rustc", "+" + version, "--version", "--verbose"])
        args = ["cargo", "+" + version, "test", "--manifest-path", PROBE / "Cargo.toml",
                "--offline", "--locked"]
        artifacts = command("build-" + version, [*args, "--no-run", "--message-format=json"])
        entries = [json.loads(line) for line in artifacts.splitlines() if line.startswith(b"{")]
        executable, = [Path(item["executable"]) for item in entries
                        if item.get("executable") and item["target"]["name"] == "seata_rbatis_compatibility_probe"]
        binaries[version] = {"path": str(executable), "sha256": sha(executable)}
        command("test-" + version, [executable, "--nocapture"])
        command("clippy-" + version, ["cargo", "+" + version, "clippy", "--manifest-path",
                PROBE / "Cargo.toml", "--offline", "--locked", "--all-targets", "--", "-D", "warnings"])
    write(out / "binaries.json", binaries)
    # Restore only the published macro driver: the same tests must expose its
    # direct engine/pool lookup, while the 1.85 syntax backports stay enabled.
    negative = Path("D:/codex-builds") / ("rbatis-negative-" + stamp)
    (negative / "src").mkdir(parents=True, exist_ok=False)
    manifest = (PROBE / "Cargo.toml").read_text(encoding="utf-8")
    manifest = "\n".join(line for line in manifest.splitlines()
                          if not line.startswith("rbatis-macro-driver=")) + "\n"
    manifest = manifest.replace('path="../../third_party/',
                                'path="' + ROOT.as_posix() + '/third_party/')
    (negative / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (negative / "src/lib.rs").write_bytes((PROBE / "src/lib.rs").read_bytes())
    env["CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS"] = "fallback"
    command("negative-lock", ["cargo", "+1.99.0", "generate-lockfile", "--offline",
                               "--manifest-path", negative / "Cargo.toml"])
    env["CARGO_TARGET_DIR"] = str(ROOT.parent / ".seata-target-nodebug-185")
    command("negative-macro", ["cargo", "+1.85.0", "test", "--offline", "--locked",
            "--manifest-path", negative / "Cargo.toml", "--", "--nocapture"], allowed=(101,))
    for name in ("Cargo.toml", "Cargo.lock", "src/lib.rs"):
        target = out / "negative-inputs" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((negative / name).read_bytes())
    command("active-time", ["cargo", "+1.99.0", "tree", "--manifest-path", PROBE / "Cargo.toml",
                             "--offline", "--locked", "-i", "time"])
    db = ROOT.parent / ".seata-advisory-db"
    command("advisory-pin", ["git", "-C", db, "rev-parse", "HEAD"])
    command("audit", ["cargo-audit", "audit", "--file", PROBE / "Cargo.lock", "--db", db,
                      "--no-fetch", "--json"], allowed=(0, 1))
    assert source == inputs(), "Runtime source changed during validation"
    write(out / "source-end.json", inputs())
    write(out / "report.json", {"status": "compatibility-baseline-passed", "tests_per_rust": 9,
                                "original_macro_regression": "3 expected failures",
                                "formal_adapter": "pending", "database_tc_validation": "pending",
                                "security_admission": "open"})
    catalog = {p.relative_to(out).as_posix(): sha(p) for p in sorted(out.rglob("*")) if p.is_file()}
    write(out / "SHA256SUMS.json", catalog)
    result = subprocess.run([sys.executable, "-B", str(ROOT / "tools/formal-source-junit/validate_rbatis_compatibility.py"), str(out)],
                            cwd=ROOT, env=env, check=True, capture_output=True,
                            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    print(result.stdout.decode("utf-8"), end="")
    print(out, flush=True)


if __name__ == "__main__":
    run()
