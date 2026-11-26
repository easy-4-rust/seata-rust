# Live original-TC RAFT retry-timeout diagnostics — 2026-11-12

Final run [`run_20261112_165835_c293c228/result.json`](run_20261112_165835_c293c228/result.json) completed17:01:49 +08:00. All three diagnostic cohorts completed, with six real leader transitions, three old-node snapshot reloads and12 owned JVM instances. Cleanup has no errors; all nine reserved ports were released. Development/runtime data remained on D:/E:.

This is a **diagnostic result, not a safe recovery pass**. Both retry-timeout states trigger an original asynchronous leader-reload exception while their records and locks remain available. [Open findings and implications](../../raft-reload-runtime-findings.md).

Java pin: `cba73e180fd8896a99df6415b859d275122feaa7`. Original executable JAR SHA256: `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`. Fresh helper tests audit2122 production source/resource/POM files before and after Maven, allowing CRLF→LF only. Only one external test-control class is added to the original TC loader; all session, lock, consensus, snapshot and recovery implementations execute unchanged. The separate JDK recording reader is not loaded into the TC.

| Cohort | Original records | Outcome after both elections and old-node restart | Exact reload exceptions |
|---|---|---|---:|
| Begin1 | One AT branch plus one branch-empty transaction | Branch-bearing original becomes RollbackRetrying5 with its lock; empty original disappears/queries Finished15 |0|
| CommitRetryTimeout16 | Same profiles | Both retain16; original branch/resource/lock key and held lock remain |2|
| RollbackRetryTimeout17 | Same profiles | Both retain17; original branch/resource/lock key and held lock remain |2|

Original identities are `192.168.1.10:28091:940000` and `...:940001`; the branch is950000 on `mysql://raft-retry-0`, lock key `orders:1`. States are deliberately injected through original `GlobalSession.begin/addBranch/changeGlobalStatus` and actual JRaft tasks. They are not native TC-issued business transactions or naturally expired retries. No private map, snapshot bytes or RocksDB record is edited.

Each cohort starts three members, requires caught-up committed/applied indices and original records on every replica, and snapshots each real member. It stops the owned leader, verifies another node and higher term, checks actual console and independent Rust native V1 status queries, and inspects JFR. The old node restarts with a new PID from the same store; original session-snapshot loading and catch-up are required. A second leader stop/election repeats the state/query/exception checks. The independent verifier checks39 replica record sets and18 native original-XID queries, plus matching console responses.

Useful raw files under the final run:

- `status_*/seeded-node_*.json`, `snapshotted-node_*.json`, `election_one-node_*.json`, `restarted-node_*.json`, `election_two-node_*.json`: original state/identity, real leader/term/indices and independent lock observations.
- `status_*/election_*-reload-exceptions.json`: pre-stop UTC boundary, dump attempt, exact class/message and original async reload stack. Each matched timeout event is after its own transition boundary.
- `status_*/*-console-*.json`, `*-native.txt`: actual original console and TM-only native V1 queries. The Rust mode sends one GlobalStatus per original XID and no mutation or RM registration.
- `node_*_generation_*/tc-*.log`: original server logs, including snapshot load, reload error and completion/control behavior.
- `snapshots/` and `snapshot-inventory.json`: byte-preserved actual member snapshot files and source paths/hashes. Original D: RocksDB/log directories are retained as well.

Fresh Java4/4, Rust106/106 and Python3/3 pass; Rust fmt/Clippy, wrapper syntax and diff checks pass. Java tests cover owner markers/reserved paths/ports and the actual XID parser for the independent observer. Rust tests require both identities and prohibit mutations or treating Finished as retained timeout. Python negative cases reject wrong/missing record identity, actual branch resource/lock/XID drift, stale terms/indices, old exceptions, wrong exception class/message, truncated stacks and premature async classification. The standalone reader is also checked against the same real binary JFR: nine recorded frames match, a five-frame export and wrong state/time do not.

Retained failed attempts are not passes:

1. `run_20261112_163656_2c110813`: malformed observer ID made the original lock manager reject numeric parsing. Fixed with an actual-parser regression (Java4 tests, one expected RED failure).
2. `run_20261112_164628_40dc0453`: Begin control's class-loading exception mentioned the desired exception name; substring matching falsely rejected it. Exact thrown class/path/time classification fixes this.
3. `run_20261112_165048_ad293d4c`: full Begin cohort passed, then strict16 proof could not see the async caller because `jfr print` defaults to five frames. A standalone JDK `RecordingFile` stream reader preserves all recorded frames and filters irrelevant events. The original binary JFR independently confirms the real16 exception.

All original JFR files, failed results and logs remain. Seventeen large failed-attempt JSON exports were gzip-compressed losslessly, with original byte counts/SHA256 and compressed hashes in `lossless-json-compression.json`; decompression reproduces exact bytes. The final reader avoids generating those irrelevant class-loading exports. `manifest.json` binds sources, raw evidence, reader/helper/binary hashes and snapshots. [Read-only review](review.json) findings were fixed and re-reviewed; root execution and the archive verifier establish runtime evidence separately.

`JAVA-RAFT-RELOAD-001`, metadata deserialization and Sofa/Logback compatibility observations remain open. Natural deadlines, RAFT operator retry, durable RM compensation/replay, network partitions and production readiness are not covered. REC003/004 are no longer unknown at runtime, but safe recovery acceptance and the complete plan remain open. [Reproduction](../../../../tools/java-tc-integration/raft_recovery/README.md).
