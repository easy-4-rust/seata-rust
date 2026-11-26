# Local native authentication and RM reconnect — 2026-11-12

Final live run: [`run_20261112_161702_b4eda7e4/result.json`](run_20261112_161702_b4eda7e4/result.json), completed2026-11-12 16:19:12 +08:00. All three profiles passed, no cleanup errors, native port28091 released. All runtime/build data remained on D:/E:.

Java authority: `cba73e180fd8896a99df6415b859d275122feaa7`; unchanged original TC JAR SHA256 `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`. The runner freshly audits2122 production source/resource/POM files before and after Maven against that pin, allowing CRLF→LF only. Test policy/source overlays are built separately on D:, then only one provider class and its SPI descriptor are loaded externally. The actual TC, processors, codecs, channel manager and FILE store are unchanged.

| Profile | Observation | Raw evidence |
|---|---|---|
| Default policy, checks enabled | Both roles identify with no credential; original default is permissive | [client](run_20261112_161702_b4eda7e4/default/client.txt), [TC](run_20261112_161702_b4eda7e4/default/tc-stdout.log) |
| Test extension, checks enabled |16 identified=false responses, same-channel transaction stimuli close, zero FILE global bytes | [denials](run_20261112_161702_b4eda7e4/extension/denied.txt), [store checkpoint](run_20261112_161702_b4eda7e4/extension/after-denial-store.json) |
| Test extension, checks disabled | Wrong credential still identifies both roles | [client](run_20261112_161702_b4eda7e4/bypass/client.txt), [TC](run_20261112_161702_b4eda7e4/bypass/tc-stdout.log) |

Denials cover missing/wrong/prefix credential, thrown checker exception, wrong app/group for both roles, plus empty/unknown/mixed/trailing RM resource lists. The client requires the identified flag, not an inherited Java ResultCode; timeout or returned transaction data cannot pass the unbound-channel check. Zero FILE bytes independently supplement the actual same-channel closure, without claiming to inspect every private channel map.

For reconnect, original RM port64479 registered two distinct XIDs/branches on `mysql://auth-a` and `mysql://auth-b`. Fresh authenticated RM port64480 registered the complete resource list. Before permitting end requests, the owned runner required the TC's removal log for the old port. New-connection callbacks retained:

| Resource | Original XID | Original BranchId | Callback |
|---|---|---|---|
| auth-a | `198.18.0.1:28091:4045076390250651649` | `4045076390250651650` | BranchCommit, RPC1, one simulated action |
| auth-b | `198.18.0.1:28091:4045076390250651651` | `4045076390250651652` | BranchRollback, RPC2, one simulated action |

[Client log](run_20261112_161702_b4eda7e4/extension/accepted-stdout.log), [structured identity proof](run_20261112_161702_b4eda7e4/extension/reconnect-proof.json), and [TC receipt/completion log](run_20261112_161702_b4eda7e4/extension/tc-stdout.log) agree. Commit status was first AsyncCommitting8, then Finished15; rollback reached Finished15. Matching successful end responses and validated callbacks precede these queries. No branch re-registration or end-request replay occurs.

Fresh focused Java7/7 and Rust103/103 pass; Rust formatting and Clippy pass. Java selection regression uses the actual enhanced loader and temporary SPI descriptor. Rust regressions reject timeout/data as denial proof, check exact credential placement, permit asynchronous convergence using only status queries, and reject opposite/begin states. The scoped read-only reviewer found no actionable issue; runtime success was independently verified by the root agent.

Final combined Java run passed150 tests across11 selected target suites, with zero failures/errors/skips and91-module BUILD SUCCESS at16:21:56 +08:00. [Combined reports](combined-reports) and `combined-production-source-audit.json` preserve the exact run and before/after production audit. This is a local selected overlay run, not a fresh CI or full upstream test-suite claim. `manifest.json` binds current harness source hashes and raw evidence; historical failed attempts remain included.

Failed attempts are retained, not counted as passes:

- `run_20261112_160442_61e39ee9`: extension priority0 lost to default100. Actual loader sorts ascending and selects the last entry. New actual-loader test failed before priority200 fixed the fixture.
- `run_20261112_161334_96952c7a`: all16 denials passed; first original callback arrived, but the probe queried Finished before asynchronous TC completion. A new query-only pending-state regression failed before bounded polling fixed the probe.

The credential `phase0-local-only` is deliberately public and fixed. This checker is a loopback test fixture. The default permissive policy is not a production authentication service, and the original successful registration INFO logs expose extraData: [JAVA-AUTH-EXTRADATA-001 and limits](../../auth-runtime-boundaries.md). Production credential transport/redaction, TLS, replay protection, arbitrary reconnect, durable RM state and real AT compensation remain open. REG-003–006 pass only for this bounded fixture scope; Phase0 and the full plan remain open.

[Runner and reproduction](../../../../tools/java-tc-integration/auth_validation/README.md).
