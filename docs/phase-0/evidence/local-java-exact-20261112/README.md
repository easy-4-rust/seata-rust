# Exact current Java harness verification — 2026-11-12

The earlier [combined local result](../local-java-combined-20261112/README.md) and a fresh rerun of the existing oracle both passed 17 target tests. The old oracle overlays differed from the current repository only in formatting and frame assertion diagnostics. This run removes that provenance ambiguity: export the pinned Java commit to a new D: directory and copy the **exact current** three harness files into it.

Source: `cba73e180fd8896a99df6415b859d275122feaa7`, exported with `git archive`, no branch created/switched. Test directory: `D:\codex-builds\seata-java-exact-overlay-20261112`. JDK 17.0.12, Maven 3.9.9, existing temporary Maven Central settings, E: dependency cache. Spotless was explicitly skipped so it could not rewrite copied tests.

| Target test-suite/test-new-version class | Tests | Failures | Errors | Skipped |
|---|---:|---:|---:|---:|
| ProtocolV1GoldenFrameTest | 6 | 0 | 0 | 0 |
| AbstractTCInboundHandlerExceptionTest | 8 | 0 | 0 | 0 |
| SessionHolderRetryTimeoutRecoveryTest | 3 | 0 | 0 | 0 |
| Total | 17 | 0 | 0 | 0 |

91 reactor modules completed with `BUILD SUCCESS`, exit 0, 5 minutes 5 seconds. These are selected tests, not all upstream tests. The independent serializer module's extra six frame tests are not double-counted in the target module's 17.

`execution.json` contains the exact command, pinned snapshot tar/full-log hashes, and **post-run hash equality** of all executed overlays with the current Rust repository's files. `tested-*.java` and copied raw Surefire XML/text reports preserve what was run. `build-summary.txt` captures the reactor tail; full log remains in the recorded D: directory. Fixtures and equality assertions were not weakened or modified in this goal turn.

`github-run42-jobs.json` records a direct read-only GitHub connector check on the already-existing [Run #42](https://github.com/easy-4-rust/seata-rust/actions/runs/42474150028): Java and Go jobs are completed/success. This verifies later successful historical CI state; it is not a new workflow run, a current-head CI claim, or a determination of the cause of every earlier failed run.

This closes the current local combined-harness provenance/verification gap. It does not certify all message payloads against Java, all exception branches, database/RAFT/Redis recovery, auth-enabled registration, or the full Phase 0–7 implementation.
