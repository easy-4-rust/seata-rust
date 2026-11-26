# Heartbeat / merge envelope validation — 2026-11-12

Final successful run: `run_20261112_174052_29ab5466`, completed at `2026-11-12T17:43:45.367410+08:00`. [Run summary](run_20261112_174052_29ab5466/summary.json), [archive manifest](manifest.json), [review](review.json), [wire contract](../../protocol-v1-control-envelopes.md), [reproduction](../../../../tools/java-control-envelopes/README.md).

| Executed check | Result | Boundary |
|---|---|---|
| Fresh selected Java reports |163 tests,13 suites, zero failures/errors/skips| Includes nine new control cases; not all upstream tests |
| Rust standalone probe |114 tests; Clippy warnings denied; formatting passed| All new models remain `cfg(test)`; no production protocol crate |
| Python capture/provenance checks |5 passed| Includes rejection of a ROOT source edit during Maven |
| Three heartbeat frames |Java/Go/Rust field and byte checks passed| PING, PONG and one nonempty HeadMap; no live socket/capability test |
| Six merge frames |Java/Rust field and byte checks passed| Original Go lacks59/60 codecs: **GO-V1-MERGE-001 remains OPEN** |

The actual pinned Go Getty reader consumes each merge frame but produces a nil body; its original registry has no matching codec and cannot generate the payload. The diagnostic exits successfully after identifying that absence. It does not certify merge interoperability. No replacement codec was registered. The original29 payload and six full-frame fixtures were not changed.

The nine new frame binaries, six serialized payload binaries, JSON/TSV records and per-case certification are under [control_envelopes](../../../../tests/fixtures/protocol/v1/control_envelopes/). Java output is captured afresh before the Rust/Go checks; both consume that same capture. Typed Rust re-encoding constructs new bytes. Negative checks cover every truncated prefix, trailing bytes, malformed counts/IDs/headers, depth and total-node limits, and single-byte mutations. Probe limits and source-derived synthetic cases are distinguished from certified cross-language samples in the wire contract.

## Provenance and freshness

Java authority is `cba73e180fd8896a99df6415b859d275122feaa7`; Go is `82570d905919c1c66bb025a3a5979f25390de498`. Before/after source audits cover2122 Java production/resource/POM files and714 original Go source/module files. The unchanged server JAR SHA-256 is `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`. No upstream production implementation or source pin was changed.

All18 Java overlay inputs are frozen **before** Maven and retained separately from the source Maven executed. Thirteen owned reports and the new test class are removed before execution. Report totals, source hashes and actual captured records are checked; a changed ROOT source during the build fails the run. The new `ProtocolV1ControlEnvelopeTest.java` input/executed bytes match exactly. Nine older overlays were reformatted by the pinned build: `AbstractTCInboundHandlerExceptionTest.java`, `FailureVariantCompatibilityTest.java`, `HandlerWireRecord.java`, `PayloadGoldenCorpusTest.java`, `PayloadOptionalityTest.java`, `ProtocolV1GoldenFrameTest.java`, `RegistrationProcessorAuthTest.java`, `SessionHolderRetryTimeoutRecoveryTest.java`, `TCInboundFailureMatrixTest.java`. Their input/executed hashes and both snapshots are retained; this archive does not claim those nine files were byte-identical.

The read-only review found a P2 input-capture timing flaw. A regression test failed before the copy-time snapshot/ROOT drift guard, then all five Python checks passed; the final full run uses the fix. The reviewer confirmed resolution without making a runtime claim.

## Earlier attempts and cleanup

Raw failed attempts are retained rather than replaced by the final run. Initial Java9 had one fixture error caused by an unset result code, fixed by constructing explicit successful result objects. The first driver attempt rejected a mistyped expected Go archive hash before Maven. A second passed Java163 but stopped at an overly broad legacy-format identity check. A third passed Java163/Go diagnostic9/Rust114 but failed the new compiler's Clippy recommendation for test hex parsing. The final run passes all gates after those fixes. Separate RED logs preserve the missing decoder and provenance-guard failures.

The two owned compiled Go executables were copied byte-for-byte to `D:/codex-builds/seata-control-binaries/<run_name>/seata-control-envelopes.exe`, hash-checked, then removed only from generated evidence directories. [Retention records](binary-retention.json) preserve paths/hashes; final binary SHA-256 is `83062949b2c7d984a8653aefa8bcaac21a6ba271f836c013788b3cf42b7f58f6`. Source, reports, captures and logs remain here. [Postcheck](postcheck.json) found no owned Java process or test listener remaining; detached HEAD stayed `130ad634279cef10cd2998fc3bfae82996224d66`.

Both updated shell wrappers separately passed syntax checks. The Java wrapper selects159 tests on Linux and adds four Windows D:/E: ownership tests on Windows, totaling163. This local run executed163 on Windows. No fresh Linux checkout or current-head GitHub Actions execution is claimed. Phase0/WP-00, formal production crates and the full Phase0–7 plan remain unfinished.
