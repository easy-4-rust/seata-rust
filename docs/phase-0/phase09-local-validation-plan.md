# Phase 0.9 local validation plan

**Goal:** verify the independent Rust tool against the pinned Java TC, without creating production TM/RM crates.

**Architecture:** retain `tools/java-tc-integration/rust-client` as an isolated std-only binary. Extend the already agreed native V1 integration plan with empty-global-transaction lifecycle checks; do not mistake those checks for AT compensation or branch callback coverage.

**Spec:** `java-tc-live-integration-plan.md`; `registration-v1-wire-contract.md`.

## Constraints

- Seata-Rust base: `130ad634279cef10cd2998fc3bfae82996224d66`.
- Java oracle: `cba73e180fd8896a99df6415b859d275122feaa7`.
- Work only in the detached local validation worktree. Preserve the original checkout's untracked documents.
- No commit, push, merge, or remote workflow change without user approval.
- Auth-disabled registration is not authentication compatibility. Empty global transactions are not MySQL AT verification.

## Tasks

- [x] Inspect live CI evidence and current repository, instead of redoing solved bootstrap work.
- [x] Run the existing Rust tests: baseline 2 passed.
- [x] Revalidate the historical two Java registration failures against the current combined harness: exact current overlays on a fresh pinned source snapshot pass all 17 selected target tests; the later historical Run #42 Java/Go jobs were also directly verified as successful. No fixtures or equality assertions were weakened. Earlier failed-run root cause is not claimed; current-head CI was not rerun. Evidence: `evidence/local-java-exact-20261112/`.
- [x] Add fail-closed wire/frame boundary tests to the standalone Rust tool, run RED, then implement the minimal change.
- [x] Add GlobalBegin/Status/Commit/Rollback for two separate no-branch transactions, retaining TC-issued XIDs and explicit result/status checks. A response timeout must not trigger automatic replay.
- [x] Reproduce and fix the TM/RM runtime client-version discrepancy with a loopback TCP regression test.
- [x] Run Rust tests (13 passed), formatting and Clippy.
- [x] Execute the new lifecycle probe against the actual pinned TC. On 2026-11-11, the isolated Maven Central build succeeded, TM/RM registration passed, and two separate empty transactions completed commit/rollback. Evidence: `evidence/local-phase09-live/`; this does not close branch callback or restart gates.
- [x] Save exact evidence, remaining blockers and local diff. Do not claim remote CI passed from local results.

## Branch callback increment — 2026-11-12

Extend the same live integration spec (INT-004 and INT-006–009) with `--branches`. Keep registration-only and `--lifecycle` behavior unchanged. Use two distinct TC-issued XIDs, one simulated AT branch per XID, the registered resource, and no database operations.

Acceptance: validate BranchRegisterResponse success and positive signed BranchId; receive real V1 synchronous BranchCommit/BranchRollback requests while TM end requests are outstanding; reject malformed envelopes or mismatched XID/BranchId/type/resource/applicationData without acknowledging; echo TC request IDs and return PhaseTwo_Committed=5 / PhaseTwo_Rollbacked=8. Query TC status afterwards and cross-check server logs for received branch responses. AT commit can respond to TM before callback completion, so that TM response alone cannot pass the test. No automatic replay after missing responses.

- [x] Add boundary tests and observe RED for missing branch callback behavior.
- [x] Implement isolated registration and concurrent RM callback handling.
- [x] Pass full Rust tests (21 passed), fmt and Clippy.
- [x] Run against the pinned built Java TC; capture both sides, hashes, exit codes and cleanup.
- [x] Update gates with precise evidence boundaries. Duplicate callbacks, loss/retry, auth, database compensation and restart recovery remain open.

Independent read-only review found one minor missing distinct-XID assertion. Its regression failed before the fix; normal loopback tests now use distinct identities and duplicate XIDs are rejected before a second BranchRegister. Reviewer recheck found no remaining findings. Final live run was repeated after this correction.

## Review focus

## INT-013/014 actual response loss — 2026-11-12

Use a separate loopback frame-forwarding proxy to discard exactly one complete **successful** BranchRegister/GlobalCommit/GlobalRollback response before the Rust process receives it. Keep the pinned TC and original channels; never replay the mutating request. Proxy bytes/BranchId are observer evidence only and must not feed the client recovery decision.

