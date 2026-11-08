# Java Seata TC Integration Harness (Phase 0.9.1)

This harness creates an isolated, reproducible Java Seata TC process for the later native V1 client tests.

## Frozen baseline

- Java Seata: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`
- JDK: 17
- Storage: FILE for global sessions, branch sessions and locks
- Registry/config center: file
- Native service port: 18091 by default
- Data and logs: `$SEATA_TC_WORK_DIR`, defaulting to `$TMPDIR/seata-rust-java-tc`

The checked-in configuration disables TC auth checks **only for this isolated bootstrap smoke test**. Authentication and registration behavior are separate Phase 0.9 integration gates and must not be considered verified by this smoke test.

## Commands

Run from the Seata-Rust repository root:

```bash
bash tools/java-tc-integration/scripts/build-tc.sh
bash tools/java-tc-integration/scripts/start-tc.sh
bash tools/java-tc-integration/scripts/stop-tc.sh
bash tools/java-tc-integration/scripts/reset-storage.sh
bash tools/java-tc-integration/scripts/smoke.sh
```

Environment overrides:

- `SEATA_TC_WORK_DIR`: isolated checkout, data and logs root.
- `SEATA_TC_PORT`: native service port; defaults to 18091.

The build script clones the Java source if needed, checks out the exact pinned SHA, then runs:

```bash
mvn -B -ntp -Prelease-seata-jar -pl server -am \
  -DskipTests \
  -Dcheckstyle.skip=true \
  -Dspotless.check.skip=true \
  clean package
```

It expects the self-contained Spring Boot executable artifact at `server/target/seata-server.jar`, based on the pinned server POM's `release-seata-jar` profile (`finalName=seata-server`, default executable JAR layout). The `release-seata` profile instead creates a ZIP-layout launcher that relies on an external `lib/` directory; invoking it with plain `java -jar` does not provide Spring Boot classes and is not used by this harness.

## What the smoke test proves

- The pinned Java Seata server source can be built into its executable server artifact.
- The server process starts with the isolated FILE configuration.
- The configured TCP port accepts a connection.

## What it does NOT prove

- Seata V1 frame acceptance by the live server.
- RegisterTM / RegisterRM success.
- Authentication compatibility.
- GlobalBegin / BranchRegister / GlobalStatus / Commit / Rollback behavior.
- RM phase-two callbacks.
- Session/branch persistence across a real process restart.

Those remain later INT-xxx gates. In particular, a successful TCP connect is not a successful Seata protocol handshake.
