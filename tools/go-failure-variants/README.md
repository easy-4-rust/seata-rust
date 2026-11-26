# V1 failure diagnostics

`expected.json` supplements the frozen 29 payloads without changing them. Seven independent typed cases run against the exact Go snapshot: four are Java-compatible; three expose **OPEN** differences. A diagnostic pass is not an interoperability pass.

The Go golden-vector entrypoint `tools/go-golden-vectors/verify.sh` now runs this diagnostic in a separate directory after the unchanged 29 payload/6 frame checks. Java `FailureVariantCompatibilityTest` independently builds Java objects and validates normative Java bytes, expected Go decoding failures and the fail-fast ordinal semantic mismatch. Rust tests use the exported binaries and the Java authority.

```text
python tools/go-failure-variants/export_fixtures.py
python -m unittest discover -s tools/go-failure-variants -p test_export_fixtures.py -v
```

The stdlib exporter writes 14 files under `tests/fixtures/protocol/v1/failure_variants/` and a provenance/hash manifest. Java and Go bytes are kept separately even when equal. It rejects differing existing artifacts and validates case names, prefixes and compatibility classification before writing. JSON line endings are normalized only for provenance hashes; payloads are never normalized. Generation alone proves no runtime compatibility.

See [known differences](../../docs/phase-0/known-go-v1-failure-divergences.md) and [execution evidence](../../docs/phase-0/evidence/local-failure-variants-20261112/README.md). No upstream source patch is applied.
