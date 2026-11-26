# Real Java handler runtime payloads

These78 `.bin` files come from the successful pinned Java135-test run recorded by `manifest.json`'s raw log hash. Each `.expect` is a tab-separated normalized field expectation, independently specified and checked against that runtime output. `events.json` retains all79 named case events; the null-report encoding diagnostic has no fabricated payload.

Expectation columns: operation, result/identified flag, exception code, UTF-8 message hex, global status (`-1` means not a field), UTF-8 XID hex, branch ID, lockable0/1, UTF-8 version hex. Registration uses its identified flag; inherited Java ResultCode/error text are not wire fields.

Case count is not distinct-byte count: some failure/fallback/auth contexts intentionally produce identical bytes. All78 named contexts are still required. Original29 corpus and supplemental7 failure pairs remain separate and unchanged.

The local `.gitattributes` preserves exact `.bin` and `.expect` bytes across Git checkouts; expectation sidecars use canonical LF, matching their manifest hashes. Fresh capture directories are written directly with those same bytes.

[Collector/reproduction](../../../../../tools/java-handler-wires/README.md), [runtime evidence](../../../../../docs/phase-0/evidence/local-handler-runtime-20261112/README.md). Go results include62 compatible,6 length differences,1 ordinal difference and9 opaque-state cases; diagnostic success does not close those issues. Rust is test-only proof code, not a production protocol API.
