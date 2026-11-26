# Frozen V1 serialized payloads

The canonical 29 `.bin` files and their manifest are at the exact location required by the existing certification plan: [tests/fixtures/protocol/v1](../../../../tests/fixtures/protocol/v1/README.md). This directory is only a documentation pointer, not a second fixture source.

The canonical manifest records type, length, SHA-256, upstream commits, serializer/compressor and input provenance. Source JSON hashes normalize only checkout line endings; runtime evidence separately records raw source hashes. Payload bytes remain unchanged.

Reproduce from the repository root:

```text
python tools/go-golden-vectors/export_payload_fixtures.py
python -m unittest discover -s tools/go-golden-vectors -p test_export_payload_fixtures.py -v
```

The exporter validates all metadata/prefix/body relationships before output, preserves same-byte existing artifacts and refuses different content. Generation alone is not compatibility proof: see [validation ledger](../../validation-ledger.md) and [Java payload evidence](../../evidence/local-java-payloads-20261112/README.md). Rust full-corpus decoding/encoding remains open; the current standalone TCP probe covers only its implemented subset.
