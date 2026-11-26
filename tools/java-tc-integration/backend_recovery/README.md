# MySQL / Redis retry-timeout restart harness

Run from the repository root on the documented Windows development machine:

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/java-tc-integration/backend_recovery/run.py
python -m unittest discover -s tools/java-tc-integration/backend_recovery -p test_run.py -v
```

This uses the existing JDK17, Maven3.9.9, Python, Rust and Docker Desktop. Runtime paths must remain on D/E; the runner checks Docker's data folder and desktop-linux context. Cached image IDs are fixed in `run.py`; it never pulls or installs anything. Ports28091/28093/28094 must be free. Each run gets a fresh database, D-drive bind data, evidence directory and labeled containers. Cleanup stops only its own processes/containers and preserves data and stopped containers for inspection.

The owned Java build tree must be the pinned source snapshot. Before/after a fresh helper build, the runner validates every tracked production source/resource and Maven POM against that commit, allowing only CRLF/LF differences (2122 files). Two owned helper overlays are copied into the test module and their generated class leaves removed before fresh tests. Three fresh Java tests include real ChannelManager parsing of the stored old client address; port0 is invalid, so the fixture uses an old port1 and exercises actual channel fallback. Source equality is checked after formatting; if it fails, reconcile the two tested helper sources and rerun. No old passing report is accepted.

For each backend, real pinned Java managers seed four records: CommitRetryTimeout16 and RollbackRetryTimeout17, each with and without an AT branch. Nonempty cases acquire a real stored lock. The writer exits; the actual backend process restarts; a distinct reader JVM checks original statuses, BranchIds and locks. The original TC JAR then starts using that store. Both fixture JVMs use the original JAR dependency cohort, with only one external helper class and the already-cached JDBC8.0.33 JAR added through PropertiesLauncher. A real StandardEnvironment is populated from the endpoint-validated owned configuration before the JAR's configuration SPI loads; managers and stores are not mocked. The TC stage remains a complete original application startup.

Do not use the reactor test module's full runtime classpath for these fixtures: it resolves Jedis3.8, whose hset returns Long, while the original TC's Jedis7 hset returns long. The resulting NoSuchMethodError was reproduced and retained in the failed-run evidence; the original JAR cohort avoids that harness ABI mismatch.

The runner queries the actual same-port console API, starts the Rust probe, then invokes `changeGlobalStatus` once. Rust first requires the original timeout state and correct lock occupation, then serves the original branch callback and checks session absence/lock release. Rust never sends an end request or BranchRegister for the old XID. Only its separate contender transactions are created and ended.

A second operator request must be rejected without resurrecting a record. On this pinned Netty console path the missing-session exception becomes HTTP500 with an empty body. That response alone is insufficient: the runner also requires newly appended owned-TC logs containing the current XID and the exact missing-session cause. Unit negative controls reject unrelated failures, malformed responses and another transaction's logs.

Ordinary observer rollbacks acknowledge11 but DB/Redis can retain status4 under DELAY_HANDLE_SESSION. After original-target checks, the runner selects only its two distinct recorded observer XIDs, verifies actual status4 and zero branches, explicitly finishes them through the TC's sendCommitOrRollback operator API, then verifies absence. This is explicit fixture teardown, not automatic recovery or a native retry of the original target. Final checks still require zero DB global/branch/lock rows and no Redis global/branch keys.

This is a real store/restart/operator integration with injected persisted statuses and **simulated RM actions**. It does not test natural retry-deadline generation, real business SQL/Undo Log compensation, durable RM replay safety, live authentication, RAFT leader recovery or production readiness. Executed results belong under `docs/phase-0/evidence/local-backend-retry-timeout`; this README alone is not test evidence.