For lost BranchRegister, preserve XID/resource and an applicationData nonce created before sending; learn the positive BranchId only from a later validated TC rollback callback matching that pending context. Require one original registration, one simulated rollback action, TC completion logs and lock release using a different XID. This covers a single pending simulated branch per context, not database Undo Log or arbitrary concurrent registrations.

For lost Commit/Rollback, service the known branch callback and query authoritative state without replay. An explicit matching terminal status may reconcile the decision; Finished15 alone must retain uncertainty. Cross-check dropped TC response, callback identity and server completion independently. Use owned processes/fresh runtime data, preserve failures and stop only those processes.

- [ ] RED/GREEN for unknown-outcome retention, no replay, pending callback identity and proxy frame/drop controls.
- [ ] Execute all3 loss scenarios against the pinned TC through the external proxy.
- [ ] Verify request counts/identities, callbacks, lock occupancy/release, cleanup and full affected regression/static checks.
- [ ] Review/archive exact evidence and update gates with their actual scope.

## Runtime handler response Go/Rust comparison — 2026-11-12

Close the remaining request-exception matrix comparison item by capturing structured events from the real Java handler/processor tests after their independent field/manual-wire assertions. Require all79 named cases:78 serializable ordinary/auth outcomes and the explicitly unencodable null-report diagnostic. Export raw bytes and independent normalized expectations with source/log/case hashes, without replacing original29/supplemental7 fixtures.

Feed every serializable case to the test-only Rust model/codec and pinned unpatched Go codec. Compare independent fields and bytes. Known failed text-length/error-ordinal differences remain OPEN. If Go carries a numeric Java state without a corresponding named Go state, distinguish byte preservation from established state semantics. Reject missing/duplicate/unknown cases and conflicting output artifacts; prove semantic guards with altered expectations.

- [x] Observe that old Java logs cannot satisfy the complete structured capture gate.
- [x] Capture all79 case events from a fresh pinned Java run and export78 response fixtures.
- [x] Execute all emitted payloads through Rust and pinned Go with explicit semantic/byte classifications.
- [x] Verify collector/negative controls, review, source/report/fixture hashes and update the remaining matrix item honestly.

Result: fresh Java135/135, capture78payloads+1unencodable case, Rust83/83, Go78 native checks split62compatible/6length/1ordinal/9opaque-state. The9 Java status19 values are byte-preserved but lack a named Go state (GO-V1-STATE-001), not a new byte mismatch. Collector6tests/static checks/review passed. Separate expected-version mutation failed Go/Rust and restored checks passed. Exact evidence: `evidence/local-handler-runtime-20261112/`. Comparison is complete for this selected matrix; the known compatibility/state/null-report issues and full plan remain open.

## Complete selected Java handler exception matrix — 2026-11-12

Continue `java-tc-request-exception-matrix.md` using the pinned unmodified Java handlers. Add independent test coverage for every ordinary request handler (Begin/Commit/Rollback/Status/Report/BranchRegister/BranchReport/LockQuery): normal result, TransactionException, RuntimeException and StoreException; preserve error code/text and response-specific status/identity/lockable fields. Exercise Commit/Rollback/Status fallback for present/absent session, lookup failure and status-getter failure. Confirm lookup failures retain the response's previous status, including a handler-modified prior value. GlobalReport has no local StoreException wrapper and must remain a distinct branch.

Also verify BranchRegister lock conflict/fail-fast, successful lockable=false versus Failed+false, and the actual RegTM/RM processors' auth false/throw paths with no channel registration. Auth-hook unit tests do not substitute for a live authenticated TC or credential algorithm test. No automatic replay or production harness/TC implementation is introduced.

- [x] Add matrix tests and prove injection/expectation guards with observed failing runs.
- [x] Run actual pinned Java handler/processor tests, then combine with existing protocol/recovery suites.
- [x] Review and preserve precise evidence; update per-request gates without closing untested network/auth or cross-language cases.

Actual result: handler72 + registration processor7, combined target135/135,91-module BUILD SUCCESS. RED intentionally omitted three injection paths and failed the new assertions; no upstream production fix is claimed. Tests also preserve OPEN JAVA-V1-REPORT-NULL-001: failed GlobalReport with null status cannot be encoded by the pinned response codec. Full runtime-handler-byte Go/Rust comparison and live auth remain future gates; exact sources/reports are in `evidence/local-java-handler-matrix-20261112/`.

