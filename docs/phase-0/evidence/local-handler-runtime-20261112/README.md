# Actual handler outputs → Rust / pinned Go — 2026-11-12

This completes the comparison of **all serialized outputs from the selected Java handler/registration matrix**, not full protocol/production delivery. A fresh pinned Java run passed135 target tests; structured events were emitted only after independent field/manual-wire assertions. Capture requires all79 exact case identities:78 actual serialized payloads and one separately recorded unencodable null-report case. No response bytes are fabricated for that case.

| Verification | Observed result |
|---|---|
| Java selected target | 135/135,0 failures/errors/skips;91-module BUILD SUCCESS in02:06 |
| Capture | 79 complete named events,78 `.bin`/`.expect` pairs; raw source-log hash and per-file hashes retained |
| Collector checks | 6 passed after missing-collector RED; old successful Java log without all79 events correctly rejected |
| Rust | 83/83 full tests,0 failed/ignored; every78 output matches independent normalized fields and exact bytes; all new prefixes/trailing bytes rejected |
| Pinned Go native checks | 78 typed native roundtrips, unmodified codecs |
| Go comparison | 62 compatible covered fields/bytes;6 OPEN length cases;1 OPEN ordinal case;9 opaque-state cases |
| Negative controls | Expected server version2.8→3.8 in a separate D copy with payload unchanged: Rust exit101,Go exit1; original fixtures unchanged and restored-path checks passed |
| Static/review | Rust fmt/Clippy deny warnings,Go vet,both shell syntax checks passed; independent read-only review found no actionable issue |

The6 length cases are actual transaction/runtime/store failure outputs for BranchReport and LockQuery, corresponding to existing GO-V1-FAIL-001/002. The one fail-fast case corresponds to GO-V1-FAIL-003. Diagnostic green means expected differences were detected, not repaired. No captured Go error text exceeded127 bytes in this run; long-text behavior is not inferred from that result.

For9 callback-modified status cases, Java global status19 is preserved by Go as a raw number and in byte re-encoding. The pinned `message.GlobalStatus` declarations end at15. **GO-V1-STATE-001 is a state-semantics evidence gap, not a newly observed byte mismatch**. Do not infer either full state-machine support or universal Go-client failure from raw preservation. [Known differences and boundaries](../../known-go-v1-failure-divergences.md).

## Sources, artifacts and guards

- Rust base `130ad634279cef10cd2998fc3bfae82996224d66`; Java `cba73e180fd8896a99df6415b859d275122feaa7`; Go `82570d905919c1c66bb025a3a5979f25390de498`.
- [Execution metadata](execution.json) contains exact commands, selected reports, source hashes, observed counts and boundaries. All eight selected JUnit XML reports are included; upstream serializer39 tests are not double-counted into135.
- [Capture manifest](capture-manifest.json) binds the raw Java log to all78 binary/expectation pairs. [Events](capture-events.json) preserve all79 named contexts. Context count is not distinct-byte count; repeated byte values from different error/auth/fallback branches remain separately required.
- Exact executed Java marker helper/test sources are retained; Maven formatting changes whitespace and import ordering, and the final repository overlays' token streams match the tested copies. Go probe source matches its executed copy; source/module files were checked unchanged against the commit-pinned archive, including state/error declarations and actual codec implementations.
- [Current source hashes](source-sha256.json) cover collector/tests, Go probe, Rust tests, marker helper/additions and wrappers. Original29 JSON/frame inputs and all8 source/build resource copies remain byte-identical to the expected repository inputs.

The collector has a finite independent specification and Java layout oracle; it rejects missing/duplicate/unknown cases, schema/type/field/body alterations, failed builds and differing existing artifacts before writing. Its synthetic unit events test the collector only; actual runtime evidence is the successful Java log, tested-source/report integrity and captured outputs. Export alone is never certification.

`.expect` columns describe normalized wire fields. Registration's result column is identified, not Java's inherited ResultCode; nonserialized Java message fields are deliberately absent. Existing separate typed optionality tests establish Java null/default distinctions beyond those normalized wire fields.

## Reproduction and remaining scope

[Collector and full fresh-run wrapper](../../../../tools/java-handler-wires/README.md) describe reproduction. Existing Java verification now checks complete capture after Maven; the new wrapper passes the same fresh directory into Go/Rust. Both wrappers are syntax checked and read-only reviewed. This Windows increment executed equivalent individual Java/collector/Go/Rust commands using D/E output/temp directories; the fresh-clone Bash wrapper itself was not executed. No remote workflow run/change or SDK installation is claimed.

JAVA-V1-REPORT-NULL-001 remains unencodable, the three Go failure issues remain OPEN, and the status19 business-state gap remains OPEN. Live authenticated TC/credentials, INT-013/014 response loss, remaining recovery/operator/store matrices, merge/heartbeat and formal production phases are still required by the unchanged full objective. These are serialized payloads rather than complete TCP exchanges. No production source patch, branch switch, commit, push or SDD initialization occurred.
