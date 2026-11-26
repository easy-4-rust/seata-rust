# Explicit Go length compatibility candidate — 2026-11-12

Final run: `run_20261112_181635_276c0f53`, completed `2026-11-12T18:18:41.554144+08:00`. [Summary](run_20261112_181635_276c0f53/summary.json), [manifest](manifest.json), [review](review.json), [plan](../../go-java-v1-length-patch-plan.md), [runner](../../../../tools/go-java-v1-length-patch/README.md).

| Executed check | Actual result |
|---|---|
| Fresh Java selected tests |187 tests,14 suites, zero failures/errors/skips; includes24 new length cases |
| Original Go, same Java capture |Exactly21 named failed-response subtests fail; three named success controls pass |
| Explicitly patched Go |80 top-level codec/bytes tests pass, including all24 Java boundary subtests |
| Original29/six-frame regression in patched cohort |29 payloads and six complete frames pass |
| Independent Rust probe |115 tests pass, including typed checks of all24 fresh Java length samples; Clippy/formatting pass |
| Capture/provenance tests |Six new Python tests pass; five existing control-capture tests separately pass |

The patch changes only the type14/type22 failed-text read/write prefix from8 to16 bits and uses the existing16-bit text limit. The candidate patch SHA-256 is `9b75b422e856e91adfb422f831e0e84d99b8096e4f911875b98289f614012bf7`. Both original/patched file hashes are in its manifest. **The unchanged Go upstream remains incompatible**; GO-V1-FAIL-001/002 have a bounded candidate remedy, not an upstream resolution. GO-V1-FAIL-003, GO-V1-MERGE-001 and other open issues are not closed.

## Input and execution provenance

Java authority remains `cba73e180fd8896a99df6415b859d275122feaa7`, Go reference `82570d905919c1c66bb025a3a5979f25390de498`. Before/after audits cover2122 Java production/resource/POM files. Original JAR SHA-256 remains `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`. The Go archive is checked against its previously verified SHA-256; all714 original Go source/module files are compared before/after, and all714 cohort files are audited before/after patch execution. Only the two explicitly named codec files may differ.

All19 Java overlay input bytes were frozen before Maven. Fourteen owned reports and the new Java test class are removed before execution. ROOT source drift during Maven fails validation. Input/executed snapshots are separately retained; nine older inputs were reformatted by pinned Spotless and are honestly marked nonidentical. The new `GoErrorLengthCompatibilityTest.java` input/executed bytes match exactly. The generated24 records are the same input for original Go, patched Go and Rust. Unicode message metadata is escaped ASCII JSON, independent of the console code page.

The runner freezes its relevant source inputs, patch, manifest, Rust sources, Cargo files, shared support and golden regression harness. Short snapshot names avoid Windows MAX_PATH; `source-input-audit.json` maps each name to its original path/hash. A final drift guard checks the frozen ROOT sources. The actual new cohort Go regression test is independently archived as `go-test-executed.go` and checked before/after RED/GREEN. It cannot be omitted from the audit merely because it was absent in the original upstream archive.

The original Go RED gate checks the exact failed set and the exact success set (`Branch_Success`, `Lock_SuccessFalse`, `Lock_SuccessTrue`), not just counts. The patched gate requires all exact24 identities. A read-only reviewer found these two audit gaps in the earlier driver; both were fixed and confirmed. A separate mutation replay removed only the cohort-test drift guard in memory, causing its regression to fail with `RuntimeError not raised`; the full six Python tests then pass again with the real guard. ROOT was not modified for that RED replay.

## Cases and limits

The24 cases cover both response types with empty text, ASCII1/127/128/255/256/32767 bytes, ASCII32768 Java truncation, valid multibyte text of132 and32766 bytes, plus successful responses and lockable true/false controls. Failed result/error codes remain present after the text; a failed lock response is never treated as a successful lockability result. Empty Java decoded text is null; Go's empty string representation is explicitly handled as the same wire absence, not a claim that Go can distinguish both domain values.

These are valid Java-decodable signed-short byte lengths. Oversized Unicode, Java's UTF-16 truncation/short-overflow hazard, malformed Go decoding, other enum ordinals, missing merge codecs, peer capability negotiation and production Rust behavior remain outside this candidate. No live TC or real MySQL RM was started here. No formal Rust workspace or production delivery is claimed.

## Earlier attempts and reproducibility

The initial24 Java cases passed, but the first collector tried decoding an entire Windows GBK log as UTF-8. The raw log/executed source are retained; its actual records were decoded with verified GBK for the initial RED. Final execution uses escaped ASCII records and parses only marker lines. The first integrated run passed Java187 but rejected patch-file hashes because the system Git setting converted patched files to CRLF. Per-command line-ending overrides now yield exact LF hashes without changing configuration. A source-snapshot packaging attempt stopped before Maven at Windows MAX_PATH; final indexed short names fix that packaging problem. Earlier failed attempts remain beside the final run.

The Java and Go wrappers separately passed syntax checks. Current Java selection is183 on Linux and187 on Windows (four Windows-specific RAFT ownership tests); this run executed187 on Windows. There is no fresh Linux/current-head CI claim. Build data/caches and all independent Go source cohorts remain on D:; reports, source snapshots and fixtures are on E:. [Postcheck](postcheck.json) records owned-process/listener cleanup, unchanged HEAD and the JAR hash. [WP-00 staged-freeze proposal](../../wp00-staged-freeze-proposal.md) remains a separate pending decision and does not change this evidence.
