# Java handler/registration exception matrix — 2026-11-12

This increment executes the pinned, unmodified Java `handle` and registration `process` implementations. Tests isolate do-core, session, auth SPI and channel-bookkeeping dependencies. They do not claim a live authenticated TC, real storage failure/restart, AT database behavior or formal production Rust delivery.

**Actual result:** 72 new handler cases +7 registration cases passed. With all existing protocol, optionality, failure-diagnostic, exception and reload overlays, the selected target module ran **135 tests, 0 failures/errors/skips**;91-module BUILD SUCCESS in01:43. Upstream serializer39 tests are not double-counted into135. [Execution and source/report hashes](execution.json).

## Covered behavior

| Scope | Cases | Independent checks |
|---|---:|---|
| Eight ordinary handlers × normal/TransactionException/RuntimeException/StoreException | 32 | ResultCode, exact error message/code, XID/BranchId/status/lockable, one core callback, correct fallback invocation or none |
| Commit/Rollback/Status ×3 failures ×4 edge conditions | 36 | Absent session→Finished; lookup/getter failure retains previous status; callback-modified prior status is preserved; original core error remains intact |
| BranchRegister lock conflict + fail-fast | 2 | Both explicit failure codes/messages and zero BranchId |
| Successful lockable=false | 1 | Distinct from Failed+false |
| Null report failure diagnostic | 1 | Failed/null preserved; actual response encoding throws, remains OPEN |
| TM/RM auth-hook allow/false/throw | 6 | Real processor; hook invocation, allowed/no rejected channel-register calls, version bookkeeping call boundaries, captured response and actual V1 encoding/decoding |
| Denied closed RM channel | 1 | Cleanup call with no resource-registration call |

Every serializable ordinary outcome also matches a manually assembled full V1 serialized payload, independent of the Java codec. These are Java wire assertions, not a substitute for feeding every runtime output into Go/Rust. Earlier seven static failure examples and Rust81 evidence remain separate.

GlobalReport does not wrap StoreException as FailedStore: it follows the generic RuntimeException branch. Failed response status is never promoted into success. Failed lock checks are never treated as a successful false check.

Registration V1 transmits identified+version, omitting inherited ResultCode and error text. The real denied response object is Failed with a message, but the freshly decoded Java object can have default ResultCode.Success while identified remains false and message is null. Tests explicitly protect the identified flag as the authority. TM version bookkeeping can run before auth; no claim that all metadata remains untouched on rejection is made.

## Negative control and source integrity

The initial test fixture deliberately kept rollback/branch-report/lock failure injection inactive, reproducing the missing coverage paths in the previous small fixture. The new assertions failed with expected Failed vs actual Success. Raw RED source/log/XML are retained. Surefire reports two failing factory entries in that RED run; the successful final report contains all72 individual cases. This validates the test guard, not an upstream production bug fix. Removing that deliberate fixture suppression made the actual pinned handlers pass; no production source was changed.

Rust worktree base `130ad634279cef10cd2998fc3bfae82996224d66`; Java source `cba73e180fd8896a99df6415b859d275122feaa7`; existing JDK17.0.12/Maven3.9.9, outputs/temp on D. Exact executed Java sources and overlays are retained; Maven formatting changes whitespace, with Java token equality checked for all eight selected test classes. All eight canonical/supplemental source/build JSON resources match repository bytes. Original29 payload JSON and frame JSON remain unchanged from HEAD.

The updated shell entrypoint includes both new suites. Git Bash syntax check passed; the equivalent local Maven command in execution.json ran. The fresh-clone shell wrapper itself was not executed on Windows. No commit, push, branch switch, installation or SDD initialization occurred.

## Remaining work

**JAVA-V1-REPORT-NULL-001 is OPEN:** GlobalReportRequest can serialize a null status using `0xff`; a failed response keeps null, then the pinned GlobalReportResponse encoder dereferences it and throws NullPointerException. This diagnostic does not prove a usable wire failure response. A future explicit API/error policy and appropriate validation are still required.

Full Go/Rust comparison of every newly observed runtime-handler response is not complete; the broader [matrix checklist](../../java-tc-request-exception-matrix.md) retains that item unchecked. Live auth/credentials, TM and BranchRegister response loss, recovery/operator/store matrices and the three already recorded Go divergences also remain open. Phase0 and the full delivery goal are not marked complete.
