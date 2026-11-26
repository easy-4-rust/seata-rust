# Seata V1 binary payload fixtures

The 29 `.bin` files contain `uint16_be(message_type) || codec_body`, exactly as recorded in `tools/go-golden-vectors/expected.json`. These are serialized payloads, **not complete TCP frames**. They cover 25 base message types and four selected failure variants.

Supplemental [failure_variants](failure_variants/README.md) holds seven separate Go/Java pairs: four compatible, three OPEN mismatches. Those 14 files are outside the original29 corpus and never overwrite it.

The separate [handler_matrix](handler_matrix/README.md) records78 actual Java handler/auth outputs with independent normalized field expectations from a fresh135-test run. One unencodable case remains diagnostic-only; Go differences/state gaps are not counted as compatible. Original29 hashes are unchanged.

[formal_payload_oracles/manifest.json](formal_payload_oracles/manifest.json) indexes425 existing pinned Java execution records (250 exception,168 status,7 supplemental failure cases) for the formal seata-protocol tests. This is a derived index, not a new Java execution or a reclassification of Go differences. The read-only `python tools/protocol-fixtures/verify_formal_payload_oracles.py` checks original source hashes and reproduces every index byte from those records.

`manifest.json` records pinned Java/Go commits, protocol version, serializer/compressor, message codec, typed fixture input reference/body bytes, file lengths and SHA-256 values. Its source JSON hash normalizes CRLF checkout line endings to LF; runtime evidence preserves the raw input hash separately. Payload bytes are never normalized.

From repository root:

```text
python tools/go-golden-vectors/export_payload_fixtures.py
python -m unittest discover -s tools/go-golden-vectors -p test_export_payload_fixtures.py -v
```

The exporter validates the full input/destination set first and refuses to overwrite different files. Same-byte exports are repeatable, including across CRLF/LF checkouts. Generation is not runtime certification: [Java evidence](../../../../docs/phase-0/evidence/local-java-payloads-20261112/README.md), [Rust evidence](../../../../docs/phase-0/evidence/local-rust-payloads-20261112/README.md) and [validation ledger](../../../../docs/phase-0/validation-ledger.md) describe tested scope. All29 original samples also pass the formal Rust payload codec with independent semantic expectations. Its59/60 implementation also passes44 existing Java merge payloads; `python tools/protocol-fixtures/verify_formal_merge_oracles.py` checks their historical capture identity. Complete formal frame codecs now also cover the six original frames, nine controls and37 new actual Java frames in `formal_frames`; full WP-01 acceptance remains open.

The37 new frames are indexed by `formal_frames/manifest.json` and its stricter-source `certification.json`. The Java encoder and decoder ran against pinned original classes, then formal Rust checks their fields and exact bytes. `python tools/protocol-fixtures/verify_formal_frame_oracles.py` verifies both captures and the78 handler-sidecar hashes without executing Java. The current formal crate covers those78 contexts; the handler README's test-only statement describes its original evidence epoch.
