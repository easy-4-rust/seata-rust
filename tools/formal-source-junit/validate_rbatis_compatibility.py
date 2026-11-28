"""Independently check the bounded rbatis compatibility evidence and its exact scope."""
from pathlib import Path
import hashlib
import json
import sys

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(out):
    catalog = json.loads((out / "SHA256SUMS.json").read_bytes())
    assert set(catalog) == {p.relative_to(out).as_posix() for p in out.rglob("*")
                            if p.is_file() and p.name != "SHA256SUMS.json"}
    assert all(sha(out / p) == digest for p, digest in catalog.items())
    source = json.loads((out / "source-inputs.json").read_bytes())
    assert source == json.loads((out / "source-end.json").read_bytes())
    assert all(sha(ROOT / p) == digest for p, digest in source.items())
    commands = {r["label"]: r for r in json.loads((out / "commands.json").read_bytes())}
    tests = {"interceptor_lookup_preserves_concrete_type",
             "native_scalar_domains_round_trip_without_float_conversion",
             "page_count_preserves_zero_exact_remainder_and_unsigned_bounds",
             "html_macro_dispatches_without_pool_and_binds_original_values",
             "native_crud_dispatches_through_supplied_executor_and_preserves_receipt",
             "option_downcast_preserves_identity_mutation_and_type_rejection",
             "static_macro_dispatches_without_pool_and_binds_original_values",
             "dynamic_sql_preserves_conditional_parameter_order"}
    for version in ("1.85.0", "1.99.0"):
        output = (out / ("test-" + version + ".stdout")).read_text(encoding="utf-8")
        assert "9 passed; 0 failed; 0 ignored" in output
        assert "test cache_checks::statement_cache_preserves_mutation_replacement_and_lru_resources ... ok" in output
        assert {line.split("::", 1)[1].split(" ... ")[0] for line in output.splitlines()
                if line.startswith("test tests::") and line.endswith(" ... ok")} == tests
        for label in ("rustc-", "build-", "test-", "clippy-"):
            assert commands[label + version]["returncode"] == 0
    report = json.loads((out / "report.json").read_bytes())
    assert report == {"status": "compatibility-baseline-passed", "tests_per_rust": 9,
                      "original_macro_regression": "3 expected failures",
                      "formal_adapter": "pending", "database_tc_validation": "pending",
                      "security_admission": "open"}
    assert commands["negative-macro"]["returncode"] == 101
    negative = (out / "negative-macro.stdout").read_text(encoding="utf-8")
    assert "6 passed; 3 failed" in negative
    assert (out / "negative-macro.stderr").read_text(encoding="utf-8").count("rbatis pool not inited") == 3
    expected_negative = (ROOT / "tools/rbatis-compatibility/src/lib.rs").read_bytes()
    assert (out / "negative-inputs/src/lib.rs").read_bytes() == expected_negative
    assert "rbatis-macro-driver=" not in (out / "negative-inputs/Cargo.toml").read_text(encoding="utf-8")
    for directory in ("rbatis", "rbdc", "scraper", "rbatis_macro_driver", "time"):
        info = json.loads((ROOT / "third_party" / directory / "SEATA_PROVENANCE.json").read_bytes())
        publication = json.loads((out / (directory + ".publication.json")).read_bytes())["version"]
        assert publication["checksum"] == info["archive_sha256"] and not publication["yanked"]
    audit = json.loads((out / "audit.stdout").read_bytes())
    assert commands["audit"]["returncode"] == (1 if audit["vulnerabilities"]["found"] else 0)
    assert "time v" in (out / "active-time.stdout").read_text(encoding="utf-8")
    print(json.dumps({"valid": True, "scope": "dependency and macro compatibility only",
                      "source_files": len(source), "catalog_files": len(catalog),
                      "catalog_sha256": sha(out / "SHA256SUMS.json")}, indent=2))


if __name__ == "__main__":
    validate(Path(sys.argv[1]).resolve())
