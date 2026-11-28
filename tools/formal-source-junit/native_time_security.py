"""Reproduce RUSTSEC-2026-0009 in isolated children and verify the source backport."""
from pathlib import Path
import datetime
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
PROBE = ROOT / "tools/time-security"
ENTRIES = ("date", "time", "offset", "primitive", "offset_datetime", "utc_datetime")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def inputs():
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", Path(__file__).resolve(),
             ROOT / "tools/formal-source-junit/validate_time_security.py",
             ROOT / "docs/phase-2/time-rfc2822-security-contract.md"]
    for base in [ROOT / "crates", ROOT / "third_party", PROBE,
                 ROOT / "tools/rbatis-compatibility"]:
        paths.extend(p for p in base.rglob("*") if p.is_file())
    assert all("__pycache__" not in p.parts for p in paths)
    return {p.relative_to(ROOT).as_posix(): sha(p) for p in sorted(set(paths))}


def run():
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d_%H%M%S_") + uuid.uuid4().hex[:8]
    out = ROOT / "docs/phase-2/evidence/time-rfc2822" / stamp
    out.mkdir(parents=True, exist_ok=False)
    work = Path("D:/codex-builds") / ("time-rfc2822-" + stamp)
    work.mkdir(parents=True, exist_ok=False)
    env = os.environ | {"CARGO_HOME": str(ROOT.parent / ".seata-cargo-cache"),
                        "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2",
                        "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0",
                        "TEMP": "D:/Temp", "TMP": "D:/Temp", "PYTHONUTF8": "1",
                        "PYTHONDONTWRITEBYTECODE": "1", "PYTHONIOENCODING": "utf-8"}
    frozen = inputs()
    write(out / "source-inputs.json", frozen)
    commands = []

    def command(label, argv, expected=(0,), timeout=600):
        print(label, flush=True)
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, env=env,
                                capture_output=True, timeout=timeout,
                                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        commands.append({"label": label, "argv": list(map(str, argv)),
                         "entry": env.get("SEATA_TIME_ENTRY"), "returncode": result.returncode})
        write(out / "commands.json", commands)
        if expected == "stack-crash":
            assert result.returncode != 0 and (b"stack overflow" in result.stderr
                   or b"overflowed its stack" in result.stderr), (label, result.stderr)
        else:
            assert result.returncode in expected, (label, result.returncode, result.stderr[-3000:])
        return result.stdout

    command("sources", [sys.executable, "-B", ROOT / "tools/rbatis-compatibility/verify_sources.py"])
    provenance = json.loads((ROOT / "third_party/time/SEATA_PROVENANCE.json").read_bytes())
    for version in ("0.3.45", "0.3.47"):
        request = urllib.request.Request("https://crates.io/api/v1/crates/time/" + version,
                    headers={"User-Agent": "seata-rust-time-security-validation"})
        metadata = json.load(urllib.request.urlopen(request, timeout=30))
        write(out / ("time-" + version + ".publication.json"), metadata)
        expected = provenance["archive_sha256" if version == "0.3.45" else "fix_source_archive_sha256"]
        assert metadata["version"]["checksum"] == expected and not metadata["version"]["yanked"]
        if version == "0.3.47":
            request = urllib.request.Request("https://crates.io/api/v1/crates/time/0.3.47/download",
                        headers={"User-Agent": "seata-rust-time-security-validation"})
            archive = urllib.request.urlopen(request, timeout=30).read()
            assert hashlib.sha256(archive).hexdigest() == expected
            with tarfile.open(fileobj=io.BytesIO(archive)) as tf:
                rel = "src/parsing/combinator/rfc/rfc2822.rs"
                (out / "official-fix.rs").write_bytes(tf.extractfile("time-0.3.47/" + rel).read())

    # Counterfactual uses exactly the current tests, with only the patch removed.
    negative = work / "original"
    (negative / "src").mkdir(parents=True)
    manifest = (PROBE / "Cargo.toml").read_text(encoding="utf-8").split("[patch.crates-io]")[0]
    (negative / "Cargo.toml").write_text(manifest, encoding="utf-8")
    for p in (PROBE / "src").glob("*.rs"):
        (negative / "src" / p.name).write_bytes(p.read_bytes())
    env["CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS"] = "fallback"
    command("original-lock", ["cargo", "+1.99.0", "generate-lockfile", "--offline",
                                "--manifest-path", negative / "Cargo.toml"])
    env["CARGO_TARGET_DIR"] = str(ROOT.parent / ".seata-target-nodebug-185")

    def build(label, version, manifest):
        raw = command(label, ["cargo", "+" + version, "test", "--offline", "--locked",
                      "--manifest-path", manifest, "--no-run", "--message-format=json"])
        artifacts = [json.loads(line) for line in raw.splitlines() if line.startswith(b"{")]
        binary, = [Path(r["executable"]) for r in artifacts if r.get("executable")
                   and r["target"]["name"] == "seata_time_security_probe"]
        return binary

    binary = build("original-build", "1.85.0", negative / "Cargo.toml")
    binaries = {"original-1.85.0": {"path": str(binary), "sha256": sha(binary)}}
    for entry in ENTRIES:
        env["SEATA_TIME_ENTRY"] = entry
        command("original-" + entry, [binary, "--exact", "security_checks::attack_worker",
                "--ignored", "--nocapture"], expected="stack-crash", timeout=15)
    command("original-boundary", [binary, "--exact", "security_checks::nesting_limit_matches_upstream_boundary",
                                   "--nocapture"], expected=(101,))
    for name in ("Cargo.toml", "Cargo.lock", "src/lib.rs", "src/security_checks.rs"):
        target = out / "original-inputs" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((negative / name).read_bytes())

    for version, target in [("1.85.0", ".seata-target-nodebug-185"),
                            ("1.99.0", ".seata-target-nodebug")]:
        env["CARGO_TARGET_DIR"] = str(ROOT.parent / target)
        command("rustc-" + version, ["rustc", "+" + version, "--version", "--verbose"])
        binary = build("build-" + version, version, PROBE / "Cargo.toml")
        binaries[version] = {"path": str(binary), "sha256": sha(binary)}
        command("functional-" + version, [binary, "--nocapture"])
        for entry in ENTRIES:
            env["SEATA_TIME_ENTRY"] = entry
            command("fixed-" + version + "-" + entry,
                    [binary, "--exact", "security_checks::attack_worker", "--ignored", "--nocapture"], timeout=15)
        command("clippy-" + version, ["cargo", "+" + version, "clippy", "--offline", "--locked",
                "--manifest-path", PROBE / "Cargo.toml", "--all-targets", "--", "-D", "warnings"])
        command("rbatis-" + version, ["cargo", "+" + version, "test", "--offline", "--locked",
                "--manifest-path", ROOT / "tools/rbatis-compatibility/Cargo.toml"])
        command("rbatis-clippy-" + version, ["cargo", "+" + version, "clippy", "--offline", "--locked",
                "--manifest-path", ROOT / "tools/rbatis-compatibility/Cargo.toml", "--all-targets", "--", "-D", "warnings"])
        command("workspace-" + version, ["cargo", "+" + version, "test", "--offline", "--locked",
                "--workspace", "--all-features"], timeout=900)
    write(out / "binaries.json", binaries)
    command("mysql-normal", ["cargo", "+1.99.0", "tree", "--offline", "--locked", "-p",
                              "seata-db-mysql", "--all-features", "--edges", "normal"])
    command("active-time", ["cargo", "+1.99.0", "tree", "--offline", "--locked", "--manifest-path",
                             ROOT / "tools/rbatis-compatibility/Cargo.toml", "-i", "time"])
    db = ROOT.parent / ".seata-advisory-db"
    command("advisory-pin", ["git", "-C", db, "rev-parse", "HEAD"])
    command("audit-original", ["cargo-audit", "audit", "--file", out / "original-inputs/Cargo.lock",
                                "--db", db, "--no-fetch", "--json"], expected=(1,))
    for label, lock in [("workspace", ROOT / "Cargo.lock"),
                        ("rbatis", ROOT / "tools/rbatis-compatibility/Cargo.lock")]:
        command("audit-" + label, ["cargo-audit", "audit", "--file", lock, "--db", db,
                                   "--no-fetch", "--json"], expected=(0, 1))
    assert inputs() == frozen, "Frozen runtime input changed"
    write(out / "source-end.json", inputs())
    write(out / "report.json", {"source_backport_verified": True,
          "original_stack_crashes": 6, "fixed_workers": 12, "functional_per_rust": 8,
          "rbatis_per_rust": 8, "upstream_version_advisory_reproduced": True,
          "raw_audit_exit_codes": {r["label"]: r["returncode"] for r in commands if r["label"].startswith("audit-")},
          "formal_rbatis_adapter": "pending", "production_security_admission": "open"})
    write(out / "SHA256SUMS.json", {p.relative_to(out).as_posix(): sha(p)
                                    for p in sorted(out.rglob("*")) if p.is_file()})
    command_result = subprocess.run([sys.executable, "-B", str(ROOT / "tools/formal-source-junit/validate_time_security.py"), str(out)],
                          cwd=ROOT, env=env, capture_output=True, check=True,
                          creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    print(command_result.stdout.decode("utf-8"), end="")
    print(out, flush=True)


if __name__ == "__main__":
    run()
