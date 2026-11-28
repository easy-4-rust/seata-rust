"""Verify the original crash, shared upstream algorithm, fixed children and regressions."""
from pathlib import Path
import hashlib
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
ENTRIES = ("date", "time", "offset", "primitive", "offset_datetime", "utc_datetime")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def body(text, name):
    text = "\n".join(line.split("//", 1)[0] for line in text.splitlines())
    start = re.search(r"fn " + name + r"\b", text).start()
    opening = text.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return re.sub(r"\s+", "", text[start:end])


def validate(out):
    catalog = json.loads((out / "SHA256SUMS.json").read_bytes())
    assert set(catalog) == {p.relative_to(out).as_posix() for p in out.rglob("*")
                           if p.is_file() and p.name != "SHA256SUMS.json"}
    assert all(sha(out / p) == digest for p, digest in catalog.items())
    inputs = json.loads((out / "source-inputs.json").read_bytes())
    assert inputs == json.loads((out / "source-end.json").read_bytes())
    assert all(sha(ROOT / p) == digest for p, digest in inputs.items())
    current = (ROOT / "third_party/time/src/parsing/combinator/rfc/rfc2822.rs").read_text(encoding="utf-8")
    official = (out / "official-fix.rs").read_text(encoding="utf-8")
    assert all(body(current, name) == body(official, name) for name in ("cfws", "comment", "ccontent"))
    assert re.search(r"const DEPTH_LIMIT: u8 = 32;", current)
    assert re.search(r"const DEPTH_LIMIT: u8 = 32;", official)
    commands = {r["label"]: r for r in json.loads((out / "commands.json").read_bytes())}
    for name in ("lib.rs", "security_checks.rs"):
        assert (out / "original-inputs/src" / name).read_bytes() == (ROOT / "tools/time-security/src" / name).read_bytes()
    assert "[patch.crates-io]" not in (out / "original-inputs/Cargo.toml").read_text(encoding="utf-8")
    assert commands["original-boundary"]["returncode"] == 101
    for entry in ENTRIES:
        assert commands["original-" + entry]["returncode"] != 0
        stderr = (out / ("original-" + entry + ".stderr")).read_text(encoding="utf-8")
        assert "overflowed its stack" in stderr or "stack overflow" in stderr
    expected_tests = {"all_public_parsers_reject_large_nested_comments",
                      "malformed_nested_and_quoted_comments_return_errors",
                      "nesting_limit_matches_upstream_boundary",
                      "serde_rfc2822_and_option_preserve_regular_values",
                      "normal_rfc2822_values_preserve_all_entry_points",
                      "serde_rfc2822_rejects_excess_nesting",
                      "shallow_comments_and_folding_preserve_valid_inputs",
                      "quoted_parentheses_are_not_nested_comments"}
    totals = {}
    for version in ("1.85.0", "1.99.0"):
        functional = (out / ("functional-" + version + ".stdout")).read_text(encoding="utf-8")
        assert "8 passed; 0 failed; 1 ignored" in functional
        assert {line.split("::", 1)[1].split(" ... ")[0] for line in functional.splitlines()
                if line.startswith("test security_checks::") and line.endswith(" ... ok")} == expected_tests
        for entry in ENTRIES:
            label = "fixed-" + version + "-" + entry
            assert commands[label]["returncode"] == 0 and commands[label]["entry"] == entry
            assert "bounded parse error: " + entry + " depth=100000 stack=262144" in (out / (label + ".stdout")).read_text(encoding="utf-8")
        for label in ("build-", "clippy-", "rbatis-", "rbatis-clippy-", "workspace-"):
            assert commands[label + version]["returncode"] == 0
        rbatis = (out / ("rbatis-" + version + ".stdout")).read_text(encoding="utf-8")
        assert "8 passed; 0 failed; 0 ignored" in rbatis
        workspace = (out / ("workspace-" + version + ".stdout")).read_text(encoding="utf-8")
        groups = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", workspace)
        assert groups
        totals[version] = [sum(int(group[i]) for group in groups) for i in range(3)]
        assert totals[version][0] >= 853 and totals[version][1] == 0
    assert totals["1.85.0"] == totals["1.99.0"]
    normal = (out / "mysql-normal.stdout").read_text(encoding="utf-8")
    assert not any(name in normal for name in ("rbatis", "sea-orm", "time v"))
    active = (out / "active-time.stdout").read_text(encoding="utf-8")
    assert "time v0.3.45+seata.1" in active and "fastdate" in active
    original = json.loads((out / "audit-original.stdout").read_bytes())
    assert commands["audit-original"]["returncode"] == 1
    assert any(item["advisory"]["id"] == "RUSTSEC-2026-0009"
               for item in original["vulnerabilities"]["list"])
    for label in ("workspace", "rbatis"):
        audit = json.loads((out / ("audit-" + label + ".stdout")).read_bytes())
        assert commands["audit-" + label]["returncode"] == (1 if audit["vulnerabilities"]["found"] else 0)
        assert audit["settings"]["ignore"] == []
        if label == "rbatis":
            warnings = audit["warnings"]["unsound"]
            assert {"RUSTSEC-2026-0002", "RUSTSEC-2026-0253"}.issubset({item["advisory"]["id"] for item in warnings})
    report = json.loads((out / "report.json").read_bytes())
    assert report == {"source_backport_verified": True, "original_stack_crashes": 6,
                      "fixed_workers": 12, "functional_per_rust": 8, "rbatis_per_rust": 8,
                      "upstream_version_advisory_reproduced": True,
                      "raw_audit_exit_codes": {"audit-original": 1, "audit-workspace": 1, "audit-rbatis": 0},
                      "formal_rbatis_adapter": "pending",
                      "production_security_admission": "open"}
    print(json.dumps({"valid": True, "workspace_totals": totals, "input_files": len(inputs),
                      "catalog_files": len(catalog), "catalog_sha256": sha(out / "SHA256SUMS.json")}, indent=2))


if __name__ == "__main__":
    validate(Path(sys.argv[1]).resolve())
