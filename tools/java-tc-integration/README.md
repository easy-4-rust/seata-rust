# Java Seata TC Integration Harness (Phase 0.9.2)

This harness creates an isolated, reproducible Java Seata TC process for the later native V1 client tests.

## Frozen baseline

- Java Seata: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`
- JDK: 17
- Storage: FILE for global sessions, branch sessions and locks
- Registry/config center: file
- Native service port: 18091 by default
- Data and logs: `$SEATA_TC_WORK_DIR`, defaulting to `$TMPDIR/seata-rust-java-tc`

The checked-in configuration disables TC auth checks **only for this isolated test harness**. Run #23 verified RegisterTM/RegisterRM with auth disabled; authentication compatibility and denial behavior remain separate gates.

## Commands

Run from the Seata-Rust repository root:

```bash
bash tools/java-tc-integration/scripts/build-tc.sh
bash tools/java-tc-integration/scripts/start-tc.sh
bash tools/java-tc-integration/scripts/stop-tc.sh
bash tools/java-tc-integration/scripts/reset-storage.sh
bash tools/java-tc-integration/scripts/smoke.sh
# Or run the Rust V1 registration client against an already-running TC:
bash tools/java-tc-integration/scripts/run-rust-registration.sh
```

Environment overrides:

- `SEATA_TC_WORK_DIR`: isolated checkout, data and logs root.
- `SEATA_TC_PORT`: native service port; defaults to 18091.

The build script clones the Java source if needed, checks out the exact pinned SHA, activates the self-contained `release-seata-jar` profile, and retries Maven up to three times for transient dependency download failures. It then runs:

```bash
mvn -B -ntp -Prelease-seata-jar -pl server -am \
  -DskipTests \
  -Dcheckstyle.skip=true \
  -Dspotless.check.skip=true \
  clean package
```

It expects the self-contained Spring Boot executable artifact at `server/target/seata-server.jar`, based on the pinned server POM's `release-seata-jar` profile (`finalName=seata-server`, default executable JAR layout). The `release-seata` profile instead creates a ZIP-layout launcher that relies on an external `lib/` directory; invoking it with plain `java -jar` does not provide Spring Boot classes and is not used by this harness.

## Independent Rust registration client

`rust-client/` is a standalone test-only Rust binary using the standard library (no production TM/RM crates or third-party Rust dependencies). The smoke script runs its unit tests and then connects to the live Java TC on two separate TCP connections:

- TM: RegisterTM (message type 101) and validates RegisterTMResponse (102).
- RM: RegisterRM (message type 103) and validates RegisterRMResponse (104).

It validates V1 framing, serializer IDs, request correlation, IdentifyResponse and `identified=true`. It holds both connections briefly after registration. It does not yet issue GlobalBegin/BranchRegister or handle RM phase-two callbacks.

## What the smoke test proves

- The pinned Java Seata server source can be built into its executable server artifact.
- The server process starts with the isolated FILE configuration and accepts TCP connections.
- The independent Rust client exchanged native V1 RegisterTM and RegisterRM messages with the live Java TC in [Run #23](https://github.com/easy-4-rust/seata-rust/actions/runs/42475353541). The follow-up run will reconfirm using the pinned Go client version string 1.1.0 and the corrected resettable FILE data path.

## What it does NOT prove

- GlobalBegin / BranchRegister / GlobalStatus / Commit / Rollback behavior.
- Authentication compatibility.
- RM phase-two callbacks.
- Session/branch persistence across a real process restart.

Those remain later INT-xxx gates. A successful TCP connect alone is not a successful Seata protocol handshake; the Rust registration requests now provide the first live protocol evidence.
