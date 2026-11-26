# Merged payload profile — 2026-11-12

Final run `run_20261112_193633_d41d73bc`, completed `2026-11-12T19:39:06.463640+08:00`. [Summary](run_20261112_193633_d41d73bc/summary.json), [manifest](manifest.json), [review](review.json), [plan](../../go-java-v1-merge-profile-plan.md), [runner/API](../../../../tools/go-java-v1-merge-profile/README.md).

| Executed check | Actual result |
|---|---|
| Fresh selected Java |475 tests,16 suites, zero failures/errors/skips |
| Rust standalone probe |117 tests, Clippy and formatting pass |
| Native codec/bytes/errors |110 top-level tests: inherited104 plus six merge/boundary tests |
| New Java merged cases |38, covering all25 ordinary types, all12 results,250 exception results, nesting, signed/optional boundaries |
| Original Java merged cases |Six reproduce missing codecs in original Go; explicit candidate passes all six |
| Explicit merged profile |All44 merged cases pass wire-field, raw-byte, pointer, caller-preservation and modeled Java-semantic checks;29 ordinary payloads also regress |
| Python checks |New seven pass; exception six, length six and control five separately pass |

This completes the **payload stage** of the explicit candidate, not the required Getty/native-frame integration stage. The original Go registry still lacks59/60 codecs; default Getty still cannot decode their bodies. GO-V1-MERGE-001 remains open for default/transport/full-client compatibility. Production Rust, WP-00 exit and the overall Phase0–7 goal are not completed.

## Boundaries and semantic fixes

The candidate uses strict per-type schemas for all25 ordinary payloads so concatenated children have exact boundaries without an invented child-length prefix. Nested59 containers accept ordinary or nested messages;60 accepts only12 ordinary result families. Optional signed32 child IDs are wholly absent or exactly one per child. Encode/decode both enforce8MiB payload,1024 children/container,2048 total nodes and depth8. Cycles, invalid counts/lengths/IDs/types/enums/UTF-8, truncated/trailing data, unsupported objects, ignored fields and native truncation/casts are rejected.

Review found that native Go reads GlobalBegin timeout as uint32 while Java uses int32. A real regression failed for−1, MIN_INT32 and positive overflow, then passed after explicit signed conversion. Four actual Java timeout samples now cover−1/MIN/0/MAX. Durations that cannot preserve exact signed-int milliseconds are rejected.

Registration-response `Version` is raw wire metadata. `RegistrationVersion(response, javaCurrent)` returns explicit `WireVersion *string` and the contextual Java getter value; absent version requires a known Java current-version context. Both Java TM/RM absent-version cases prove `getVersion()==2.8.0-SNAPSHOT` and **different Java re-encoded bytes**. Go/Rust preserve the original wire absence and expose getter meaning separately; they do not claim Java re-encoding leaves a zero version unchanged. `ReportStatus` similarly distinguishes absent255 from present0 Unknown. Direct and merged metadata tests pass. Non-wire inherited registration ResultCode/Msg fields are not result indicators.

The Rust composition test builds containers from payload fixtures whose independent typed expectations remain in the same full suite. It verifies type/order/IDs,250 nested exception values, signed timeouts and explicit optional fields; it does not replace the earlier independent ordinary/exception assertions.

## Freshness and source closure

The linked [exception stage](../local-go-java-exception-profile/run_20261112_193633_139ba3ce/summary.json) and [Java/Rust stage](../local-go-java-length-patch/run_20261112_193633_c15bc8da/summary.json) are included by file hash in this archive. Twenty-one Java overlay inputs are frozen before Maven;16 reports and the owned new-test class refresh precede execution. Input/executed sources and legacy formatter differences are separately recorded; new canonical test inputs match execution. Both29/seven Java resource oracles are copied from frozen bytes and verified after Maven.

The same Java log generates the38 new payloads,250 exception cases,24 length cases and nine original control frames. Rust receives all three fresh fixture directories, including `SEATA_IT_CONTROL_WIRE_DIR`; no default/ambient control fixture is used by this final run. Original29/six-frame and all seven previous failure regressions remain part of inherited verification.

Java source audits cover2122 original production/resource/POM files. Original Go714 source/module members remain byte-identical before/after. Final candidate audit checks all714 with only0001's two allowed changes,0002's two additions,0003's three additions, all three executed test files, three harness sources and the **complete** Go source path set. It rejects unexpected additions and changes to inherited implementations/tests, not only new0003 sources. New Python test sources and consuming helpers are frozen before imports and verified unchanged after all four test processes.

Java pin is `cba73e180fd8896a99df6415b859d275122feaa7`; Go pin is `82570d905919c1c66bb025a3a5979f25390de498`.0003 SHA-256 is `5b874eaa7865a422d8f15f433641196c764d043cbfa12b984417de5bf594ce8b`, explicitly dependent on prior0001/0002 hashes in its manifest. Original server JAR SHA-256 remains `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`. Raw fixtures/certification are under [merge_profile](../../../../tests/fixtures/protocol/v1/merge_profile/); old fixture sets were not overwritten.

## Earlier attempts, review and remaining work

The initial30-case run passed but did not exercise the later signed/optional boundaries. Its original patch/source bytes and real logs are retained. Static review identified the unsigned timeout and absent-version semantic gaps; signed RED/GREEN and eight new Java cases address them through conversion and explicit metadata representation. A first475/117/110 pass lacked fresh control-directory binding and a complete final inherited-source audit. Those two P2 gaps were fixed and statically confirmed; the final fresh run uses both guards. Earlier passing attempts are not substituted for final certification.

A separate mutation replay removes inherited profile hash checking only in memory: changing a real copied0002 file then escapes detection and the regression fails with `RuntimeError not raised`. Actual seven tests pass again with the guard; ROOT was not changed for the replay. The reviewer executed no tests and made no runtime-success claim.

No live TC or production deployment occurred. Test processes exited, fixed test listeners are gone, HEAD/JAR are unchanged ([postcheck](postcheck.json)). Updated wrappers separately pass syntax checks; current selection is471 Linux/475 Windows, but no new Linux/current-head CI run is claimed. The next required stage is real Getty profile injection/frame read-write/error propagation with default regression. The staged-freeze proposal remains pending confirmation and effective WP dependencies remain unchanged.
