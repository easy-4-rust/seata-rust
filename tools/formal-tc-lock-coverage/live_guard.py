"""Run the formal Rust preparation path against the pinned Java TC DB lock store."""
import argparse
import json
import os
from pathlib import Path
import socket
import struct
import sys
import tempfile
import time
import uuid
import zipfile

from live import DRIVER, DRIVER_SHA, IMAGE, PASSWORD, ROOT, need, put, reg, save, sha, support
import collation_equivalence
import identity_boundaries
import java_writer_admission
import writer_permissions
import lock_bundle
import lock_bundle_race
import mixed_java_lock
import rust_owner_lock
import storage_boundaries

HERE = Path(__file__).resolve().parent
TEST = "lock_guard_cases::native_registration_guard_preserves_exclusive_tc_lock_and_releases_on_original_rollback"


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def compile_auth(jar, work, out, env, identity=False):
    classes, libs, overlay = (work / name for name in ("classes", "libs", "fixture"))
    with zipfile.ZipFile(jar) as archive:
        for leaf in archive.namelist():
            if leaf.startswith("BOOT-INF/classes/") and leaf.endswith(".class"):
                put(classes / leaf.removeprefix("BOOT-INF/classes/"), archive.read(leaf))
            if leaf.startswith("BOOT-INF/lib/") and Path(leaf).name.startswith(
                ("seata-common-", "seata-core-", "spring-core-", "slf4j-api-")
            ):
                put(libs / Path(leaf).name, archive.read(leaf))
    handler = (HERE / "ResourceAliasAuthHandler.java" if identity else
               ROOT / "tools/formal-commit-direction/CallbackOwnershipAuthHandler.java")
    inputs = [ROOT / "tools/java-v1-golden/Phase0CredentialCheckAuthHandler.java", handler]
    cp = os.pathsep.join(map(str, [classes, *sorted(libs.glob("*.jar"))]))
    args = [reg.JAVA.with_name("javac.exe"), "-g", "--release", "8", "-encoding", "UTF-8",
            "-cp", cp, "-d", overlay, *inputs]
    support.command(args, env, out / "auth-compile.log")
    put(overlay / "META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler",
        f"org.apache.seata.server.auth.{handler.stem}\n")
    base = overlay / "org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class"
    historical = json.loads(reg.HISTORICAL.read_bytes())["extension_class_sha256"]
    save(out / "auth-binding.json", dict(sources={str(x): sha(x) for x in inputs},
         historical_class_sha256=historical, historical_bytes_reused=sha(base) == historical,
         runtime={x.relative_to(overlay).as_posix(): sha(x) for x in overlay.rglob("*") if x.is_file()}))
    return overlay


def verify_frames(out, obs):
    def frames(role):
        rows = [json.loads(line) for line in (out / f"{role}-frames.jsonl").read_text("utf-8").splitlines()]
        need(rows[-1]["succeeded"] and rows[-1]["faults"] == 0, "observer incomplete")
        return [row for row in rows if row["event"] == "frame"]
    rm, tm = frames("RM"), frames("TM")
    identity = obs["identity"]
    registration = [row for row in rm if row["direction"] == "c2s" and row["kind"] == 11]
    need(len(registration) == 2, "unexpected branch registration count")
    for request, xid, accepted in zip(registration, (identity["owner"], identity["contender"]), (True, False)):
        fields = support.branch_identity(request, False)
        need((fields["xid"], fields["resource"], fields["lock"], fields["branch_type"], fields["data"])
             == (xid, identity["resource"], identity["lock_key"], 0, ""), "request identity differs")
        replies = [row for row in rm if row["direction"] == "s2c" and row["kind"] == 12
                   and row["connection"] == request["connection"] and row["request_id"] == request["request_id"]]
        need(len(replies) == 1, "missing distinct registration reply")
        payload = support.payload(replies[0])
        if accepted:
            need(payload[2:4] == b"\x01\x00" and struct.unpack(">q", payload[4:])[0] == identity["branch_id"],
                 "original successful branch response differs")
        else:
            need(payload[2] == 0 and payload[-9] == obs["lock_conflict_code"], "original conflict differs")
    queries = [row for row in rm if row["direction"] == "c2s" and row["kind"] == 21]
    need(len(queries) == 1, "unexpected lock query count")
    query = queries[0]
    fields = support.branch_identity(query, False)
    need((fields["xid"], fields["resource"], fields["lock"]) ==
         (identity["contender"], identity["resource"], identity["lock_key"]), "lock query identity differs")
    query_replies = [row for row in rm if row["direction"] == "s2c" and row["kind"] == 22
                     and row["connection"] == query["connection"] and row["request_id"] == query["request_id"]]
    need(len(query_replies) == 1 and support.payload(query_replies[0])[2:] == b"\x01\x00\x00\x00",
         "original lock query was not unconditionally false")
    callbacks = [row for row in rm if row["direction"] == "s2c" and row["kind"] == 5]
    need(len(callbacks) == 1, "unexpected original rollback callbacks")
    callback = callbacks[0]
    fields = support.branch_identity(callback, True)
    need((fields["xid"], fields["branch_id"], fields["resource"]) ==
         (identity["owner"], identity["branch_id"], identity["resource"]), "original callback differs")
    returned = [row for row in rm if row["direction"] == "c2s" and row["kind"] == 6
                and row["connection"] == callback["connection"] and row["request_id"] == callback["request_id"]]
    need(len(returned) == 1, "original callback reply missing")
    payload = support.payload(returned[0])
    xid, position = support.string(payload, 4)
    need(payload[2:4] == b"\x01\x00" and xid == identity["owner"]
         and struct.unpack(">q", payload[position:position+8])[0] == identity["branch_id"]
         and payload[position+8:] == b"\x08", "original rollback status/identity differs")
    ends = [support.string(support.payload(row), 2)[0] for row in tm if row["direction"] == "c2s" and row["kind"] == 9]
    need(ends == [identity["contender"], identity["owner"]], "global rollback repeated or retargeted")
    return dict(branch_registrations=2, successful=1, conflict=1, lock_queries=1,
                original_rollback_callbacks=1, global_rollbacks=2)


