# Java cross-decoding of 29 Go payload samples — 2026-11-12

Java baseline `cba73e180fd8896a99df6415b859d275122feaa7`, Go baseline `82570d905919c1c66bb025a3a5979f25390de498`. Canonical `tools/go-golden-vectors/expected.json` was not changed. This is a local dirty-checkout result, not new remote CI or full Rust conformance.

## Final observed result

| Selected target test class | Tests | Failures / Errors / Skipped |
|---|---:|---|
| PayloadGoldenCorpusTest | 29 | 0 / 0 / 0 |
| ProtocolV1GoldenFrameTest | 6 | 0 / 0 / 0 |
| AbstractTCInboundHandlerExceptionTest | 8 | 0 / 0 / 0 |
| SessionHolderRetryTimeoutRecoveryTest | 3 | 0 / 0 / 0 |
| **test-suite/test-new-version total** | **46** | **0 / 0 / 0** |

The 91-module reactor completed BUILD SUCCESS, exit 0, 2 minutes 58 seconds. A focused serializer-module run also passed all 29 samples; these repeated executions are not added to the target module's 46. This is the selected harness, not all upstream tests.

For each sample the new test validates type-prefix/body consistency, decoded Java class/type, independently declared semantic values (including error/result/status and nullable fields), then requires identical re-encoded Go bytes. The sample set is 25 base types plus four failure variants, not every field value or optional message family.

The executing test file and both source/built resource copies in both participating modules were hash-checked against the current repository. See `execution.json`, `tested-PayloadGoldenCorpusTest.java`, raw `TEST-*.xml`/`.txt` reports and `combined-build-summary.txt`. Full logs remain in the recorded D: oracle directory, with final log hash recorded. JDK 17.0.12, Maven 3.9.9, existing isolated Maven Central settings were used; no upstream tracked source was altered.

## Failures that strengthened the verifier

1. `initial-optionality-failure.txt` / `initial-PayloadGoldenCorpusTest.java`: the initial test incorrectly expected null TM ExtraData. The pinned shared TM decoder sets a zero-length string to `""`; RM's override preserves `null`. Source inspection and the runtime failure agree. The expectation was corrected per codec; the fixture bytes and equality assertions were unchanged. This confirms ADR-0008 must not apply one universal zero-length rule.
2. `semantic-mutant.json` / `semantic-guard-failure.txt`: an owned temporary resource changed GlobalBegin timeout 2000→2001 in both body and serialized bytes. Its bytes were internally valid and round-trippable, but the independent semantic check failed exactly one sample (`expected 2000, actual 2001`). The canonical repository corpus was never changed.
3. `resource-cache-diagnosis.json` / `stale-resource-failure.txt`: restoring the source copy preserved its older mtime, so Maven skipped copying over the newer mutated build resource. The verifier caught 2001 again. Source hash was canonical while built-resource hash matched the mutant. Both owned resource copies were restored and hash-verified before the successful final run; post-run equality was also checked.
4. `red-export.txt` / `red-line-endings.txt`: fixture export was added after a missing-export failure, then a regression exposed Windows CRLF vs LF manifest disagreement. Only the source JSON provenance hash normalizes checkout line endings; binary bytes never change. Earlier generated manifests are archived, not silently overwritten.
5. `red-case-collision.txt`: read-only review found that case-only duplicate names could collide on Windows after the first write. A regression reproduced partial output, then normalized-name validation fixed the issue before any output. Six exporter tests now pass (`export-tests.txt`); reviewer recheck found no remaining findings.

## Reproduction and binary fixtures

The existing `tools/java-v1-golden/verify.sh` now copies this test and the canonical JSON into the fresh Java target module and selects all four classes. Shell syntax passed; in this Windows session the equivalent exact overlays/Maven selection were executed directly, not the clone-based shell entrypoint itself. No workflow was triggered or changed remotely.

```bash
bash tools/java-v1-golden/verify.sh
```

The local Maven selection was:

```text
mvn.cmd -s D:\codex-builds\seata-phase09-live-20261111\maven-central-settings.xml -B -ntp -l payload-combined-final.log -pl test-suite/test-new-version -am -Dtest=PayloadGoldenCorpusTest,ProtocolV1GoldenFrameTest,AbstractTCInboundHandlerExceptionTest,SessionHolderRetryTimeoutRecoveryTest -Dsurefire.failIfNoSpecifiedTests=false -Dcheckstyle.skip=true -Dspotless.skip=true -Djacoco.skip=true test
```

Raw binaries and manifest are at the plan's required [tests/fixtures/protocol/v1](../../../../tests/fixtures/protocol/v1/README.md). The manifest includes source commits, protocol/serializer/compressor, codec, typed builder/input reference, body hex and per-file SHA-256. Raw canonical JSON SHA-256 is `3e7f1a28c4de89c732ebc8e2eb7434a68a26d275948c8a60635641f64ea85e64`; the LF-normalized source hash is `4b0cef7e41159eff52c88e5cbc22d4dbae4cfabaaa292fc8258b1eb94d6c942d`.

The exporter refuses differing existing output and path-traversal/duplicate names; same-byte exports are repeatable on LF/CRLF checkouts. Generation is not proof of runtime compatibility; the Java/Go evidence supplies that separately.

## Remaining scope

Rust full-corpus decoding/encoding, malformed input matrix, all enum/field boundaries, optional batching/heartbeat behavior, auth, remaining TC exception/recovery tests and the full Phase 0–7 goal remain open. A successful byte round trip does not prove database AT behavior or production correctness. Existing user changes were preserved; no branch creation/switch, commit, push or new SDD initialization occurred.