## Remaining transaction failure variants — 2026-11-12

Extend coverage beyond the frozen 29 samples with separate failed BranchRollback, GlobalRollback, BranchReport, GlobalStatus, GlobalReport and GlobalLockQuery cases. Keep the original expected.json and binaries unchanged. For each added case independently construct typed Go/Java messages, capture body/type-prefix bytes, assert field semantics and re-encoding, then verify Rust against Java's normative bytes. Preserve a failed result even when a status/lockable field exists.

The pinned Go BranchReport/GlobalLockQuery failure codecs use an 8-bit error-text length; Java AbstractResultMessageCodec uses a 16-bit length. First reproduce the incompatibility with runtime evidence. Record it as an open compatibility issue rather than changing Rust to guess dialects, weakening Java equality, or claiming those two samples are three-language-compatible. A separate diagnostic matrix may pass by correctly detecting the known divergence; that is not an interoperability pass. Java remains the approved wire authority.

- [x] Run unchanged Go29/frames6 on a controlled exact source snapshot.
- [x] Observe incompatible failure bytes and preserve the failing cross-language assertion.
- [x] Add six independently specified failure cases with separate Go/Java bytes and explicit compatibility status; add one fail-fast error ordinal comparison discovered in the pinned source (seven cases total).
- [x] Execute Go/Java/Rust checks; reject incompatible Go short8 failure bytes in the Java-target Rust decoder, and retain the Go19/Java20 semantic difference without remapping a valid Java NotRaftLeader.
- [x] Retain exact upstream/input/source/report hashes, review and update remaining gates.

Evidence: `evidence/local-failure-variants-20261112/`. Rust81/81, Java focused7/7 and combined56/56, Go original29/6 and native new7 passed; exporter5tests and static checks passed. Four new pairs are interoperable; GO-V1-FAIL-001–003 remain OPEN. Diagnostic completion does not resolve those compatibility requirements or close Phase 0.

## Rust all-message payload increment — 2026-11-12

Continue the existing Phase 0 all-29-payload exit condition using the frozen binary fixtures and ADR-0008. Add test-only, std-only typed codecs to the isolated probe (one Java message/model per snake_case file), with explicit type dispatch, field-order/width checks and byte-identical re-encoding. Compare decoded objects against independently specified semantic expectations; fixture bytes must not be echoed from an opaque cache. Preserve failed result/status/error text and each Java codec's zero-length null/empty distinction.

Reject unknown message/enum values, truncation at every byte boundary, trailing data, invalid UTF-8, negative signed lengths and excessive declared sizes before allocation. Bound total payload at the probe's existing 8 MiB limit; signed Java short string lengths must be representable (0..32767). This is a deliberately strict malformed-input policy: Java sometimes returns partially filled objects on truncated data, which is not accepted by this tool. Heartbeat/merge, V2, arbitrary business variants and production APIs remain separate gates.

- [x] Observe RED for missing all-message decoding, then implement explicit codecs.
- [x] Verify all 29 fixtures against independent semantic objects and re-encoding, including four failures.
- [x] Verify malformed input and relevant null/empty/signed/UTF-8 boundaries.
- [x] Run full Rust tests, fmt/Clippy, independent review and retain exact evidence.
- [x] Update ledger and remaining Phase 0 gates without counting test-only code as formal components.

Evidence: `evidence/local-rust-payloads-20261112/`. Full Rust suite 73/73 (39 new + 34 existing), fmt/Clippy exit 0. Review found two valid variants beyond the 29 examples: nullable report status and Java's retained default version on absent registration-response version. Both have RED/GREEN regressions and three independent pinned Java oracle tests (focused Java total 32/32). Canonical corpus and binaries are unchanged. Strict malformed policies are documented separately from valid nullable variants.

Final combined Java target suite: 49/49, 0 failures/errors/skips; 91-module BUILD SUCCESS. Exact formatter-produced Java sources, token equivalence to repository overlays and all four canonical-resource hashes are archived with raw reports. This is a local run, not a new remote workflow execution.

## All-message Java payload increment — 2026-11-12

Continue the existing Phase 0 exit gate for all 29 Go-certified payload samples (25 base messages plus four failures). Keep canonical expected.json unchanged; run the pinned Java V1 serializer against every sample, verify message type, independently specified semantic fields including nullable fields/failure states, and require byte-identical re-encoding. Export binary payload fixtures with source/JSON hashes and no overwrite of differing existing files. Heartbeat/merge and arbitrary message variants remain separate work.