def run(version, storage=False, identity=False, require_binding=False, enforce_lock_limits=False,
        collation=None, combined_profile=False, guard_location="both", java_owner=False, rust_owner=False,
        account_key="integer", bound_tinyint=False, lock_bundle_case=False, race=False, writer_admission=False, separated_writers=False, definer_metadata=False):
    owner = uuid.uuid4().hex
    mixed_clients = java_owner or rust_owner
    mismatched_tinyint = account_key == "tinyint-mismatch" and not bound_tinyint
    account = mixed_java_lock.AccountCase(account_key) if mixed_clients else None
    work = Path(tempfile.mkdtemp(prefix="seata_original_guard_", dir="D:/codex-builds"))
    family = ("at-definer-metadata" if definer_metadata else "at-writer-permissions" if separated_writers else "java-tinyint-writer-admission" if writer_admission else "tc-lock-bundle-race" if race else "tc-lock-bundle" if lock_bundle_case else "tc-lock-rust-java" if rust_owner else
              "tc-lock-java-rust" if java_owner else
              "java-tc-mysql-db-profile" if combined_profile else
              "tc-lock-collation-guard" if collation is not None else
              "tc-lock-db-length-guard" if enforce_lock_limits else
              "tc-lock-resource-binding" if require_binding else
              "tc-lock-identity-boundaries" if identity else
              "tc-lock-storage-boundaries" if storage else "tc-lock-native-registration")
    out = ROOT / "docs/phase-2/evidence" / family / work.name
    out.mkdir(parents=True)
    observations = work / "observations"
    for directory in (observations, work / "data", work / "init"):
        directory.mkdir()
    print(str(out), flush=True)
    env = os.environ | dict(TEMP="D:/Temp", TMP="D:/Temp", PYTHONUTF8="1", PYTHONDONTWRITEBYTECODE="1",
        CARGO_HOME=str(ROOT.parent / ".seata-cargo-cache"), CARGO_TARGET_DIR="D:/cargo-target/seata-rust-load-driver",
        CARGO_INCREMENTAL="0")
    for key in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "SEATA_CONFIG_NAME", "SEATA_ENV"):
        env.pop(key, None)
    env.pop("SEATA_TC_LOCK_ALIAS_PORT", None)
    env.pop("SEATA_TC_REQUIRE_RESOURCE_BINDING", None)
    env.pop("SEATA_TC_DB_LOCK_PROFILE", None)
    env.pop("SEATA_TC_ENFORCE_DB_LOCK_LIMITS", None)
    env.pop("SEATA_TC_LOCK_COLLATION_EQUIVALENCE", None)
    env.pop("SEATA_TC_ENFORCE_DB_LOCK_COLLATION", None)
    env["SEATA_TC_LOCK_GUARD_LOCATION"] = guard_location
    env.pop("SEATA_TC_JAVA_LOCK_OWNER", None)
    env.pop("SEATA_TC_RUST_LOCK_OWNER", None)
    env["SEATA_TC_ACCOUNT_KEY"] = account_key
    env.pop("SEATA_TC_AT_WRITER_USER", None)
    env.pop("SEATA_TC_AT_WRITER_PASSWORD", None)
    env.pop("SEATA_TC_BIND_TINYINT_PROFILE", None)
    env.pop("SEATA_TC_VERIFY_PREPROVISIONED_TINYINT", None)
    if writer_admission:
        env["SEATA_TC_VERIFY_PREPROVISIONED_TINYINT"] = "1"
    env.pop("SEATA_TC_LOCK_BUNDLE", None)
    env.pop("SEATA_TC_LOCK_BUNDLE_RACE", None)
    if race:
        env["SEATA_TC_LOCK_BUNDLE_RACE"] = "1"
    if bound_tinyint:
        env["SEATA_TC_BIND_TINYINT_PROFILE"] = "1"
    if lock_bundle_case:
        env["SEATA_TC_LOCK_BUNDLE"] = "1"
    if java_owner:
        env["SEATA_TC_JAVA_LOCK_OWNER"] = "1"
    if rust_owner:
        env["SEATA_TC_RUST_LOCK_OWNER"] = "1"
    if collation is not None:
        env["SEATA_TC_LOCK_COLLATION_EQUIVALENCE"] = "1"
        if collation == "guarded":
            env["SEATA_TC_ENFORCE_DB_LOCK_COLLATION"] = "1"
    if enforce_lock_limits:
        env["SEATA_TC_ENFORCE_DB_LOCK_LIMITS"] = "1"
    if require_binding:
        env["SEATA_TC_REQUIRE_RESOURCE_BINDING"] = "1"
    if combined_profile or mixed_clients or race:
        env["SEATA_TC_DB_LOCK_PROFILE"] = "1"
    cargo = "cargo"
    if version == "1.85":
        rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
        cargo = rust / "cargo.exe"
        env.update(RUSTC=str(rust / "rustc.exe"), RUSTDOC=str(rust / "rustdoc.exe"),
                   CARGO_TARGET_DIR="D:/cargo-target/seata-rust-load-driver-msrv")
    jar = reg.ORACLE / "server/target/seata-server.jar"
    need(reg.command(["git", "-C", reg.ORACLE, "rev-parse", "HEAD"], env).decode().strip() == reg.PIN
         and sha(jar) == reg.JAR_SHA and sha(DRIVER) == DRIVER_SHA, "pinned runtime differs")
    db = "undo_" + owner[:12]
    if storage:
        db += "_storage_boundaries"
        env["SEATA_TC_LOCK_STORAGE_BOUNDARIES"] = "1"
    else:
        env.pop("SEATA_TC_LOCK_STORAGE_BOUNDARIES", None)
    name = "seata-native-lock-guard-" + owner[:12]
    ports = [port() for _ in range(6 if rust_owner else 5 if identity or java_owner else 4)]
    need(len(set(ports)) == len(ports), "port collision")
    database_port, tc_port, rm_port, tm_port = ports[:4]
    alias_port = ports[4] if identity else None
    for role, rel in (("client", "script/client/at/db/mysql.sql"), ("server", "script/server/db/mysql.sql")):
        blob = reg.command(["git", "-C", reg.ORACLE, "cat-file", "blob", f"{reg.PIN}:{rel}"], env)
        put(out / f"{role}-schema.sql", blob)
    put(work / "init/00-tc.sql", (out / "server-schema.sql").read_bytes())
    if mixed_clients or race:
        put(work / "init/01-undo.sql", (out / "client-schema.sql").read_bytes())
    put(work / "client-schema.sql", (out / "client-schema.sql").read_bytes())
    sql = ["docker", "exec", "-e", "MYSQL_PWD="+PASSWORD, name, "mysql", "--protocol=TCP",
           "-h127.0.0.1", "-uroot", "--default-character-set=utf8mb4", "-N", "-B", "--raw", db, "-e"]
    resource = f"jdbc:mysql://127.0.0.1:{database_port}/{db}"
    alias = f"jdbc:mysql://127.0.0.1:{alias_port}/{db}" if identity else None
    if identity:
        env["SEATA_TC_LOCK_ALIAS_PORT"] = str(alias_port)
    put(work / "application.yml", f"""spring:
  main:
    web-application-type: none
logging:
  file:
    path: {work.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: {tc_port}
    enable-check-auth: {str(not mixed_clients).lower()}
    retry-dead-threshold: 1000
  store:
    mode: db
    session:
      mode: db
    lock:
      mode: db
    db:
      datasource: druid
      db-type: mysql
      driver-class-name: com.mysql.cj.jdbc.Driver
      url: {resource}?useSSL=false&allowPublicKeyRetrieval=true
      user: root
      password: {PASSWORD}
      min-conn: 1
      max-conn: 4
""")
    put(out / "application.yml", (work / "application.yml").read_bytes())
    auth = compile_auth(jar, work, out, env, identity)
    java_client_classpath = mixed_java_lock.compile_actor(work, out, env) if mixed_clients else None
    build = [cargo, "test", "--offline", "--locked", "-p", "seata-db-mysql"]
    if rust_owner:
        build += ["--features", "experimental_changed_commit"]
    build += ["--test", "live_commit_direction_java", "--no-run", "--message-format=json"]
    output = reg.command(build, env, timeout=180)
    put(out / "cargo-build.jsonl", output)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith(b"{")]
    compiled, = [Path(x["executable"]) for x in artifacts if x.get("executable")
                 and x["target"]["name"] == "live_commit_direction_java"]
    exe = work / "native-client.exe"
    put(exe, compiled.read_bytes())
    executable = sha(exe)
    put(out / "client-schema.binding", sha(out / "client-schema.sql"))
    env.update(SEATA_MYSQL_PORT=str(database_port), SEATA_MYSQL_DATABASE=db, SEATA_MYSQL_PASSWORD=PASSWORD,
               SEATA_UNDO_SCHEMA_PATH=str(work / "client-schema.sql"), SEATA_UNDO_STORE_OUT=str(observations),
               SEATA_RM_ENDPOINT=f"127.0.0.1:{rm_port}", SEATA_TM_ENDPOINT=f"127.0.0.1:{tm_port}")
    stop = work / "stop"
    processes = []
    container = None
    owner_actor = None
    try:
        alias_binding = ["-p", f"127.0.0.1:{alias_port}:3306"] if identity else []
        container = reg.command(["docker", "create", "--pull=never", "--name", name, "--label",
            "seata.fixture.owner="+owner, "-p", f"127.0.0.1:{database_port}:3306", *alias_binding, "--mount",
            "type=bind,source="+(work/"data").as_posix()+",target=/var/lib/mysql", "--mount",
            "type=bind,source="+(work/"init").as_posix()+",target=/docker-entrypoint-initdb.d,readonly",
            "-e", "MYSQL_ROOT_PASSWORD="+PASSWORD, "-e", "MYSQL_ROOT_HOST=%", "-e", "MYSQL_DATABASE="+db,
            IMAGE], env).decode().strip()
        reg.command(["docker", "start", container], env)
        if identity:
            bindings = json.loads(reg.command(["docker", "inspect", "--format",
                "{{json .NetworkSettings.Ports}}", container], env))
            need({(x["HostIp"], x["HostPort"]) for x in bindings["3306/tcp"]}
                 == {("127.0.0.1", str(database_port)), ("127.0.0.1", str(alias_port))},
                 "aliases do not map to the same owned MySQL container")
            save(out/"mysql-port-bindings.json", bindings)
        limit = time.monotonic() + 90
        while True:
            try:
                put(out/"mysql-runtime.tsv", reg.command(sql+["SELECT VERSION(),@@innodb_flush_log_at_trx_commit; SELECT COUNT(*) FROM lock_table"], env))
                break
            except RuntimeError:
                need(time.monotonic() < limit, "MySQL initialization timeout")
                time.sleep(.25)
        if storage:
            schema_profile = reg.command(sql+[
                "SELECT @@global.sql_mode,@@session.sql_mode,@@collation_server; "
                "SELECT COLUMN_NAME,CHARACTER_MAXIMUM_LENGTH,COLLATION_NAME FROM information_schema.COLUMNS "
                "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='lock_table' "
                "AND COLUMN_NAME IN ('row_key','pk','table_name') ORDER BY COLUMN_NAME"], env)
            put(out/"mysql-lock-schema.tsv", schema_profile)
            storage_boundaries.verify_schema(schema_profile)
        java = [reg.JAVA, "-Xlog:class+load=info:file=class-load.log", "-Djava.io.tmpdir=D:/Temp",
            "-Duser.home="+str(work), "-Dseata.callback.fixture.resource="+resource,
            "-Dspring.config.location=file:"+(work/"application.yml").as_posix(),
            "-Dloader.path="+auth.as_posix()+","+DRIVER.as_posix(), "-cp", jar,
            "org.springframework.boot.loader.launch.PropertiesLauncher", "-h", "127.0.0.1"]
        if identity:
            java.insert(4, "-Dseata.callback.fixture.alias="+alias)
        tc = support.spawn(java, work, env, out, "tc")
        processes.append(tc)
        support.wait_owned(tc, tc_port, env)
        for role, listener in (("RM", rm_port), ("TM", tm_port)):
            process = support.spawn([sys.executable, HERE/"proxy.py", "--listen-port", listener,
                "--upstream-port", tc_port, "--events", out/f"{role}-frames.jsonl", "--stop-file", stop,
                "--role", role], work, env, out, role)
            processes.append(process)
            support.wait_owned(process, listener, env)
        if mixed_clients:
            mixed_java_lock.seed_accounts(sql, env, account, out)
        if race:
            reg.command(sql + [lock_bundle_race.SEED], env)
        if lock_bundle_case:
            reg.command(sql + ["INSERT INTO accounts VALUES(0,2000);"
                               "CREATE TABLE reserves(id INT PRIMARY KEY,balance INT NOT NULL) ENGINE=InnoDB;"
                               "INSERT INTO reserves VALUES(1,3000)"], env)
        admission = None
        writer = "root"
        writer_password = PASSWORD
        if writer_admission:
            java_writer_admission.install_binding(sql, env, resource)
        if separated_writers:
            roles = writer_permissions.provision(sql, env, out, name, db, rust_commits=rust_owner,
                                                 definer_metadata=definer_metadata)
            writer, writer_password = roles.java
            env["SEATA_TC_AT_WRITER_USER"], env["SEATA_TC_AT_WRITER_PASSWORD"] = roles.rust
        if writer_admission:
            admission = java_writer_admission.verify_rejected(
                work, out, env, sql, java_client_classpath, ports[4], tc_port, resource, account,
                user=writer, password=writer_password
            )
        if java_owner:
            owner_actor = mixed_java_lock.start_actor(work, out, env, java_client_classpath,
                                                      ports[4], tc_port, resource, writer_password,
                                                      account_key=account_key, admission=admission, user=writer)
            processes.append(owner_actor[-1])
            if admission:
                java_writer_admission.check_binding_lock(owner_actor, out, sql, env)
            java_phase = mixed_java_lock.held(owner_actor, out)
        client = support.spawn([exe, TEST, "--ignored", "--exact", "--test-threads=1"], ROOT, env, out, "client")
        processes.append(client)
        save(out/"ownership.json", dict(container=container, owner=owner, db=db, java=list(map(str, java)),
             ports=ports, pids=[p.pid for p in processes],
             executable=str(exe), executable_sha256=executable))
        stages = (("commit-metadata-denied", "before-business-commit", *rust_owner_lock.STAGES)
                  if rust_owner and separated_writers else lock_bundle_race.STAGES if race else lock_bundle.STAGES if lock_bundle_case else mixed_java_lock.BOUND_STAGES if bound_tinyint else
                  (mixed_java_lock.MISMATCH_STAGE,) if mismatched_tinyint else
                  rust_owner_lock.STAGES if rust_owner else mixed_java_lock.STAGES if java_owner else
                  collation_equivalence.CASES if collation is not None else
                  identity_boundaries.CASES if identity else
                  storage_boundaries.CASES if storage else ("held", "contended"))
        observed_stages = []
        java_stages = []

        def start_contender(conflict):
            actor = mixed_java_lock.start_actor(work, out, env, java_client_classpath,
                ports[4 if conflict else 5], tc_port, resource, writer_password,
                label="java-conflict" if conflict else "java-admitted", expect_lock_conflict=conflict,
                account_key=account_key, admission=admission, user=writer)
            processes.append(actor[-1])
            if admission:
                label = "java-conflict" if conflict else "java-admitted"
                java_writer_admission.check_binding_lock(actor, out / label, sql, env)
            return actor

        for stage in stages:
            limit = time.monotonic() + 50
            while True:
                try:
                    expected = json.loads((observations/f"{stage}.json").read_bytes())
                    break
                except (FileNotFoundError, ValueError):
                    need(client.poll() is None and time.monotonic() < limit, f"client exited before {stage}")
                    time.sleep(.05)
            put(out/f"{stage}.json", (observations/f"{stage}.json").read_bytes())
            observed_stages.append(expected)
            if race:
                lock_bundle_race.observe(out, stage, expected, observed_stages[0], sql, env)
            elif lock_bundle_case:
                lock_bundle.observe(owner_actor, java_phase, out, stage, expected, observed_stages, sql, env)
            elif rust_owner and stage not in rust_owner_lock.STAGES:
                writer_permissions.observe_commit_admission(stage, expected, sql, env, out, roles, resource, account)
            elif rust_owner:
                java_stages.append(rust_owner_lock.observe(stage, expected, out, sql, env, start_contender, account))
            elif mismatched_tinyint:
                mixed_java_lock.observe_mismatch(owner_actor, java_phase, expected, out, sql, env, account)
            elif java_owner:
                mixed_java_lock.observe(owner_actor, java_phase, out, stage, expected, sql, env, account,
                                        bound_tinyint=bound_tinyint)
            elif storage or identity or collation is not None:
                storage_boundaries.snapshot(out, stage, expected, observed_stages, sql, env)
            else:
                raw = reg.command(sql+["SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table"], env)
                put(out/f"{stage}-database.tsv", raw)
                need(raw.decode().strip().split("\t") == [expected["owner"], str(expected["branch_id"]), resource,
                     "undo_work", "1", resource+"^^^undo_work^^^1", "0"], "independent lock row differs")
            put(observations/f"{stage}.ack", "confirmed\n")
        need(client.wait(timeout=45) == 0, "native client failed")
        put(stop, "complete\n")
        need(all(process.wait(timeout=12) == 0 for process in processes[1:3]), "frame observer failed")
        observation = json.loads((observations/"tc-lock-guard-result.json").read_bytes())
        put(out/"observation.json", (observations/"tc-lock-guard-result.json").read_bytes())
        if java_owner and not (mismatched_tinyint or lock_bundle_case):
            put(out / "guard-branches.log", (observations / "guard-branches.log").read_bytes())
        counts = reg.command(sql+["SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
            "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_work),(SELECT COUNT(*) FROM guard_text)"], env)
        put(out/"final-counts.tsv", counts)
        need(counts.decode().strip() == "0\t0\t0\t0\t0", "transaction, lock or business rows remain")
        if race:
            wire = lock_bundle_race.finish(out, observed_stages, observation, sql, env)
        elif lock_bundle_case:
            wire = lock_bundle.finish(owner_actor, out, java_phase, observed_stages, observation, sql, env)
            owner_actor = None
        elif rust_owner:
            wire = rust_owner_lock.finish(out, observation, observed_stages[-2:], java_stages, sql, env, account)
        elif mismatched_tinyint:
            wire = mixed_java_lock.finish_mismatch(owner_actor, out, java_phase, observed_stages[0],
                observation, sql, env, account, rust_owner_lock.verify_java_frames)
            owner_actor = None
        elif java_owner:
            wire = mixed_java_lock.finish(owner_actor, out, java_phase, observed_stages, observation, sql, env,
                                         account, bound_tinyint=bound_tinyint)
            owner_actor = None
        elif identity:
            need(observation["cases"] == observed_stages, "final cases differ from observations")
            physical = json.loads((observations/"identity-physical.json").read_bytes())
            need(observation["physical"] == physical, "final physical identity differs")
            put(out/"identity-physical.json", (observations/"identity-physical.json").read_bytes())
            actual = reg.command(sql+["SELECT @@server_uuid,DATABASE(),@@port,COUNT(*) FROM identity_pair"], env)
            put(out/"final-identity-database.tsv", actual)
            need(actual.decode().strip().split("\t") == [physical["server_uuid"], db, "3306", "0"]
                 and physical["schema"] == db, "independent database identity or final count differs")
            wire = identity_boundaries.verify(out, observed_stages, physical, resource, alias,
                                              require_binding=require_binding)
            if require_binding:
                binding_bytes = reg.command(sql+["SELECT HEX(resource_id) FROM seata_rs_at_resource_binding WHERE slot=1"], env)
                put(out/"binding-bytes.tsv", binding_bytes)
                need(bytes.fromhex(binding_bytes.decode()) == resource.encode(), "original binding bytes differ")
            storage_boundaries.verify_terminal_rows(out, observed_stages, sql, env)
        elif collation is not None:
            need(observation["cases"] == observed_stages, "final cases differ from observations")
            collation_equivalence.verify_database(out, sql, env)
            wire = collation_equivalence.verify(out, observed_stages, enforce=collation == "guarded")
            storage_boundaries.verify_terminal_rows(out, observed_stages, sql, env)
        elif storage:
            need(observation["cases"] == observed_stages, "final cases differ from observations")
            tables = sorted({case["table"] for case in observed_stages})
            need(all(table in ("storage_text", "t"*32, "t"*33) for table in tables),
                 "unexpected fixture table")
            remaining = reg.command(sql+["SELECT "+",".join(f"(SELECT COUNT(*) FROM {table})"
                                                            for table in tables)], env)
            put(out/"final-storage-business-counts.tsv", remaining)
            need(remaining.decode().strip().split("\t") == ["0"]*3, "storage business rows remain")
            wire = storage_boundaries.verify(out, observed_stages, enforce_lock_limits=enforce_lock_limits)
            storage_boundaries.verify_terminal_rows(out, observed_stages, sql, env)
        else:
            wire = verify_frames(out, observation)
        if writer_admission:
            wire["java_writer_admission"] = "original_jdbc_resource_and_tinyint_mapping_verified"
        if separated_writers:
            wire.update(writer_permissions.finish(sql, env, out, roles, resource, rust_commits=rust_owner,
                                                 committed_identity=observation.get("identity")))
        loaded = (work/"class-load.log").read_text("utf-8")
        for klass in ("org.apache.seata.server.lock.AbstractLockManager", "org.apache.seata.server.coordinator.DefaultCore",
                      "org.apache.seata.server.storage.db.lock.LockStoreDataBaseDAO"):
            line = next(line for line in loaded.splitlines() if f" {klass} source: " in line)
            need(jar.as_posix() in line.replace("\\", "/"), f"shadowed original class {klass}")
        need(sha(exe) == executable and sha(jar) == reg.JAR_SHA, "runtime binary changed")
        status = ("RUST_CHANGED_COMMIT_WITH_DEFINER_METADATA" if definer_metadata else
                  "RUST_CHANGED_COMMIT_WITH_SEPARATE_DATABASE_GRANTS" if separated_writers and rust_owner else
                  "JAVA_RUST_WRITERS_USE_SEPARATE_DATABASE_GRANTS" if separated_writers else
                  "JAVA_TINYINT_WRITER_ADMISSION_VERIFIED" if writer_admission else
                  "ORIGINAL_TC_CONCURRENT_LOCK_BUNDLE_ONE_WINNER" if race else
                  "ORIGINAL_TC_REJECTS_LOCK_BUNDLE_WITHOUT_ORPHAN_LOCKS" if lock_bundle_case else
                  "BOUND_TINYINT_PROFILE_REJECTS_MISMATCH_BEFORE_REGISTRATION" if bound_tinyint else
                  "MIXED_TINYINT_PROFILES_ACCEPT_TWO_LOCKS_ON_SAME_ROW" if mismatched_tinyint else
                  "RUST_AT_OWNER_EXCLUDES_ORIGINAL_JAVA_CONTENDER" if rust_owner else
                  "ORIGINAL_JAVA_AT_OWNER_EXCLUDES_RUST_CONTENDER" if java_owner else
                  "ORIGINAL_TC_DB_COLLATION_MISMATCH_REJECTED_BEFORE_REGISTRATION" if collation == "guarded" else
                  "ORIGINAL_TC_DB_COLLATION_MISMATCH_OBSERVED" if collation == "baseline" else
                  "ORIGINAL_TC_DB_OVERLENGTH_LOCK_KEYS_REJECTED_BEFORE_REGISTRATION" if enforce_lock_limits else
                  "ORIGINAL_TC_DB_LOCK_ALIAS_REJECTED_BEFORE_REGISTRATION" if require_binding else
                  "ORIGINAL_TC_DB_LOCK_ALIAS_AND_LOCAL_COMPOSITE_GUARD_OBSERVED" if identity else
                  "ORIGINAL_TC_DB_LOCK_STORAGE_BOUNDARIES_OBSERVED" if storage
                  else "ORIGINAL_TC_NATIVE_REGISTRATION_LOCK_LIFECYCLE_VERIFIED")
        save(out/"result.json", dict(status=status,
            rust=version, java_pin=reg.PIN, jar_sha256=reg.JAR_SHA, jdbc_sha256=DRIVER_SHA,
            mysql_image=IMAGE, guard_location=guard_location, network=wire,
            account_key=account_key, composite_key=account_key == "composite", bound_tinyint=bound_tinyint,
            actual_rust_business_commit=rust_owner, full_TC_gate=False))
    finally:
        if owner_actor:
            owner_actor[2].close()
        if not stop.exists():
            put(stop, "cleanup\n")
        for process in reversed(processes):
            support.stop(process)
        if (work/"class-load.log").exists():
            put(out/"class-load.log", (work/"class-load.log").read_bytes())
        owned = ["docker", "ps", "-aq", "--no-trunc", "--filter", "label=seata.fixture.owner="+owner]
        candidates = reg.command(owned, env).decode().split()
        need(len(candidates) <= 1, "fixture label identifies multiple containers")
        if candidates:
            candidate, = candidates
            inspected = json.loads(reg.command(["docker", "inspect", candidate], env))[0]
            need(inspected["Id"] == candidate
                 and (container is None or container == candidate)
                 and inspected["Name"] == "/"+name
                 and inspected["Config"]["Labels"]["seata.fixture.owner"] == owner,
                 "container does not belong to this run")
            try:
                put(out/"mysql.log", reg.command(["docker", "logs", candidate], env))
            finally:
                reg.command(["docker", "rm", "-f", candidate], env)
            need(not reg.command(owned, env).strip(), "owned container remains")
        need(all(process.poll() is not None for process in processes), "owned process remains")
        save(out/"cleanup.json", dict(owned_processes_stopped=True, container_removed=bool(candidates)))
    members = [dict(path=p.relative_to(out).as_posix(), sha256=sha(p)) for p in sorted(out.rglob("*")) if p.is_file()]
    save(out/"catalog.json", dict(members=members))
    print(f"VERIFIED {version} {out} {sha(out/'catalog.json')}", flush=True)


