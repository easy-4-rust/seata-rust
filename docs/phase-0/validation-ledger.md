# Phase 0.8 — Compatibility Validation Ledger

 > **Status: Go and Java V1 protocol jobs passed in GitHub Actions Run #8.** This ledger distinguishes pinned-source facts, static fixture validation, and runtime test results. The Java TC recovery and live Rust↔Java TC integration gates remain open.

## 1. Frozen source baselines

| Upstream | Commit | Purpose |
|---|---|---|
| Apache Seata Java 2.x | `cba73e180fd8896a99df6415b859d275122feaa7` | V1 frame decoder/encoder, serializer and TC semantic authority |
| Apache Seata-Go v2 | `82570d905919c1c66bb025a3a5979f25390de498` | Native Seata codec and primary V1 client execution reference |
| RSeata | `c6062776c72a2490f62f8ebb25012c60884f62bd` | Third-party Rust abstraction reference only |

## 2. Baseline defect corrected in Phase 0.8

The previous `tools/go-golden-vectors/verify.sh` referenced Go commit `82970d905919c1c66bb025a3a5979f25390de498`, while all Phase 0 source documentation declared `82570d905919c1c66bb025a3a5979f25390de498`.

The script now checks out the exact documented commit:

```bash
GO_SHA="82570d905919c1c66bb025a3a5979f25390de498"
```

This is a provenance correction. It does not certify the vectors; only an actual run can do that.

## 3. Executable test assets

| Asset | Coverage | Runtime status |
|---|---|---|
| `tools/go-golden-vectors/expected.json` | 29 codec body + serialized payload fixtures (25 base messages + 4 failure responses) | Candidate; Go execution pending |
| `tools/go-golden-vectors/frames.json` | 4 complete V1 frames (begin request, begin response, optional head map, failed begin response) | Candidate; Go execution pending |
| `tools/go-golden-vectors/main.go` | Constructs typed Go messages; checks type IDs, codec body, serializer prefix, decode equality, re-encode stability and complete frame bytes | Not executed in this session |
| `tools/java-v1-golden/ProtocolV1GoldenFrameTest.java` | Four Java V1 decode/re-encode frame tests, including optional head map and zero-length field semantics | Not executed in this session |

## 4. GitHub Actions automation

Workflow: [Phase 0 Protocol Compatibility](../../.github/workflows/phase-0-protocol-compatibility.yml)

Triggers:
- push to main when protocol fixtures, scripts, workflow or Phase 0 docs change;
- pull requests touching the same paths;
- manual workflow dispatch.

Jobs:
1. `go-golden-vectors`: Go 1.23; clones and checks out the pinned Go commit; runs `bash tools/go-golden-vectors/verify.sh`.
2. `java-v1-frames`: Temurin JDK 17; clones the pinned Java commit; overlays the JUnit test; runs `bash tools/java-v1-golden/verify.sh`.

**Verified by test:** [GitHub Actions Run #8](https://github.com/easy-4-rust/seata-rust/actions/runs/42457053815) completed successfully for Seata-Rust commit `756a34a9e0198b0d6b2d6a7cafe9f404a2109804` on 2026-11-08.

- `go-golden-vectors`: success; 29 payload vectors and 4 complete frames passed against Go commit `82570d905919c1c66bb025a3a5979f25390de498`.
- `java-v1-frames`: success; 4 Java V1 decoder/encoder round-trip tests passed against Java commit `cba73e180fd8896a99df6415b859d275122feaa7`; Maven reactor build succeeded.

Earlier failed runs were caused by harness setup issues and one incorrect test expectation. Those were corrected before Run #8. Run #8 is the authoritative passing result.

## 5. Evidence ledger

| ID | Claim / gate | Evidence level | Status |
|---|---|---|---|
| SRC-001 | Java source contains V1 decoder/encoder and serializer | Verified in pinned source | Complete |
| SRC-002 | Go native codec prepends big-endian uint16 message type | Verified in pinned source | Complete |
| FIX-001 | 29 fixture serialized prefixes match their declared type IDs | Static fixture check | Complete; not runtime proof |
| FIX-002 | Four frame fixtures have consistent full/head lengths | Static fixture check | Complete; not runtime proof |
| GO-001 | Pinned Go encoder reproduces all 29 codec vectors | Verified by test in Run #8 | Passed |
| GO-002 | Pinned Go serializer reproduces all four full frames | Verified by test in Run #8 | Passed |
| JAVA-001 | Java V1 decoder accepts all four frames and Java encoder re-emits identical bytes | Verified by test in Run #8; 4 tests, 0 failures | Passed |
| TC-001 | Java TC request exception mapping | Source audit | Complete; runtime tests pending |
| TC-002 | Java retry-timeout storage recovery | Source audit / inferred storage impact | Runtime reproduction pending |
| RUST-001 | Rust protocol implementation matches Java/Go | Not implemented | Not started |

## 6. Current WP-00 exit decision

**Protocol codec/frame sub-gate: passed.** GO-001, GO-002 and JAVA-001 are now runtime-verified.

**WP-00 as a whole remains open.** It is still blocked on Java TC recovery tests REC-TIMEOUT-001–010, Java request exception runtime tests, and a live Java TC integration test that exercises actual registration and transaction execution.

Do not begin claiming V1 protocol conformance or mark source-derived candidate vectors as certified golden vectors until the CI jobs pass.

## 7. Commands for local reproduction

From the Seata-Rust repository root:

```bash
bash tools/go-golden-vectors/verify.sh
bash tools/java-v1-golden/verify.sh
```

Required local prerequisites:
- Git and network access to GitHub;
- Go 1.23 (Go module declares Go 1.20);
- JDK 17+ and Maven;
- network access for Maven and Go dependencies.

