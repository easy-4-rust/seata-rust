"""Verify every maintained dependency file against its published-source ledger."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[2]
DIRECTORIES = ("rbatis", "rbdc", "scraper", "rbatis_macro_driver")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify():
    result = {}
    for directory in DIRECTORIES:
        base = ROOT / "third_party" / directory
        manifest = json.loads((base / "SEATA_PROVENANCE.json").read_bytes())
        expected = manifest["files"]
        actual = {
            p.relative_to(base).as_posix(): sha(p)
            for p in base.rglob("*") if p.is_file()
            and p.name not in ("SEATA_PROVENANCE.json", "SEATA.patch")
        }
        assert actual == {p: entry["sha256"] for p, entry in expected.items()}, directory
        changed = [p for p, entry in expected.items()
                   if entry["sha256"] != entry["upstream_sha256"]]
        assert len(changed) == {"rbatis": 5, "rbdc": 3, "scraper": 2,
                                "rbatis_macro_driver": 4}[directory], changed
        result[directory] = {"crate": manifest["crate"], "version": manifest["version"],
                             "files": len(actual), "changed": changed,
                             "archive_sha256": manifest["archive_sha256"]}
    return result


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
