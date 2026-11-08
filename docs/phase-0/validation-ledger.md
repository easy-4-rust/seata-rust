# Seata-Rust Phase 0 — Compatibility and Integration Validation Ledger

> **Current status:** Go codec/frame vectors and selected Java V1/TC unit tests have passed in GitHub Actions. Phase 0.9 Java TC FILE bootstrap harness is committed; its CI execution is pending. No live Java TC Seata request/response interoperability is claimed yet.

## 1. Frozen source baselines

| Upstream | Commit | Purpose |
|---|---|---|
| Apache Seata Java 2.x | `cba73e180fd8896a99df6415b859d275122feaa7` | V1 frame decoder/encoder, serializer and TC semantic authority |
| Apache Seata-Go v2 | `82570d905919c1c66bb025a3a5979f25390de498` | Native Seata codec and primary V1 client execution reference |
| RSeata | `c6062776c72a2490f62f8ebb25012c60884f62bd` | Third-party Rust abstraction reference only |

## 2. Protocol runtime evidence

### Run #8 — Go and Java V1 protocol tests

[GitHub Actions Run #8](https://github.com/easy-4-rust/seata-rust/actions/runs/42457053815), Seata-Rust commit `756a34a9e0198b0d6b2d6a7cafe9f404a2109804`.

- Go job passed 29 codec payload vectors and 4 complete V1 frames against Go commit `82570d9`.
- Java job passed 4 V1 Decoder → Encoder complete-frame byte-identical round trips against Java commit `cba73e1`.
- This proves the tested Go codec/serializer fixtures and four Java V1 frame cases. It does not prove all Java message codecs or a live Java TC transaction.

### Run #18 — Java TC inbound exception tests

[GitHub Actions Run #18](https://github.com/easy-4-rust/seata-rust/actions/runs/42458368199).

- 8 selected `AbstractTCInboundHandler` exception tests passed.
- Together with 4 V1 frame tests, Maven reported 12 tests, 0 failures, 0 errors, 0 skipped.
- Covered GlobalBegin TransactionException/RuntimeException/StoreException, GlobalCommit status fallback, GlobalStatus status fallback, GlobalReport request-status preservation and BranchRegister StoreException.
- Remaining GlobalRollback, BranchReport, GlobalLockQuery, RegTM/RM auth and status-fallback lookup failure cases remain open.

### Run #25 — Java retry-timeout FILE/RAFT reload unit tests

[GitHub Actions Run #25](https://github.com/easy-4-rust/seata-rust/actions/runs/42458877018).

- 3 retry-timeout unit tests passed.
- Together with the 12 earlier tests, Maven reported 15 tests, 0 failures, 0 errors, 0 skipped.
- Verified FILE pre-filter asymmetry and SessionHolder FILE/RAFT reload branches.
- This does not reproduce a full persisted FILE restart, DB/Redis restart, or live Raft leader recovery.

## 3. Fixture inventory

| Asset | Coverage | Evidence |
|---|---|---|
| `tools/go-golden-vectors/expected.json` | 29 codec-body + serialized-payload vectors (25 base messages + 4 failure responses) | Go runtime verified in Run #8 |
| `tools/go-golden-vectors/frames.json` | 6 full V1 frames: 4 begin cases plus RegisterTM and RegisterRM | Go runtime verified in Run #8 and Run #40 |
| `tools/go-golden-vectors/main.go` | Type IDs, body bytes, serializer prefix, decode equality, byte-stable re-encode and frame checks | Run #8 |
| `tools/java-v1-golden/ProtocolV1GoldenFrameTest.java` | Six Java V1 frame decode/re-encode tests, including TM/RM registration | 6/6 passed in Run #40 |
| `tools/java-v1-golden/AbstractTCInboundHandlerExceptionTest.java` | Eight selected Java TC exception/status fallback cases | 8/8 passed in Run #18 |
| `tools/java-v1-golden/SessionHolderRetryTimeoutRecoveryTest.java` | Three FILE/RAFT retry-timeout reload branch cases | 3/3 passed in Run #25 |

### Run #40 — RegisterTM/RegisterRM V1 frames added

[GitHub Actions Run #40](https://github.com/easy-4-rust/seata-rust/actions/runs/42473731588) passed after adding complete RegisterTM and RegisterRM V1 frames.

- Go: 29 payload vectors and all 6 complete V1 frames passed.
- Java: 6 V1 frame Decoder → Encoder round trips passed, including RegisterTM and RegisterRM.
- The same Java Maven run executed 8 TC exception tests and 3 FILE/RAFT reload tests: **17 tests, 0 failures, 0 errors, 0 skipped**.
- The Java RM registration request decoder maps a zero-length `extraData` field to `null`, while TM's shared IdentifyRequest decoder maps it to an empty string. The test now asserts the Java behavior explicitly; byte re-encoding remains stable.

This verifies registration payload/frame compatibility against the pinned Go and Java codecs. It is not a live RegisterTM/RegisterRM exchange with a running TC.

### Run #42 — Registration V1 frames and all current Java unit overlays

[GitHub Actions Run #42](https://github.com/easy-4-rust/seata-rust/actions/runs/42474150028) passed on the registration contract/test commit.

- Go: 29 payload vectors and 6 complete V1 frames passed, including RegisterTM and RegisterRM.
- Java: 6 V1 frame round trips passed; 8 selected TC exception tests passed; 3 FILE/RAFT reload tests passed.
- Maven result: **17 tests, 0 failures, 0 errors, 0 skipped; BUILD SUCCESS**.
- Java's RegisterRM codec decodes zero-length extraData to null, unlike the TM identify-request decoder which produces an empty string. Both re-encode to the same zero-length wire field; the fixture asserts this difference.

## 4. Phase 0.9 Java TC bootstrap harness

Harness: [tools/java-tc-integration](../../tools/java-tc-integration/README.md)

Workflow: [Phase 0.9 Java TC FILE Bootstrap](../../.github/workflows/phase-0-java-tc-file-smoke.yml)

Pinned Java source: `cba73e180fd8896a99df6415b859d275122feaa7`.

The harness:
- clones and checks out the pinned Java source;
- builds the server reactor with Maven;
- starts the server using an isolated FILE storage configuration;
- waits for the native TCP listener;
- supports stop and isolated storage reset.

**Execution status:** the harness has reached the Java ServerApplication startup path, but no successful end-to-end smoke result is recorded yet. Run #14 failed during Maven dependency resolution with a transient Maven Central HTTP 504 while resolving Kotlin compiler 2.2.20; the build script now retries Maven up to three times. The current retry-enabled bootstrap run is pending. Record its final run URL, Maven result, server process result and listener result before marking ENV-001/002 passed.

**Evidence boundary:** even a passing bootstrap workflow proves only that the pinned Java TC process starts and accepts TCP connections. It does not prove RegisterTM/RegisterRM, Seata V1 frame acceptance by the live TC, transaction lifecycle, RM callbacks or persistence recovery.

### Run #15 — Java TC FILE bootstrap passed

[GitHub Actions Run #15](https://github.com/easy-4-rust/seata-rust/actions/runs/42474316751) passed on 2026-11-08.

- Pinned Java Seata source checked out at `cba73e180fd8896a99df6415b859d275122feaa7`.
- Maven `release-seata-jar` build succeeded and produced `server/target/seata-server.jar`.
- Java ServerApplication started with the isolated FILE configuration.
- TCP listener accepted connections at `127.0.0.1:18091`.
- Smoke script stopped the TC process successfully.

This is TCP bootstrap evidence only. Run #15 did not exchange Seata V1 messages. The independent Rust registration client was added after Run #15; live RegisterTM/RegisterRM will be verified by the subsequent workflow.

## 5. Evidence ledger

| ID | Claim / gate | Evidence level | Status |
|---|---|---|---|
| SRC-001 | Java source contains V1 decoder/encoder and serializer | Verified in pinned source | Complete |
| SRC-002 | Go native codec prepends big-endian uint16 message type | Verified in pinned source | Complete |
| GO-001 | Pinned Go encoder reproduces all 29 payload vectors | Verified by test, Run #8 | Passed |
| GO-002 | Pinned Go serializer reproduces all 4 full frames | Verified by test, Run #8 | Passed |
| JAVA-001 | Java V1 decoder accepts and re-encodes the 4 fixtures byte-identically | Verified by test, Run #8 | Passed |
| TC-001 | Selected Java TC request exception mapping | Verified by test, Run #18 | Passed for covered cases |
| TC-002 | Java retry-timeout FILE/RAFT reload branches | Verified by test, Run #25 | Unit-level passed; durable recovery pending |
| ENV-001 | Pinned Java TC executable server build | Verified by test, Run #15 | Passed |
| ENV-002 | Java TC FILE startup and TCP listener | Verified by test, Run #15 | Passed |
| INT-001 | Live RegisterTM over native Seata V1 | Rust test client implemented; next live TC workflow required | Pending |
| INT-002 | Live RegisterRM over native Seata V1 | Rust test client implemented; next live TC workflow required | Pending |
| INT-003–010 | GlobalBegin, BranchRegister, Status, Commit/Rollback and RM phase-two callbacks | Live TC integration required | Not implemented |
| INT-011–014 | FILE restart, branch persistence and response-loss reconciliation | Live TC integration required | Not implemented |
| RUST-001 | Rust protocol implementation matches Java/Go | Rust implementation/test required | Not started |

| INT-CLIENT-001 | Rust standalone TM/RM registration client compiles and validates response frame | Next Phase 0.9 workflow | Pending live CI |

## 6. Phase 0 exit decision

**Protocol codec/frame sub-gate: passed.** GO-001, GO-002 and JAVA-001 have runtime evidence.

**Selected Java TC unit-test sub-gates: passed.** TC-001 covered cases and TC-002 FILE/RAFT reload unit branches have test evidence.

**Java TC bootstrap sub-gate: passed in Run #15.** The self-contained Java TC builds, starts, binds TCP 18091 and shuts down with isolated FILE storage.

**WP-00 / Phase 0 as a whole: still open.** Remaining gates include:
- Phase 0.9.1 Java TC FILE build/start smoke;
- live RegisterTM/RegisterRM and transaction lifecycle over native V1;
- RM callback handling;
- durable FILE restart and operator retry;
- remaining Java TC exception matrix cases;
- all-message Java serializer equivalence and eventual Rust codec conformance.

Do not claim full Seata-Rust V1 protocol conformance or production readiness from the current test set.

## 7. Reproduction commands

From the Seata-Rust repository root:

```bash
bash tools/go-golden-vectors/verify.sh
bash tools/java-v1-golden/verify.sh
bash tools/java-tc-integration/scripts/build-tc.sh
bash tools/java-tc-integration/scripts/smoke.sh
```

Prerequisites:
- Git and network access to GitHub;
- Go 1.23;
- JDK 17 and Maven;
- network access for Maven and Go dependencies.
