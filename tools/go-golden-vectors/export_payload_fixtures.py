"""Export canonical payload bytes; generation alone does not certify compatibility."""
from pathlib import Path
import argparse
import hashlib
import json
import re


def export_fixtures(source: Path, destination: Path) -> dict:
    raw = source.read_bytes()
    rows = json.loads(raw)
    if not isinstance(rows, list) or len(rows) != 29:
        raise ValueError("Expected all 29 frozen payload samples")
    outputs = {}
    filenames = set()
    entries = []
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("Payload sample must be an object")
        name = row.get("name", "")
        type_id = row.get("type_id")
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", name):
            raise ValueError("Unsafe sample name")
        if type(type_id) is not int or not 1 <= type_id <= 65535:
            raise ValueError("Invalid message type")
        filename = name + ".bin"
        if filename.casefold() in filenames:
            raise ValueError("Duplicate sample name")
        filenames.add(filename.casefold())
        for key in ("body_hex", "serialized_hex"):
            value = row.get(key)
            if not isinstance(value, str) or not re.fullmatch(r"(?:[0-9a-fA-F]{2})+", value):
                raise ValueError("Invalid payload hex")
        body = bytes.fromhex(row["body_hex"])
        payload = bytes.fromhex(row["serialized_hex"])
        if payload != type_id.to_bytes(2, "big") + body:
            raise ValueError("Serialized type prefix/body mismatch")
        outputs[filename] = payload
        entries.append({"name": name, "type_id": type_id, "file": filename,
                        "fixture_input": {"case_name": name, "typed_builder": "tools/go-golden-vectors/main.go",
                                          "codec_body_hex": row["body_hex"]},
                        "payload_bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()})
    manifest = {
        "format": "seata_v1_serialized_payloads_v1",
        "java_source": "cba73e180fd8896a99df6415b859d275122feaa7",
        "go_source": "82570d905919c1c66bb025a3a5979f25390de498",
        "protocol_version": 1,
        "serializer": {"name": "SEATA", "id": 1},
        "compressor": {"name": "NONE", "id": 0},
        "codec": "Native Seata V1 message codecs; uncompressed serialized payload, no RPC frame",
        "source_json_lf_sha256": hashlib.sha256(raw.replace(b"\r\n", b"\n")).hexdigest(),
        "source_hash_normalization": "CRLF checkout newlines converted to LF; payload bytes are unchanged",
        "sample_count": len(entries), "samples": entries,
        "boundary": "Fixture export is not runtime certification; see validation ledger for evidence",
    }
    outputs["manifest.json"] = (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    # Validate every destination before creating any artifact. Never overwrite differing files.
    for name, data in outputs.items():
        path = destination / name
        if path.exists() and path.read_bytes() != data:
            raise ValueError(f"Refusing to overwrite different artifact: {path}")
    destination.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        path = destination / name
        try:
            with path.open("xb") as stream:
                stream.write(data)
        except FileExistsError:
            if path.read_bytes() != data:
                raise ValueError(f"Artifact changed during export: {path}")
    return manifest


if __name__ == "__main__":
    repo = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=repo / "tools/go-golden-vectors/expected.json")
    parser.add_argument("--destination", type=Path, default=repo / "tests/fixtures/protocol/v1")
    options = parser.parse_args()
    result = export_fixtures(options.source, options.destination)
    print(f"Exported {result['sample_count']} serialized payload fixtures to {options.destination}")