- [x] Add executable Java corpus validation and verify the guard detects a wrong semantic expectation or altered bytes (timeout 2000→2001 failed independently of valid byte round trip).
- [x] Run all 29 against the pinned serializer module, then with the existing combined suite (target 46/46, 0 failed/error/skipped).
- [x] Add binary fixtures and provenance at `tests/fixtures/protocol/v1/`; existing Java runner includes this coverage; six exporter tests pass.
- [x] Record exact sources/results and remaining Rust conformance work; Phase 0 and the full goal remain open.

Evidence: `evidence/local-java-payloads-20261112/`. Initial TM null/empty assumption was corrected against the exact decoder; canonical JSON/bytes stayed unchanged. A stale mutated build resource was detected, diagnosed via hashes, restored and reverified. Full-corpus Rust and malformed-input coverage are the next dependency, not implied by Java/Go conformance.

## FILE crash/restart increment — 2026-11-12

Continue INT-011–012 with separate Begin-only and AT-branch scenarios. Prepare once, record TC-issued XID/BranchId/resource using a versioned create-only local record, verify Begin before stopping, forcibly stop the owned TC process, then restart the exact pinned JAR with the same FILE data and endpoint. A fresh Rust process must re-register, query the original XID as Begin, roll it back without recreating its session/branch, receive the original BranchId callback when applicable, and observe completion. For the branch scenario query the same lock using a different XID before/after cleanup to prove restoration and release.

Use dedicated directories, no destructive storage reset, no changes to Java tracked sources, no automatic replay on ambiguous sends. This validates forced-process-restart for normal FILE Begin/Registered records; retry-timeout states, storage corruption, OS/power-loss fsync guarantees and other stores remain distinct gates.

- [x] RED/GREEN for state record validation/create-only behavior and wire lock-query semantics.
- [x] Implement prepare/resume probe paths with old XID and BranchId retained.
- [x] Run fresh owned TC processes around both scenarios; record disk inventory/hashes and process identities.
- [x] Verify recovered callback/lock behavior and cleanup; run full Rust checks (34 passed, fmt/Clippy exit 0).
- [x] Review and update ledger with actual scope and evidence.

Both default-path and spaced-runtime-path full runs passed. A reviewer finding on Start-Process argument quoting was reproduced in a failing run, fixed, then rechecked with no remaining findings. Run directories and failed evidence are preserved; see `evidence/local-phase09-restart/README.md`.

## Callback response-loss increment — 2026-11-12

Continue INT-010 with `--callback-loss`: for separate commit and rollback XIDs, apply the simulated RM action, intentionally withhold its first successful response, then require a real TC redelivery on the same RM connection. Validate the full branch identity on every delivery; reply using the new RPC request ID and cached terminal branch outcome, applying the successful simulated action only once. Never replay TM end or BranchRegister. Accept a retrying global response only in this opt-in fault scenario, require a final authoritative Finished query, and cross-check TC timeout/retry/response logs. Use isolated test-only RPC timeout/retry periods; preserve previous modes.

This tests process-local simulated idempotency, not business data atomicity or durable exactly-once execution. It does not close loss of BranchRegister/TM responses, reconnect, arbitrary duplicate streams, FILE restart, authentication, or the Java combined-suite discrepancy.

- [x] Observe RED on duplicate-effect/cached-response and conflicting-identity tests.
- [x] Add process-local branch response state and bounded first-response-loss injection.
- [x] Verify full Rust suite (29 passed), fmt/Clippy and existing branch behavior.
- [x] Execute real TC commit and rollback redelivery; record RPC IDs, action counts, authoritative state and cleanup.
- [x] Review change, preserve raw evidence and update precise gates.

The first live run exposed Java's Failed/RollbackRetrying=5 initial response after the injected rollback timeout. Preserve that failed run, reproduce it in RED, and retain the failure only in the explicit loss path while querying the authoritative final status. Final live rerun passed; independent read-only review found no issues. Evidence: `evidence/local-phase09-callback-loss/`.

Check truncation, invalid wire lengths, mismatched request IDs, failed TC results carrying a status, and unexpected/pending statuses. A missing session (`Finished`) is not proof that a preceding commit succeeded.
