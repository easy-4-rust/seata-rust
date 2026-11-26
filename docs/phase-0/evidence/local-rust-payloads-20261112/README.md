# Rust 29-payload conformance increment — 2026-11-12

This closes the **frozen-sample Rust payload sub-gate**, not Phase 0 or production implementation. The existing isolated std-only probe now has test-only typed codecs at `tools/java-tc-integration/rust-client/src/payload/`, one Java message/model per snake_case file. `main.rs` includes them only under `cfg(test)`. No production workspace or API was created.

- Source baseline: Rust `130ad634279cef10cd2998fc3bfae82996224d66`, Java `cba73e180fd8896a99df6415b859d275122feaa7`, Go `82570d905919c1c66bb025a3a5979f25390de498`.
- All 29 frozen binary payloads are decoded, compared with independent typed semantic expectations and encoded byte-identically. This includes four failure results with error messages/codes/statuses retained. The encoder rebuilds bytes from fields; it does not cache/echo the input.
- Full Rust result: **73 passed, 0 failed/ignored** (39 new tests plus 34 earlier tests); fmt check and all-target Clippy with warnings denied exit 0. See [execution metadata](execution.json), final logs and [source hashes](source-sha256.json).
- Malformed-input coverage includes every truncated prefix and trailing-byte case across the corpus, all unknown message IDs, complete known/unknown enum ranges, invalid UTF-8, negative/oversized/truncated lengths, signed integer/Unicode boundaries and 8 MiB limits. Exhaustive single-byte mutations must either be rejected or re-encode identically without panic; exact iteration counts are in [fixture integrity](fixture-integrity.json). This is bounded mutation testing, not general coverage-guided fuzzing.
- The canonical JSON remains unchanged from HEAD and all binaries match its serialized hex. No fixture was weakened. A valid timeout mutation 2000→2001 in a separate D-drive copy failed the independent semantic assertion; original fixtures were untouched. Both initial missing-codec RED and semantic-mutation failure logs are retained.

## Review corrections and Java semantic boundaries

Independent read-only review found two P2 gaps outside the original 29 values. Both were reproduced by failing Rust regressions, corrected, and rechecked with no remaining correctness findings:

1. `GlobalReportRequest.global_status` is optional: only legitimate encoder sentinel `0xff` becomes `None`, distinct from Unknown=0. All other unknown codes remain rejected. The regression asserts the complete expected object, exact encoding and rejection of 21..254.
2. Registration response zero-length version preserves Java's constructor default, rather than Java null. Rust stores **wire presence** in `wire_version` and models the Java value with `java_version(java_current)`, avoiding a guessed/hardcoded server default in the decoder. Present values override the default. Original absent bytes remain available for exact Rust wire re-encoding.

`PayloadOptionalityTest.java` independently executes these three cases on the pinned Java serializer. All **32 focused Java tests** passed (29 corpus + 3 boundaries), with zero failures/errors/skips. The fixed Java build's current version is `2.8.0-SNAPSHOT`. Java re-encodes a zero-length response version as explicit default version bytes; that normalization is deliberately distinguished from preserving original wire absence in Rust. Raw JUnit reports, tested Java source and Maven log are retained here.

The updated combined harness also passed: **49 selected target tests** (29 payload + 3 optionality + 6 frame + 8 exception + 3 reload), zero failures/errors/skips, 91-module BUILD SUCCESS in 02:55. Upstream serializer-module 32 tests are not double-counted into 49. See [combined execution](combined-execution.json), raw target JUnit reports and Maven log. Maven's formatter changes Java whitespace; exact executed source copies are retained and their Java token streams match repository overlays. All four source/build-resource copies of the corpus are byte-identical to canonical JSON.

Strict malformed policy differs from Java's permissive early return/UTF-8 replacement in several codecs: this probe rejects partial objects, negative lengths, noncanonical booleans and malformed UTF-8. A valid null sentinel is never reclassified as malformed. Signed scalar tests preserve Java wire values, without claiming those values are valid business transactions.

## Reproduction and limits

Run the three Cargo commands in `execution.json` from repository root, with build/temp outputs on D or E. The Java entrypoint `tools/java-v1-golden/verify.sh` now includes the optionality oracle; `combined-execution.json` records its equivalent local Maven invocation. Git Bash syntax checking passed; the fresh-clone shell entrypoint itself was not executed on Windows. Local Maven overlays use the controlled D-drive exact source snapshot. No new Go execution, remote workflow or live TC test was performed in this increment.

Formal protocol models/API, every failure/optional-field variant, heartbeat/merge, unsupported compressors/serializers, authentication, TM/BranchRegister loss reconciliation, database AT and remaining recovery/store matrices remain open. Existing Java BranchReport/GlobalLockQuery failure-message fields are short-length strings; dedicated Go/Java failure samples are still required. Earlier documentation's string8 notes do not establish their Java compatibility.

The source-generation helpers remain local build aids on D and are not reproduction prerequisites; the checked source plus frozen fixtures are sufficient. The separate mutant copy represents the pre-review semantic guard experiment, not the final source snapshot. No branch switch, commit, push, installation or SDD initialization occurred.
