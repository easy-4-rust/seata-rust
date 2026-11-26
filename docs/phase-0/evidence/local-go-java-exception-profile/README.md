# Explicit exception profile — 2026-11-12

Final run: `run_20261112_184713_f94f053f`, completed at `2026-11-12T18:51:11.197451+08:00`. [Summary](run_20261112_184713_f94f053f/summary.json), [manifest](manifest.json), [review](review.json), [plan](../../go-java-v1-exception-profile-plan.md), [runner](../../../../tools/go-java-v1-exception-profile/README.md).

| Executed check | Result | Scope |
|---|---|---|
| Fresh selected Java |437 tests,15 suites, zero failures/errors/skips| Includes250 actual exception-code/response cases |
| Original Go on same250 records |152 match /98 divergent| Exact case identities checked; baseline unchanged |
| Go with0001 only |190 match /60 divergent| Isolates exception semantics from text-length bugs |
| Explicit0001+0002 response profile |250/250 field/byte matches| All250 pointer encodes and caller-preservation checks also pass |
| Native codec/bytes/errors packages |104 top-level tests pass| Parent99 plus exact five new profile tests |
| Original corpus and failures |14 response roundtrips;15 unsupported sample rejections; all seven Java failure semantics;24 length-wire roundtrips| Native default29 payload/six-frame regression also passes |
| Rust standalone probe |116 tests, Clippy and formatting pass| Includes typed250-case semantics and unknown-wire-code rejection |
| Python validation |New six pass; length six and control five separately pass| Frozen before imports and unchanged afterward |

The explicit API is `NewJavaV1ResponseProfile(manager)` with error-returning Encode/Decode. It preserves Go's old domain errors0..34 and default registry. Java-only meanings use separate domain values256..260; Java19 becomes NotRaftLeader,20 becomes the existing Go fail-fast,21–24 retain their Java meanings. Unmappable Go TCC/Fence/Saga extensions and unknown Java codes return errors. These new domain values require the explicit profile; they must not be cast through the old default codec.

The original Go source remains unchanged and incompatible for its recorded cases. This is a separately identified remedy candidate for the three failure-response differences, **not** an upstream release/merge or an automatic transport fix. GO-V1-MERGE-001, GO-V1-STATE-001, TC recovery problems and production Rust implementation remain separate open work.

## Provenance and reproducibility

Java authority is `cba73e180fd8896a99df6415b859d275122feaa7`; Go remains `82570d905919c1c66bb025a3a5979f25390de498`. The unchanged server JAR SHA-256 is `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`.0001 SHA-256 is `9b75b422e856e91adfb422f831e0e84d99b8096e4f911875b98289f614012bf7`;0002 is `0b4a9573982be2d3d873be0ea4951144bfad3b6390fc9bf20ce0c3f191d800f6`.

The linked [parent execution](../local-go-java-length-patch/run_20261112_184713_7f3a1ee5/summary.json) owns the fresh Java/Rust checks and0001 validation. Its full reports/logs, frozen source index and applied sources are included by hash in this archive's linked-evidence manifest. The parent summary retains the old field name `go_codec_bytes_test_functions`; its99 here counts all three packages, including19 errors tests. The package-tagged JSON records and final exact104 identity set establish the actual scope.

Twenty Java overlay inputs are frozen before Maven. Fifteen owned reports and the new length test class are removed before execution. ROOT Java drift fails validation; both input and executed source snapshots are retained. Nine older files have explicit pinned-formatter differences. The new exception and length tests match their canonical executed bytes. Both29/seven test-resource files are copied from frozen inputs before Maven and verified afterward, including the exact oracle used by the Go seven-failure regression.

Before/after audits cover2122 Java production/resource/POM files and all714 original Go source/module files. The cohort changes only the two0001 original files and adds the two0002 files; new production/test bytes are separately hashed and checked. ROOT helper, candidate, test, Rust/Cargo and consumed fixture inputs are frozen, including the seven-failure JSON. All comparisons consume the same fresh Java250 capture, with Rust reading its TSV. There is no source-derived replacement for actual Java output.

Source snapshots use indexed short names for Windows path limits. Patch application sets line-ending options only for that command; no global configuration is changed. Scoped attributes preserve patch/manifest bytes and canonical Java/Go source line endings across checkout. The generated250 binary payloads have individual hashes and per-case certification under [exception_profile](../../../../tests/fixtures/protocol/v1/exception_profile/); the original29/six fixtures are unchanged.

## Review, failures and boundaries

The initial Java fixture used the wrong setter type for GlobalBegin extraData; it failed compilation and was corrected to String. The corrected initial250 Java cases and98/60/0 codec comparison all executed. The first combined437/116 attempt then rejected its passing Go tests because its expected set omitted the errors package's19 tests. Review also found the seven-failure oracle missing from the frozen inputs. Both P2 findings were fixed and confirmed; the final fresh run uses the corrected104-test identity set and resource/source audits. Failed attempts and their raw logs remain alongside this run.

A Python mutation replay removes the exact-divergence-identity check only in memory. The swapped-case regression then fails with `RuntimeError not raised`; the unmodified six-test suite passes again. No ROOT source was changed for the replay. The reviewer performed static read-only review, not runtime execution.

Strict decoding validates complete lengths, signed text sizes, UTF-8, results, exception/status codes and lockable before calling native codecs. Tests cover all unknown exception bytes25..255 for every matrix sample, every truncated prefix and trailing data, negative lengths, invalid UTF-8, invalid branch/global states, invalid booleans, nil/non-response/zero-profile inputs, unmappable Go domains and oversized text. The profile rejects oversized input instead of reproducing Java's UTF-16 truncation/overflow hazard. Its1MiB payload cap is a candidate safeguard, not a claim about Java's frame limit.

This is a serialized ten-response payload API, not Getty transport integration, auto peer selection, complete global-state behavior, compression/V2, missing merges or production Rust. The native registry must already be initialized and stable; arbitrary concurrent/custom registry changes are outside this candidate. No TC/server was started. Java wrappers separately passed syntax checks with current433 Linux/437 Windows selection; no new Linux/current-head CI result is claimed. [Postcheck](postcheck.json) confirms owned processes/listeners are gone and HEAD/JAR unchanged. [Staged-freeze proposal](../../wp00-staged-freeze-proposal.md) remains pending confirmation and has not changed effective WP dependencies.