if __name__ == "__main__":
    cli = argparse.ArgumentParser()
    cli.add_argument("rust", choices=["1.99", "1.85"])
    profile = cli.add_mutually_exclusive_group()
    profile.add_argument("--storage-boundaries", action="store_true")
    profile.add_argument("--identity-boundaries", action="store_true")
    profile.add_argument("--resource-binding", action="store_true")
    profile.add_argument("--mysql-lock-limits", action="store_true")
    profile.add_argument("--mysql-lock-collation", choices=("baseline", "guarded"))
    profile.add_argument("--full-db-profile", action="store_true")
    profile.add_argument("--java-owner", action="store_true")
    profile.add_argument("--rust-owner", action="store_true")
    profile.add_argument("--race-lock-bundles", action="store_true")
    key = cli.add_mutually_exclusive_group()
    key.add_argument("--composite-key", action="store_true")
    key.add_argument("--decimal-key", choices=("plain", "scientific"))
    key.add_argument("--binary-key", action="store_true")
    key.add_argument("--tinyint-key", choices=("boolean", "numeric", "mismatch"))
    key.add_argument("--multi-row-locks", action="store_true")
    cli.add_argument("--bind-tinyint-profile", action="store_true")
    cli.add_argument("--java-writer-admission", action="store_true")
    cli.add_argument("--separate-writer-roles", action="store_true")
    cli.add_argument("--definer-metadata", action="store_true")
    cli.add_argument("--guard-location", choices=("both", "transaction", "manager"), default="both")
    options = cli.parse_args()
    if options.definer_metadata and not (options.rust_owner and options.separate_writer_roles):
        cli.error("--definer-metadata requires --rust-owner --separate-writer-roles")
    if options.separate_writer_roles and not options.java_writer_admission:
        cli.error("--separate-writer-roles requires --java-writer-admission")
    if options.java_writer_admission and not (
            options.tinyint_key == "boolean" and (options.java_owner or (options.rust_owner and options.separate_writer_roles))):
        cli.error("--java-writer-admission requires --tinyint-key boolean and either a Java owner or separate writer roles")
    if options.multi_row_locks and not options.java_owner:
        cli.error("--multi-row-locks requires --java-owner")
    if options.tinyint_key == "mismatch" and not options.java_owner:
        cli.error("--tinyint-key mismatch requires --java-owner")
    if options.bind_tinyint_profile and options.tinyint_key != "mismatch":
        cli.error("--bind-tinyint-profile requires --tinyint-key mismatch")
    if (options.composite_key or options.decimal_key or options.binary_key or options.tinyint_key) and not (
            options.java_owner or options.rust_owner):
        cli.error("Key shape options require --java-owner or --rust-owner")
    if options.guard_location != "both" and not (options.mysql_lock_limits or options.mysql_lock_collation == "guarded"
                                                   or options.bind_tinyint_profile):
        cli.error("--guard-location requires --mysql-lock-limits or --mysql-lock-collation guarded")
    if options.composite_key:
        account_key = "composite"
    elif options.decimal_key:
        account_key = f"decimal-{options.decimal_key}"
    elif options.tinyint_key:
        account_key = f"tinyint-{options.tinyint_key}"
    else:
        account_key = "binary" if options.binary_key else "integer"
    run(options.rust, storage=options.storage_boundaries or options.mysql_lock_limits,
        identity=options.identity_boundaries or options.resource_binding or options.full_db_profile,
        require_binding=options.resource_binding or options.full_db_profile,
        enforce_lock_limits=options.mysql_lock_limits, collation=options.mysql_lock_collation,
        combined_profile=options.full_db_profile, guard_location=options.guard_location,
        java_owner=options.java_owner, rust_owner=options.rust_owner, account_key=account_key,
        bound_tinyint=options.bind_tinyint_profile, lock_bundle_case=options.multi_row_locks,
        race=options.race_lock_bundles, writer_admission=options.java_writer_admission,
        separated_writers=options.separate_writer_roles, definer_metadata=options.definer_metadata)
