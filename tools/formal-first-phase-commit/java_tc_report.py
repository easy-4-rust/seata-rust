"""Run the feature-gated changed COMMIT/report against the pinned original Java TC."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/formal-tc-lock-coverage"))
import live_guard as guard  # noqa: E402

OWNER = "seata-wsl-java-tc-full-profile-20261124"
LABEL = "seata-java-tc-full-profile-validation-20261124"
SOCKET = "unix:///run/seata-validation-20261120/docker.sock"
TESTS = {
    "no_change_commit": "java_no_change_commit::original_java_tc_no_change_commit_leaves_no_branch",
    "no_change_ack_loss": "java_no_change_commit::original_java_tc_no_change_lost_ack_remains_unknown_without_branch",
    "no_change_pool_recovery": "java_no_change_commit::original_java_tc_single_slot_pool_recovers_after_commit_timeout",
    "changed_pool_recovery": "original_java_tc_changed_commit_timeout_recycles_both_pool_slots",
    "tls_wrong_ca": "original_java_tc_rejects_wrong_mysql_ca_then_commits_with_trusted_ca",
    "commit_permission": "original_java_tc_retries_commit_after_terminal_column_grant",
    "tm_durable_commit_loss": "original_java_tc_durable_tm_lost_commit_reply_never_replays",
    "tm_durable_rollback_loss": "original_java_tc_durable_tm_lost_rollback_reply_never_replays",
    "tm_recovered_commit": "original_java_tc_supervised_tm_recovers_commit",
    "tm_recovered_rollback_loss": "original_java_tc_supervised_tm_recovers_rollback_with_reply_loss",
    "tm_recovered_at_commit": "original_java_tc_supervised_tm_commits_at_branch",
    "tm_recovered_at_rollback_loss": "original_java_tc_supervised_tm_rolls_back_at_branch_with_reply_loss",
    "tm_attested_at_commit": "original_java_tc_supervised_tm_commits_at_branch",
    "tm_attested_at_rollback_loss": "original_java_tc_supervised_tm_rolls_back_at_branch_with_reply_loss",
    "rollback_orphan_missing_root": "original_java_tc_retries_rollback_after_missing_undo_root_is_restored",
    "rollback_orphan_guarded_root": "original_java_tc_retries_rollback_after_guarded_undo_root_is_restored",
    "rollback_dirty": "original_java_tc_stops_automatic_rollback_retry_for_dirty_undo",
    "commit_mixed": "original_java_tc_commits_mixed_compound_key_work",
    "rollback_mixed": "original_java_tc_rolls_back_mixed_compound_key_work",
    "commit_callback_reply_loss": "original_java_tc_changed_commit_reports_then_commits_original_branch",
    "rollback_callback_reply_loss": "original_java_tc_retries_at_rollback_after_phase_two_fault",
    "rollback_missing_undo": "original_java_tc_rollback_with_missing_undo_preserves_original_guard",
    "rollback_missing_undo_reply_loss": "original_java_tc_rollback_with_missing_undo_preserves_original_guard",
    "commit": "original_java_tc_changed_commit_reports_then_commits_original_branch",
    "commit_recovered": "original_java_tc_recovered_commit_reports_then_commits_original_branch",
    "undo_checksum_legacy": "original_java_tc_recovered_commit_reports_then_commits_original_branch",
    "undo_checksum_rollback": "original_java_tc_rejects_modified_undo_then_recovers_original_bytes",
    "undo_checksum": "original_java_tc_rejects_modified_undo_then_recovers_original_bytes",
    "commit_cross_process": "original_java_tc_cross_process_recovery_reports_original_branch",
    "armed_child_business_commit": "original_java_tc_armed_child_crash_before_business_commit",
    "armed_child_business_commit_ack": "original_java_tc_armed_child_crash_after_business_commit_before_ack",
    "armed_child_commit_in_flight": "original_java_tc_armed_child_crash_during_business_commit",
    "armed_child_fence_ack_loss": "original_java_tc_armed_child_fence_ack_loss_remains_unknown_until_reobserved",
    "armed_child_query_failure": "original_java_tc_armed_child_witness_query_failure_remains_unknown",
    "armed_child_arm_ack_loss": "original_java_tc_arming_ack_loss_suppresses_business_commit",
    "armed_child_arm_blocked": "original_java_tc_arming_commit_blocked_leaves_attempt_missing",
    "armed_child_wait_rollback": "original_java_tc_recovery_waits_for_old_business_rollback",
    "armed_child_wait_commit": "original_java_tc_recovery_waits_for_old_business_commit",
    "commit_cross_process_race": "original_java_tc_cross_process_concurrent_recovery_reports_once",
    "commit_distinct_process_race": "original_java_tc_distinct_processes_compete_for_report",
    "commit_report_loss": "original_java_tc_lost_success_report_reply_keeps_confirmed_commit_and_receipt",
    "commit_report_claim_loss": "original_java_tc_lost_report_claim_commit_reply_never_sends_report",
    "commit_report_claim_child_exit": "original_java_tc_report_claim_child_exit_never_replays",
    "failed": "original_java_tc_rolled_back_local_branch_reports_phase_one_failed",
    "report_loss": "original_java_tc_lost_failure_report_reply_keeps_rollback_and_receipt",
    "undo_error": "original_java_tc_undo_table_guard_error_rolls_back_before_automatic_failure_report",
    "undo_report_loss": "original_java_tc_undo_guard_error_lost_failure_report_reply_preserves_prepare_error",
    "undo_duplicate": "original_java_tc_real_undo_unique_conflict_rolls_back_before_failure_report",
    "enroll_error": "original_java_tc_enrollment_error_rolls_back_before_automatic_failure_report",
    "enroll_duplicate": "original_java_tc_real_enrollment_unique_conflict_rolls_back_before_failure_report",
    "enroll_duplicate_report_loss": "original_java_tc_enrollment_unique_conflict_lost_failure_report_reply",
}
PHASE_TWO_DB_FAULTS = {
    f"{phase}_db_{mode}": (phase, mode)
    for phase in ("commit", "rollback")
    for mode in ("block_terminal", "block_commit", "in_flight", "drop_ok")
}
TESTS.update({
    scenario: TESTS[f"{phase}_callback_reply_loss"]
    for scenario, (phase, _) in PHASE_TWO_DB_FAULTS.items()
})
OUT_ROOT = ROOT / "docs/phase-2/evidence/java-tc-changed-commit-report"


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def run(version, scenario="commit"):
    db_fault = PHASE_TWO_DB_FAULTS.get(scenario)
    mysql_tls = scenario == "tls_wrong_ca"
    terminal_permission = scenario == "commit_permission"
    changed_pool = scenario == "changed_pool_recovery"
    pool_recovery = scenario == "no_change_pool_recovery"
    no_change = pool_recovery or scenario in ("no_change_commit", "no_change_ack_loss")
    no_change_loss = pool_recovery or scenario == "no_change_ack_loss"
    fence_ack_lost = scenario == "armed_child_fence_ack_loss"
    witness_query_failure = scenario == "armed_child_query_failure"
    arming_blocked = scenario == "armed_child_arm_blocked"
    arming_fault = arming_blocked or scenario == "armed_child_arm_ack_loss"
    wait_rollback = scenario == "armed_child_wait_rollback"
    wait_commit = scenario == "armed_child_wait_commit"
    armed_child = (fence_ack_lost or witness_query_failure or arming_fault or wait_rollback
                   or scenario == "armed_child_business_commit")
    armed_child_in_flight = scenario == "armed_child_commit_in_flight"
    armed_child_ack = armed_child_in_flight or wait_commit or scenario == "armed_child_business_commit_ack"
    business_child_exit = armed_child or armed_child_ack
    report_claim_loss = scenario == "commit_report_claim_loss"
    report_child_exit = scenario == "commit_report_claim_child_exit"
    report_claim_rollback = report_claim_loss or report_child_exit
    dirty_rollback = scenario == "rollback_dirty"
    mixed_at = scenario in ("commit_mixed", "rollback_mixed")
    orphan_rollback = scenario in ("rollback_orphan_missing_root", "rollback_orphan_guarded_root")
    missing_undo = scenario in ("rollback_missing_undo", "rollback_missing_undo_reply_loss")
    callback_reply_loss = scenario in ("commit_callback_reply_loss", "rollback_callback_reply_loss",
                                       "rollback_missing_undo_reply_loss")
    tm_remote = scenario in ("tm_attested_at_commit", "tm_attested_at_rollback_loss")
    tm_at = tm_remote or scenario in ("tm_recovered_at_commit", "tm_recovered_at_rollback_loss")
    tm_recovery = tm_at or scenario in ("tm_recovered_commit", "tm_recovered_rollback_loss")
    tm_durable_mode = tm_recovery or scenario in ("tm_durable_commit_loss", "tm_durable_rollback_loss")
    tm_rollback = scenario in ("tm_durable_rollback_loss", "tm_recovered_rollback_loss", "tm_recovered_at_rollback_loss", "tm_attested_at_rollback_loss")
    tm_drop_reply = tm_durable_mode and scenario not in ("tm_recovered_commit", "tm_recovered_at_commit", "tm_attested_at_commit")
    commit_mode = scenario in ("undo_checksum_legacy", "undo_checksum", "commit", "commit_recovered", "commit_cross_process", "commit_cross_process_race", "commit_distinct_process_race", "commit_report_loss")
    report_loss_mode = scenario in ("report_loss", "commit_report_loss", "undo_report_loss", "enroll_duplicate_report_loss")
    enroll_duplicate_mode = scenario in ("enroll_duplicate", "enroll_duplicate_report_loss")
    token = uuid.uuid4().hex[:8]
    work = Path("D:/codex-builds") / f"java-{scenario}-report-{version}-{token}"
    out = OUT_ROOT / f"run-{scenario}-{version}-{token}"
    observations = work / "observations"
    observations.mkdir(parents=True)
    out.mkdir(parents=True)
    env = os.environ | dict(TEMP="D:/Temp", TMP="D:/Temp", PYTHONUTF8="1",
                            PYTHONDONTWRITEBYTECODE="1", CARGO_INCREMENTAL="0",
                            CARGO_HOME=str(ROOT.parent / ".seata-cargo-cache"),
                            CARGO_TARGET_DIR=os.environ.get("SEATA_CARGO_TARGET_DIR",
                                                            "D:/cargo-target/seata-rust-load-driver"))
    for key in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "SEATA_CONFIG_NAME", "SEATA_ENV", "RUSTC", "RUSTDOC"):
        env.pop(key, None)
    cargo = "cargo"
    if version == "1.85":
        rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
        cargo = rust / "cargo.exe"
        env.update(RUSTC=str(rust / "rustc.exe"), RUSTDOC=str(rust / "rustdoc.exe"),
                   RUSTUP_TOOLCHAIN="1.85.0",
                   CARGO_TARGET_DIR=os.environ.get("SEATA_CARGO_TARGET_DIR_MSRV",
                                                   "D:/cargo-target/seata-rust-load-driver-msrv"))
    else:
        env["RUSTUP_TOOLCHAIN"] = "1.99.0"
    jar = guard.reg.ORACLE / "server/target/seata-server.jar"
    assert guard.reg.command(["git", "-C", guard.reg.ORACLE, "rev-parse", "HEAD"], env).decode().strip() == guard.reg.PIN
    assert guard.sha(jar) == guard.reg.JAR_SHA and guard.sha(guard.DRIVER) == guard.DRIVER_SHA
    backend = os.environ.get("SEATA_MYSQL_BACKEND", "docker")
    if mysql_tls:
        assert backend == "desktop", "MySQL TLS fixture requires Docker Desktop"
        tls_material = Path(env["SEATA_JAVA_MYSQL_TLS"]).resolve(strict=True)
    else:
        env.pop("SEATA_JAVA_MYSQL_TLS", None)
    if tm_remote:
        assert backend == "desktop", "remote signer fixture requires Docker Desktop"
    if armed_child_in_flight or (db_fault and db_fault[1] == "in_flight"):
        assert backend == "desktop", "group-commit delay requires a fresh owned MySQL container"
    if backend == "native":
        mysql_binary = Path("D:/seata-mysql-native-8.4.13/mysql-8.4.13-winx64/bin/mysql.exe")
        assert mysql_binary.is_file(), "owned native MySQL client missing"
        env["MYSQL_PWD"] = guard.PASSWORD
        mysql = [mysql_binary, "--no-defaults", "--protocol=TCP", "--host=127.0.0.1",
                 "--port=43318", "--user=root", "-N", "-B", "--raw"]
        facts = guard.reg.command(mysql + ["-e", "SELECT VERSION(), @@port, @@datadir"], env).decode().strip().split("\t")
        assert len(facts) == 3 and facts[0].startswith("8.4.13") and facts[1] == "43318"
        assert Path(facts[2]).as_posix().lower().startswith("d:/seata-mysql-native-8.4.13/data")
        database_runtime = dict(backend="native", version=facts[0], port=int(facts[1]), datadir=facts[2])
    elif backend == "desktop":
        # The caller owns this isolated Docker Desktop container and removes it after the run.
        container = os.environ["SEATA_MYSQL_CONTAINER"]
        docker = ["docker"]
        inspected = json.loads(guard.reg.command(docker + ["inspect", container], env))[0]
        assert inspected["Config"]["Labels"]["seata.fixture.owner"] == "java-tc-report"
        assert inspected["Image"] == guard.IMAGE
        bindings = inspected["NetworkSettings"]["Ports"]["3306/tcp"]
        assert bindings == [{"HostIp": "127.0.0.1", "HostPort": "43318"}]
        mysql = docker + ["exec", "-i", "-e", "MYSQL_PWD=" + guard.PASSWORD, container,
                          "mysql", "-h127.0.0.1", "-P3306", "-uroot", "-N", "-B", "--raw"]
        if mysql_tls:
            mysql += ["--ssl-mode=VERIFY_IDENTITY", "--ssl-ca=/mysql-tls/ca.pem"]
        facts = guard.reg.command(mysql + ["-e", "SELECT VERSION(), @@port"], env).decode().strip().split("\t")
        assert len(facts) == 2 and facts[0].startswith("8.4.13") and facts[1] == "3306"
        database_runtime = dict(backend="desktop", version=facts[0], container=container,
                                container_id=inspected["Id"], image=inspected["Image"])
    else:
        assert backend == "docker", f"unsupported MySQL backend: {backend}"
        docker = ["wsl.exe", "-d", "Ubuntu-20.04", "--exec", "docker", "-H", SOCKET]
        mysql = docker + ["exec", "-i", "-e", "MYSQL_PWD=" + guard.PASSWORD, OWNER,
                          "mysql", "-h127.0.0.1", "-P43317", "-uroot", "-N", "-B", "--raw"]
        image = guard.reg.command(docker + ["inspect", "--format", "{{.Id}} {{index .Config.Labels \"seata-wsl-owner\"}}", OWNER], env).decode().strip().split()
        assert len(image) == 2 and image[1] == LABEL
        database_runtime = dict(backend="docker", container_id=image[0], owner=OWNER, label=LABEL)
    db = f"undo_java_{scenario}_report_{version.replace('.', '')}_{token}"
    guard.reg.command(mysql + ["-e", "CREATE DATABASE " + db], env)
    if enroll_duplicate_mode or witness_query_failure:
        guard.reg.command(mysql + ["-e", "UPDATE performance_schema.setup_consumers SET ENABLED='YES' "
                           "WHERE NAME='events_statements_history_long'"], env)
    server = guard.reg.command(["git", "-C", guard.reg.ORACLE, "cat-file", "blob",
                                    f"{guard.reg.PIN}:script/server/db/mysql.sql"], env)
    client = guard.reg.command(["git", "-C", guard.reg.ORACLE, "cat-file", "blob",
                                    f"{guard.reg.PIN}:script/client/at/db/mysql.sql"], env)
    guard.put(work / "client-schema.sql", client)
    guard.put(out / "server-schema.sql", server)
    guard.put(out / "client-schema.sql", client)
    applied = subprocess.run(mysql + [db], input=server, env=env, cwd=ROOT,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=25)
    assert applied.returncode == 0, applied.stdout[-500:]
    tc, rm, tm = (port() for _ in range(3))
    assert len({tc, rm, tm}) == 3
    resource = f"jdbc:mysql://127.0.0.1:43318/{db}"
    jdbc_options = "useSSL=false&allowPublicKeyRetrieval=true"
    jdbc_user = "root"
    java_tls_args = []
    if mysql_tls:
        from mysql_tls_admission import JAVA_USER

        jdbc_user = JAVA_USER
        jdbc_options = "sslMode=VERIFY_IDENTITY"
        guard.reg.command(mysql + ["-e",
            f"CREATE USER '{jdbc_user}'@'%' IDENTIFIED BY '{guard.PASSWORD}' REQUIRE SSL; "
            f"GRANT ALL ON {db}.* TO '{jdbc_user}'@'%'"], env)
        java_tls_args = ["-Djavax.net.ssl.trustStore=" + str(tls_material / "trust.p12"),
                         "-Djavax.net.ssl.trustStoreType=PKCS12",
                         "-Djavax.net.ssl.trustStorePassword=seata-tls-fixture"]
        guard.save(out / "tls-certificates.json", {name: guard.sha(tls_material / name)
                   for name in ("ca.pem", "wrong-ca.pem", "server.pem")})
    guard.put(work / "application.yml", f"""spring:
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
    service-port: {tc}
    enable-check-auth: true
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
      url: {resource}?{jdbc_options}
      user: {jdbc_user}
      password: {guard.PASSWORD}
      min-conn: 1
      max-conn: 4
""")
    guard.put(out / "application.yml", (work / "application.yml").read_bytes())
    overlay = guard.compile_auth(jar, work, out, env, False)
    target = "live_tm_launcher_java" if tm_durable_mode else "live_commit_check_java"
    build = [cargo, "test", "--offline", "--locked", "-p", "seata-db-mysql"]
    if not no_change and (not tm_durable_mode or tm_at):
        build += ["--features", "experimental_changed_commit"]
    build += ["--test", target, "--no-run", "--message-format=json"]
    artifact_output = guard.reg.command(build, env, timeout=180)
    artifact, = [Path(row["executable"]) for line in artifact_output.splitlines() if line.startswith(b"{")
                 and (row := json.loads(line)).get("executable") and row["target"]["name"] == target]
    guard.put(out / "cargo-build.jsonl", artifact_output)
    env.update(SEATA_MYSQL_PORT="43318", SEATA_MYSQL_PASSWORD=guard.PASSWORD,
               SEATA_MYSQL_DATABASE=db, SEATA_UNDO_SCHEMA_PATH=str(work / "client-schema.sql"),
               SEATA_UNDO_STORE_OUT=str(observations), SEATA_RM_ENDPOINT=f"127.0.0.1:{rm}",
               SEATA_TM_ENDPOINT=f"127.0.0.1:{tm}")
    env.pop("SEATA_CHECKSUM_LEGACY", None)
    if scenario == "undo_checksum_legacy":
        env["SEATA_CHECKSUM_LEGACY"] = "1"
    env.pop("SEATA_CHECKSUM_ROLLBACK", None)
    if scenario == "undo_checksum_rollback":
        env["SEATA_CHECKSUM_ROLLBACK"] = "1"
    if callback_reply_loss:
        env["SEATA_JAVA_AT_CALLBACK_REPLY_LOSS"] = "1"
    else:
        env.pop("SEATA_JAVA_AT_CALLBACK_REPLY_LOSS", None)
    if tm_remote:
        signer_port = port()
        assert signer_port not in (tc, rm, tm)
        signer_name = f"seata-attested-tm-{token}"
        signer_material = Path(env["SEATA_ATTESTED_TEST_MATERIAL"]).resolve(strict=True)
        signer_artifact = Path(env["SEATA_ATTESTED_TEST_LINUX_ARTIFACT"]).resolve(strict=True)
        env.update(SEATA_ATTESTED_TEST_SIGNER_ENDPOINT=f"127.0.0.1:{signer_port}",
                   SEATA_TM_ATTESTATION_CREDENTIALS=str(signer_material / "recovery"))
    stop = work / "stop"
    processes = []
    signer_created = False
    db_observer = None
    commit_gate = None
    try:
        if changed_pool:
            from armed_child_business_commit import start_business_commit_observer

            db_observer = start_business_commit_observer(43318, mysql, env, out, hold_ack=True)
            assert db_observer.port not in (tc, rm, tm)
            env["SEATA_JAVA_RM_MYSQL_PORT"] = str(db_observer.port)
            resource = f"jdbc:mysql://127.0.0.1:{db_observer.port}/{db}"
        elif no_change:
            from mysql_no_change import start_no_change_observer

            db_observer = start_no_change_observer(43318, out, lose_ack=no_change_loss, recycled=pool_recovery)
            assert db_observer.port not in (tc, rm, tm)
            env["SEATA_JAVA_RM_MYSQL_PORT"] = str(db_observer.port)
            resource = f"jdbc:mysql://127.0.0.1:{db_observer.port}/{db}"
        elif business_child_exit:
            if arming_fault:
                from mysql_arming_ack_loss import start_arming_observer

                db_observer = start_arming_observer(43318, mysql, db, env, out, request_blocked=arming_blocked)
            else:
                from armed_child_business_commit import start_business_commit_observer

                before_commit = None
                if wait_commit:
                    from mysql_competing_recovery import create_commit_gate

                    commit_gate, before_commit = create_commit_gate(observations)
                db_observer = start_business_commit_observer(43318, mysql, env, out, hold_ack=armed_child_ack,
                                                             in_flight=armed_child_in_flight, drop_fence_ack=fence_ack_lost,
                                                             before_commit=before_commit)
            assert db_observer.port not in (tc, rm, tm)
            env["SEATA_JAVA_RM_MYSQL_PORT"] = str(db_observer.port)
            resource = f"jdbc:mysql://127.0.0.1:{db_observer.port}/{db}"
        elif report_claim_rollback:
            if report_child_exit:
                from report_claim_child_exit import start_mysql_observation

                db_observer = start_mysql_observation(43318, out)
            else:
                from mysql_report_claim_fault import start_report_claim_observer

                db_observer = start_report_claim_observer(43318, mysql, db, env, out)
            assert db_observer.port not in (tc, rm, tm)
            env["SEATA_JAVA_RM_MYSQL_PORT"] = str(db_observer.port)
            resource = f"jdbc:mysql://127.0.0.1:{db_observer.port}/{db}"
        elif db_fault or mixed_at or missing_undo or terminal_permission:
            from mysql_phase_two_fault import start_observer

            phase, mode = db_fault or (scenario.split("_")[0], "observe")
            db_observer = start_observer(43318, mysql, db, env, out, phase, mode)
            assert db_observer.port not in (tc, rm, tm)
            env["SEATA_JAVA_RM_MYSQL_PORT"] = str(db_observer.port)
            resource = f"jdbc:mysql://127.0.0.1:{db_observer.port}/{db}"
        else:
            env.pop("SEATA_JAVA_RM_MYSQL_PORT", None)
        java = [guard.reg.JAVA, *java_tls_args, "-Xlog:class+load=info:file=class-load.log", "-Djava.io.tmpdir=D:/Temp",
                "-Duser.home=" + str(work), "-Dspring.config.location=file:" + (work / "application.yml").as_posix(),
                "-Dseata.callback.fixture.resource=" + resource,
                "-Dloader.path=" + overlay.as_posix() + "," + guard.DRIVER.as_posix(), "-cp", jar,
                "org.springframework.boot.loader.launch.PropertiesLauncher", "-h", "127.0.0.1"]
        processes.append(guard.support.spawn(java, work, env, out, "tc"))
        guard.support.wait_owned(processes[-1], tc, env)
        for role, listen in (("TM", tm),) if tm_durable_mode and not tm_at else (("RM", rm), ("TM", tm)):
            observer_mode = ("cross_process_race" if role == "RM" and scenario == "commit_distinct_process_race"
                             else "cross_process" if role == "RM" and (business_child_exit or scenario in ("commit_cross_process", "commit_cross_process_race"))
                             else "report_loss" if role == "RM" and report_loss_mode else "basic")
            if role == "TM" and tm_drop_reply:
                observer_mode = "tm_rollback_loss" if tm_rollback else "tm_end_loss"
            if role == "RM" and tm_at:
                observer_mode = "cross_process"
            proxy_path = "tools/formal-tc-lock-coverage/proxy.py"
            if callback_reply_loss and role == "RM":
                observer_mode = "reconnect"
                proxy_path = "tools/formal-rm-java/rm_proxy.py"
            proxy = [sys.executable, ROOT / proxy_path,
                "--listen-port", listen, "--upstream-port", tc, "--events", out / f"{role}-frames.jsonl",
                "--stop-file", stop, "--role", role, "--mode", observer_mode]
            if tm_recovery and role == "TM":
                proxy.append("--supervised-tm")
            processes.append(guard.support.spawn(proxy, work, env, out, role))
            guard.support.wait_owned(processes[-1], listen, env)
        if mysql_tls:
            from mysql_tls_admission import aborted_connects, permit_trusted_ca

            aborted_before = aborted_connects(mysql, env)
        processes.append(guard.support.spawn([artifact, TESTS[scenario], "--ignored", "--exact", "--test-threads=1"],
                                             ROOT, env, out, "client"))
        if scenario in ("undo_checksum", "undo_checksum_rollback"):
            from mysql_checksum_callback import restore_after_callback
            restore_after_callback(processes[-1], observations, mysql, db, env, out)
        if changed_pool:
            from mysql_changed_pool_recovery import permit_changed_recovery

            permit_changed_recovery(processes[-1], observations, mysql, db, env, out, db_observer)
        elif mysql_tls:
            admission = permit_trusted_ca(processes[-1], observations, mysql, db, env, out, aborted_before)
        elif terminal_permission:
            from mysql_terminal_permission import restore_after_denial

            permission = restore_after_denial(processes[-1], observations, mysql, db, env, out)
        elif no_change:
            from mysql_no_change import allow_global_end

            if pool_recovery:
                from mysql_pool_recovery import permit_pool_probe

                permit_pool_probe(processes[-1], observations, mysql, db, env, out, db_observer)
            allow_global_end(processes[-1], observations, mysql, db, env, out, no_change_loss)
        elif arming_fault:
            from mysql_arming_ack_loss import observe_arming_child

            observe_arming_child(artifact, processes[-1], observations, mysql, db, env, out, db_observer,
                                 request_blocked=arming_blocked)
        elif business_child_exit:
            from armed_child_business_commit import terminate_with_original_commit_held

            before_finish = None
            if wait_rollback or wait_commit:
                from functools import partial
                from mysql_competing_recovery import wait_for_competing_recovery

                before_finish = partial(wait_for_competing_recovery, observations, mysql, db, env, out,
                                        commit_gate=commit_gate)
            terminate_with_original_commit_held(artifact, processes[-1], observations, mysql, db, env, out,
                                                db_observer, in_flight=armed_child_in_flight, before_finish=before_finish)
            if wait_rollback or wait_commit:
                from mysql_competing_recovery import save_first_recovery

                save_first_recovery(processes[-1], observations, out)
            if fence_ack_lost:
                from armed_child_business_commit import allow_fence_reobservation

                allow_fence_reobservation(processes[-1], observations, mysql, db, env, out, db_observer)
            if witness_query_failure:
                from mysql_witness_query_failure import allow_witness_reobservation

                allow_witness_reobservation(processes[-1], observations, mysql, db, env, out)
        if report_child_exit:
            from report_claim_child_exit import terminate_at_registration

            terminate_at_registration(artifact, processes[-1], observations, mysql, db, env, out, tc, db_observer)
        if tm_remote:
            until = time.monotonic() + 15
            while not (observations / "attested-schema-ready").is_file():
                assert processes[-1].poll() is None and time.monotonic() < until, "AT recovery schema not ready"
                time.sleep(0.05)
            signer_args = [
                "docker", "run", "-d", "--pull=never", "--read-only", "--cap-drop=ALL",
                "--security-opt", "no-new-privileges", "--tmpfs", "/var/lib/mysql:ro,noexec,nosuid,nodev",
                "--user", "65534:65534", "--name", signer_name, "--label", f"seata.fixture.attestation={token}",
                "-p", f"127.0.0.1:{signer_port}:4400",
                "--mount", f"type=bind,source={signer_material / 'signer'},target=/node-credentials,readonly",
                "--mount", f"type=bind,source={signer_artifact},target=/attest,readonly",
                "--mount", f"type=bind,source={observations},target=/observations",
                "-e", "SEATA_TM_ATTESTATION_CREDENTIALS=/node-credentials",
                "-e", "SEATA_UNDO_STORE_OUT=/observations",
            ]
            for key in ("SEATA_TM_ENDPOINT", "SEATA_RM_ENDPOINT", "SEATA_MYSQL_PORT",
                        "SEATA_MYSQL_PASSWORD", "SEATA_MYSQL_DATABASE"):
                signer_args += ["-e", key]
            if tm_rollback:
                signer_args += ["-e", "SEATA_ATTESTED_TEST_ROLLBACK=1"]
            signer_args += ["--entrypoint", "/attest", guard.IMAGE, "--ignored", "--exact",
                            "--nocapture", "java_tm_attested::signer_node"]
            guard.reg.command(signer_args, env)
            signer_created = True
        guard.save(out / "runtime.json", dict(database_runtime=database_runtime,
                   db=db, java_pin=guard.reg.PIN, jar_sha256=guard.reg.JAR_SHA,
                   connector_sha256=guard.DRIVER_SHA, test_sha256=guard.sha(artifact),
                   ports={"tc": tc, "rm": rm, "tm": tm, "mysql_relay": 43318},
                   pids=[p.pid for p in processes]))
        assert processes[-1].wait(timeout=75) == 0, (out / "client-stdout.log").read_text(errors="replace")
        guard.put(stop, "complete\n")
        if tm_durable_mode:
            assert all(p.wait(timeout=12) == 0 for p in processes[1:-1]), "frame observer did not finish"
            tm_rows = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
            end_kind, reply_kind, operation = (9, 10, 2) if tm_rollback else (7, 8, 1)
            ends = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == end_kind]
            replies = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "s2c" and r["kind"] == reply_kind]
            assert len(ends) == len(replies) == 1
            assert (ends[0]["connection"], ends[0]["request_id"]) == (replies[0]["connection"], replies[0]["request_id"])
            assert replies[0]["action"] == ("drop_original_tm_end_reply" if tm_drop_reply else "forward")
            assert replies[0]["delivery_count"] == int(not tm_drop_reply)
            queries = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == 15]
            observations_from_tc = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "s2c" and r["kind"] == 16]
            if tm_recovery:
                assert [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] in (7, 9)] == ends
                assert len(queries) == len(observations_from_tc) == 2
                begins = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == 1]
                assert len(begins) == 1 and begins[0]["connection"] != ends[0]["connection"]
                end_frame = bytes.fromhex(ends[0]["frame_hex"])
                for query, observation in zip(queries, observations_from_tc):
                    assert query["connection"] == observation["connection"] == ends[0]["connection"]
                    assert query["request_id"] == observation["request_id"]
                    assert bytes.fromhex(query["frame_hex"])[18:] == end_frame[18:]
                    assert bytes.fromhex(observation["frame_hex"])[16:] == bytes([0, 16, 1, 0, 1])
                    assert observation["delivery_count"] == 1 and observation["index"] < ends[0]["index"]
                completed = json.loads((observations / "java-tm-recovered.json").read_bytes())
                assert completed["new_pid"] == processes[-1].pid and completed["old_pid"] > 0
                if not tm_remote:
                    assert completed["old_pid"] != completed["new_pid"]
                assert completed["remote_stop_proof"] == tm_remote
                assert completed["origin_stop_scope_bound"] == tm_remote
                assert completed["unknown"] == tm_drop_reply
                assert completed["old_memory_recovery_denied"] == tm_drop_reply
                assert completed["xid"].encode() == end_frame[20:20 + int.from_bytes(end_frame[18:20], "big")]
                sql = ("SELECT end_operation,send_started,owner_epoch,"
                       "(SELECT COUNT(*) FROM global_table),"
                       "(SELECT COUNT(*) FROM branch_table),"
                       "(SELECT COUNT(*) FROM lock_table) FROM seata_rs_tm_launcher_v1")
                row = guard.reg.command(mysql + [db, "-e", sql], env).decode().strip().split("\t")
                assert row == [str(operation), "1", "2", "0", "0", "0"]
                if tm_at:
                    completed.update(verify_supervised_at(mysql, db, env, out, completed, tm_rollback, resource))
                if tm_remote:
                    assert guard.reg.command(["docker", "wait", signer_name], env, timeout=12).strip() == b"0"
                    log = guard.reg.command(["docker", "logs", signer_name], env)
                    assert b"ATTESTED_SIGNER proof_sent=1 peer=" in log
                    guard.put(out / "signer.log", log)
                    completed.update(signer_container=signer_name, signer_sha256=guard.sha(signer_artifact))
                guard.save(out / "verified.json", dict(
                    scenario=scenario, original_tc_end_requests=1, original_tc_status_requests=2,
                    delivered_end_responses=int(not tm_drop_reply), durable_owner_epoch=2,
                    durable_send_started=1, durable_end_operation=operation, final_tc_counts=[0, 0, 0],
                    database_runtime=database_runtime, **completed))
                print(f"PASS original Java TC supervised recovery {scenario} {version}: {out}", flush=True)
                return
            assert len(queries) == len(observations_from_tc) == 1
            query, observation = queries[0], observations_from_tc[0]
            assert (query["connection"], query["request_id"]) == (observation["connection"], observation["request_id"])
            assert query["connection"] != ends[0]["connection"]
            result = json.loads((observations / "java-tm-durable-loss.json").read_bytes())
            assert result["unknown"] and result["old_memory_recovery_denied"] and result["send_started"] == 1 and result["end_operation"] == operation
            query_frame = bytes.fromhex(query["frame_hex"])
            end_frame = bytes.fromhex(ends[0]["frame_hex"])
            status_frame = bytes.fromhex(observation["frame_hex"])
            assert query_frame[18:] == end_frame[18:], "Status must query the same XID as End"
            assert status_frame[16:] == bytes([0, 16, 1, 0, result["observed_status_code"]])
            assert observation["delivery_count"] == 1
            row = guard.reg.command(mysql + [db, "-e", "SELECT end_operation,send_started,owner_epoch FROM seata_rs_tm_launcher_v1"], env).decode().strip().split("\t")
            assert row == [str(operation), "1", "1"]
            guard.save(out / "verified.json", dict(scenario=scenario, xid=result["xid"], original_tc_end_requests=1,
                        original_tc_end_success_replies=1, delivered_replies=0, durable_end_operation=operation,
                        durable_send_started=1, old_memory_recovery_denied=True, database_runtime=database_runtime,
                        observed_status=result["observed_status"], original_tc_status_requests=1))
            print(f"PASS original Java TC durable TM {scenario} {version}: {out}", flush=True)
            return
        assert all(p.wait(timeout=12) == 0 for p in processes[1:3]), "frame observers did not finish"
        if scenario == "undo_checksum_rollback":
            result = json.loads((observations / "java-checksum-rollback.json").read_bytes())
            journal = (observations / "java-changed-report-branches.log").read_bytes()
            guard.save(out / "observation.json", result)
            guard.put(out / "branches.log", journal)
            verified = verify_at_retried_callback(mysql, db, env, out, result, resource, journal,
                rollback=True, callback_reply_loss=False, db_fault=True)
            guard.save(out / "verified.json", dict(scenario=scenario, **verified))
            print(f"PASS original Java TC checksum rollback {version}: {out}", flush=True)
            return
        if terminal_permission:
            from mysql_terminal_permission import verify_permission_commit

            terminal_sql = db_observer.terminal_sql
            mysql_wire = db_observer.stop()
            db_observer = None
            guard.save(out / "mysql-wire.json", mysql_wire)
            result = json.loads((observations / "java-changed-report.json").read_bytes())
            journal = (observations / "java-changed-report-branches.log").read_bytes()
            guard.save(out / "observation.json", result)
            guard.put(out / "branches.log", journal)
            verified = verify_at_retried_callback(mysql, db, env, out, result, resource, journal,
                                                  rollback=False, callback_reply_loss=False, db_fault=True)
            verified.update(verify_permission_commit(mysql, db, env, out, mysql_wire, verified, permission, terminal_sql))
            guard.save(out / "verified.json", dict(scenario=scenario, database_runtime=database_runtime, **verified))
            print(f"PASS original Java TC with a MySQL terminal permission failure {version}: {out}", flush=True)
            return
        if mysql_tls:
            from mysql_tls_admission import verify_original_tls_transaction

            result = json.loads((observations / "java-changed-report.json").read_bytes())
            journal = (observations / "java-changed-report-branches.log").read_bytes()
            guard.save(out / "observation.json", result)
            guard.put(out / "branches.log", journal)
            verified = verify_at_retried_callback(mysql, db, env, out, result, resource, journal,
                                                  rollback=False, callback_reply_loss=False, single_commit=True)
            verified.update(verify_original_tls_transaction(mysql, db, env, out, result, admission))
            guard.save(out / "verified.json", dict(scenario=scenario, database_runtime=database_runtime, **verified))
            print(f"PASS original Java TC with verified MySQL TLS {version}: {out}", flush=True)
            return
        if changed_pool:
            from mysql_changed_pool_recovery import verify_changed_pool

            mysql_wire = db_observer.stop()
            db_observer = None
            guard.save(out / "mysql-wire.json", mysql_wire)
            result = json.loads((observations / "java-changed-report-recovered.json").read_bytes())
            guard.save(out / "observation.json", result)
            journal = (observations / "java-changed-report-branches.log").read_bytes()
            guard.put(out / "branches.log", journal)
            verified = verify_at_retried_callback(mysql, db, env, out, result, resource, journal,
                                                  rollback=False, callback_reply_loss=False, single_commit=True)
            verified.update(verify_changed_pool(mysql, db, env, out, mysql_wire, result))
            guard.save(out / "verified.json", dict(scenario=scenario, database_runtime=database_runtime, **verified))
            print(f"PASS original Java TC AT changed pool recovery {version}: {out}", flush=True)
            return
        if no_change:
            from mysql_no_change import verify_no_change

            mysql_wire = db_observer.stop()
            db_observer = None
            guard.save(out / "mysql-wire.json", mysql_wire)
            ended = json.loads((observations / "java-no-change-ended.json").read_bytes())
            guard.save(out / "no-change-ended.json", ended)
            verified = verify_no_change(mysql, db, env, out, mysql_wire, ended,
                                        (observations / "java-no-change-branches.log").read_bytes(),
                                        lose_ack=no_change_loss, artifact=artifact, recycled=pool_recovery)
            if pool_recovery:
                from mysql_pool_recovery import verify_pool_probe

                verified.update(verify_pool_probe(out, mysql_wire, ended))
            guard.save(out / "verified.json", dict(scenario=scenario, database_runtime=database_runtime, **verified))
            print(f"PASS original Java TC no-change {scenario} {version}: {out}", flush=True)
            return
        if orphan_rollback or callback_reply_loss or db_fault or dirty_rollback or mixed_at or missing_undo or report_claim_rollback or business_child_exit:
            rollback = db_fault[0] == "rollback" if db_fault else scenario != "commit_callback_reply_loss" and not armed_child_ack
            if mixed_at:
                rollback = scenario == "rollback_mixed"
            observation = ("java-missing-undo.json" if missing_undo
                           else "java-armed-child-recovered.json" if armed_child
                           else "java-applied-child-recovered.json" if armed_child_ack
                           else "java-report-claim-loss.json" if report_claim_loss
                           else "java-report-claim-child-exit.json" if report_child_exit
                           else "java-undo-mixed.json" if mixed_at
                           else "java-undo-dirty.json" if dirty_rollback
                           else "java-undo-orphan.json" if orphan_rollback
                           else "java-rollback-callback-loss.json" if rollback
                           else "java-changed-report.json")
            result = json.loads((observations / observation).read_bytes())
            if orphan_rollback:
                assert result["existing_guard"] == (scenario == "rollback_orphan_guarded_root")
            guard.save(out / "observation.json", result)
            journal_name = ("java-branches.log" if missing_undo else
                            "java-cross-process-parent-branches.log" if business_child_exit else
                            "java-changed-report-branches.log")
            journal = (observations / journal_name).read_bytes()
            guard.put(out / "branches.log", journal)
            verified = verify_at_retried_callback(mysql, db, env, out, result, resource, journal,
                                                  rollback=rollback, callback_reply_loss=callback_reply_loss,
                                                  db_fault=bool(db_fault), dirty_rollback=dirty_rollback,
                                                  mixed_at=mixed_at, missing_undo=missing_undo,
                                                  report_claim_rollback=report_claim_rollback,
                                                  armed_child=armed_child, armed_child_ack=armed_child_ack)
            if db_observer:
                mysql_wire = db_observer.stop()
                db_observer = None
                guard.save(out / "mysql-wire.json", mysql_wire)
                if arming_fault:
                    from mysql_arming_ack_loss import verify_arming_fault

                    verified.update(verify_arming_fault(mysql, db, env, out, mysql_wire, result,
                                                        request_blocked=arming_blocked))
                elif armed_child:
                    from armed_child_business_commit import verify_blocked_business_commit

                    verified.update(verify_blocked_business_commit(mysql, db, env, out, mysql_wire, result,
                                                                  fence_ack_lost=fence_ack_lost, competing=wait_rollback))
                    if witness_query_failure:
                        from mysql_witness_query_failure import verify_witness_query_failure

                        verified.update(verify_witness_query_failure(out, mysql_wire, result))
                elif armed_child_ack:
                    from armed_child_business_commit import verify_applied_business_commit

                    verified.update(verify_applied_business_commit(mysql, db, env, out, mysql_wire, result,
                                                                  in_flight=armed_child_in_flight, competing=wait_commit))
                elif report_child_exit:
                    from report_claim_child_exit import verify_child_exit

                    verified.update(verify_child_exit(mysql, db, env, out, mysql_wire, result))
                elif report_claim_loss:
                    from mysql_report_claim_fault import verify_report_claim_loss

                    verified.update(verify_report_claim_loss(mysql, db, env, out, mysql_wire, result))
                elif missing_undo:
                    from mysql_missing_undo import verify_missing_undo

                    verified.update(verify_missing_undo(mysql, db, env, out, mysql_wire, result))
                elif mixed_at:
                    from mysql_mixed_undo import verify_mixed_sql

                    verified.update(verify_mixed_sql(mysql, db, env, out, mysql_wire, result, rollback))
                else:
                    from mysql_phase_two_fault import verify_mysql_fault

                    verified.update(verify_mysql_fault(mysql_wire, out, result, rollback))
            if wait_commit or wait_rollback:
                from mysql_competing_recovery import verify_competing_recovery

                verified.update(verify_competing_recovery(out, mysql_wire, result, committed=wait_commit))
            guard.save(out / "verified.json", dict(scenario=scenario, database_runtime=database_runtime, **verified))
            print(f"PASS original Java TC AT phase two {scenario} {version}: {out}", flush=True)
            return
        observation_name = {"undo_checksum_legacy": "java-changed-report-recovered.json", "undo_checksum": "java-changed-report-recovered.json", "commit": "java-changed-report.json",
                            "commit_recovered": "java-changed-report-recovered.json",
                            "commit_cross_process": "java-cross-process-recovered.json",
                            "commit_cross_process_race": "java-cross-process-race.json",
                            "commit_distinct_process_race": "java-distinct-process-race.json",
                            "commit_report_loss": "java-changed-report-loss.json",
                            "failed": "java-failed-report.json",
                            "report_loss": "java-failed-report-loss.json",
                            "undo_error": "java-prepare-undo-failed.json",
                            "undo_report_loss": "java-prepare-undo-report-loss.json",
                            "undo_duplicate": "java-prepare-undo-duplicate.json",
                            "enroll_error": "java-prepare-enrollment-failed.json",
                            "enroll_duplicate": "java-prepare-enrollment-duplicate.json",
                            "enroll_duplicate_report_loss": "java-prepare-enrollment-duplicate-report-loss.json"}[scenario]
        if scenario == "undo_checksum_legacy":
            guard.save(out / "checksum-legacy.json", json.loads((observations / "java-checksum-legacy.json").read_bytes()))
        result = json.loads((observations / observation_name).read_bytes())
        guard.save(out / "observation.json", result)
        sql = mysql + [db, "-e"]
        final = guard.reg.command(sql + ["SELECT (SELECT COUNT(*) FROM global_table),"
            "(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),"
            "(SELECT COUNT(*) FROM undo_work),(SELECT COUNT(*) FROM undo_log),"
            "COALESCE((SELECT first_phase_committed FROM seata_rs_at_branch_v1),0),"
            "COALESCE((SELECT commit_terminal FROM seata_rs_at_branch_v1),0)"], env)
        guard.put(out / "final-counts.tsv", final)
        expected_counts = (["0", "0", "0", "1", "0", "1", "1"] if commit_mode
                           else ["0", "0", "0", "0", "1", "0", "0"] if scenario == "undo_duplicate"
                           else ["0"] * 7)
        assert final.decode().strip().split("\t") == expected_counts
        if commit_mode:
            attempt_rows = guard.reg.command(sql + [
                "SELECT HEX(xid),branch_id,HEX(resource_id),HEX(lock_key),attempt_state,report_started "
                "FROM seata_rs_at_commit_attempt"], env)
            guard.put(out / "commit-attempt.tsv", attempt_rows)
            attempt = attempt_rows.decode().strip().split("\t")
            assert attempt == [result["xid"].encode().hex().upper(), str(result["branch_id"]),
                               result["resource_id"].encode().hex().upper(),
                               b"undo_work:1".hex().upper(), "1", "1"]
            assert result["attempt_rows"] == 1
            if scenario in ("commit_recovered", "commit_cross_process", "commit_cross_process_race", "commit_distinct_process_race"):
                assert result["recovery_outcome"] == "Applied"
            if scenario in ("commit_cross_process", "commit_cross_process_race", "commit_distinct_process_race"):
                assert result["child_exited_before_recovery"] is True
            if scenario in ("commit_cross_process_race", "commit_distinct_process_race"):
                assert result["concurrent_recovery_callers"] is True
            if scenario == "commit_distinct_process_race":
                assert result["distinct_recovery_processes"] is True
                child_pids = result["recovery_child_pids"]
                assert len(child_pids) == len(set(child_pids)) == 2
                assert result["recovery_claim_outcomes"] == [1, 2]
        rm_rows = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
        tm_rows = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
        reports = [r for r in rm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == 13]
        registers = [r for r in rm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == 11]
        end_kind = 7 if commit_mode else 9
        ends = [r for r in tm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == end_kind]
        assert len(reports) == len(registers) == len(ends) == 1
        replies = [r for r in rm_rows if r.get("event") == "frame" and r["direction"] == "s2c"
                   and r["kind"] == 14 and r["request_id"] == reports[0]["request_id"]
                   and r["connection"] == reports[0]["connection"]]
        assert len(replies) == 1 and guard.support.payload(replies[0])[2:] == b"\x01\x00"
        if scenario == "commit_distinct_process_race":
            callbacks = [r for r in rm_rows if r.get("event") == "frame" and r["direction"] == "s2c" and r["kind"] == 3]
            callback_replies = [r for r in rm_rows if r.get("event") == "frame" and r["direction"] == "c2s" and r["kind"] == 4]
            assert len(callbacks) == len(callback_replies) == 1
            assert (callback_replies[0]["connection"], callback_replies[0]["request_id"]) == (
                callbacks[0]["connection"], callbacks[0]["request_id"])
            assert guard.support.payload(callback_replies[0])[2:4] == b"\x01\x00"
        expected_action = "drop_original_report_reply" if report_loss_mode else "forward"
        assert replies[0]["action"] == expected_action
        data = guard.support.payload(reports[0])
        xid, at = guard.support.string(data, 2)
        branch = struct.unpack(">q", data[at:at+8])[0]
        status = data[at+8]
        resource_wire, _ = guard.support.string(data, at+9)
        expected_status = 2 if commit_mode else 3
        assert (xid, branch, status, resource_wire) == (result["xid"], result["branch_id"], expected_status, result["resource_id"])
        if scenario in ("failed", "report_loss"):
            assert result["before_report"] == {"business": 0, "normal_undo": 0, "tc_lock": 1}
            assert result["global_result"] == "Rollbacked"
            if scenario == "report_loss":
                assert result["report_response"] == "Lost" and result["original_receipt_preserved"]
        if scenario == "commit_report_loss":
            assert (result["report_response"], result["local_commit_confirmed"],
                    result["original_receipt_preserved"], result["global_result"],
                    result["callback"]) == ("Lost", True, True, "Committed", "Completed")
            assert result["before_commit"] == {"business": 1, "normal_undo": 1,
                                               "witness": 1, "tc_lock": 1}
        if scenario in ("undo_error", "undo_report_loss", "undo_duplicate", "enroll_error", "enroll_duplicate", "enroll_duplicate_report_loss"):
            expected_prepare_error = ("EnrollmentDuplicate" if enroll_duplicate_mode else
                                      "Enrollment" if scenario == "enroll_error" else
                                      "UndoDuplicate" if scenario == "undo_duplicate" else "Undo")
            assert (result["prepare_error"], result["local_rollback_confirmed"],
                    result["failure_report_started"], result["report_result"]) == (expected_prepare_error, True, True,
                                                                                      "Lost" if report_loss_mode else "Success")
            if scenario in ("undo_report_loss", "enroll_duplicate_report_loss"):
                assert result["original_receipt_preserved"] and result["report_replay_rejected"]
            assert result["control_rows"] == (0 if scenario == "enroll_error" or enroll_duplicate_mode else 1)
            assert result["undo_rows"] == int(scenario == "undo_duplicate")
            if scenario == "undo_duplicate" or enroll_duplicate_mode:
                assert result["driver_code"] == 1062
        if enroll_duplicate_mode:
            raw_error = guard.reg.command(mysql + ["-e", "SELECT THREAD_ID,EVENT_ID,EVENT_NAME,MYSQL_ERRNO,RETURNED_SQLSTATE,CURRENT_SCHEMA "
                "FROM performance_schema.events_statements_history_long "
                f"WHERE CURRENT_SCHEMA='{db}' AND MYSQL_ERRNO=1062 "
                "AND RETURNED_SQLSTATE='23000' AND EVENT_NAME='statement/com/Execute' ORDER BY EVENT_ID"], env)
            guard.put(out / "mysql-error-1062.tsv", raw_error)
            assert raw_error.strip(), "original control enrollment INSERT did not record MySQL 1062"
        guard.save(out / "verified.json", dict(xid=xid, branch_id=branch,
                   phase_one_done_reports=int(commit_mode), phase_one_failed_reports=int(not commit_mode),
                   phase_one_report_success_responses=1,
                   report_response_delivered=int(not report_loss_mode),
                   original_registrations=1, global_commits=int(commit_mode),
                   global_rollbacks=int(not commit_mode),
                    original_tc_callback_completed=commit_mode, final_counts=expected_counts,
                    recovery_child_pids=result.get("recovery_child_pids", []),
                    recovery_claim_outcomes=result.get("recovery_claim_outcomes", [])))
        print(f"PASS original Java TC {scenario} report {version}: {out}", flush=True)
    finally:
        if commit_gate is not None:
            commit_gate.set()
        if not stop.exists():
            guard.put(stop, "cleanup\n")
        for process in reversed(processes):
            guard.support.stop(process)
        if db_observer:
            guard.save(out / "mysql-wire.json", db_observer.stop())
        if signer_created:
            label = guard.reg.command(["docker", "inspect", "--format",
                '{{index .Config.Labels "seata.fixture.attestation"}}', signer_name], env).decode().strip()
            assert label == token
            if not (out / "signer.log").exists():
                guard.put(out / "signer.log", guard.reg.command(["docker", "logs", signer_name], env))
            guard.reg.command(["docker", "rm", "-fv", signer_name], env)
        guard.save(out / "process-cleanup.json", {"stopped": all(p.poll() is not None for p in processes),
                                                  "signer_removed": signer_created,
                                                  "pids": [p.pid for p in processes]})


def verify_supervised_at(mysql, db, env, out, result, rollback, resource):
    frames = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]

    def find(direction, kinds):
        return [r for r in frames if r.get("event") == "frame" and r["direction"] == direction and r["kind"] in kinds]

    registration, = find("c2s", (11,))
    report, = find("c2s", (13,))
    report_reply, = find("s2c", (14,))
    callback, = find("s2c", (3, 5))
    reply, = find("c2s", (4, 6))
    assert registration["connection"] == report["connection"] == report_reply["connection"]
    assert report["request_id"] == report_reply["request_id"]
    assert guard.support.payload(report_reply)[2:] == b"\x01\x00"
    payload = guard.support.payload(report)
    xid, at = guard.support.string(payload, 2)
    branch = struct.unpack_from(">q", payload, at)[0]
    report_resource, _ = guard.support.string(payload, at + 9)
    assert (xid, branch, payload[at + 8], report_resource) == (result["xid"], result["branch_id"], 2, resource)
    assert callback["kind"] == (5 if rollback else 3) and reply["kind"] == callback["kind"] + 1
    assert callback["connection"] == reply["connection"] != registration["connection"]
    assert callback["request_id"] == reply["request_id"]
    payload = guard.support.payload(callback)
    callback_xid, at = guard.support.string(payload, 2)
    callback_branch = struct.unpack_from(">q", payload, at)[0]
    callback_resource, _ = guard.support.string(payload, at + 9)
    assert (callback_xid, callback_branch, payload[at + 8], callback_resource) == (xid, branch, 0, resource)
    assert guard.support.payload(reply)[2:4] == b"\x01\x00"
    state = verify_at_terminal(mysql, db, env, out, rollback)
    return dict(original_tc_branch_callbacks=1, phase_one_done_reports=1,
                final_business_rows=state[0], final_undo_rows=state[1],
                final_normal_undo_rows=state[2], final_rollback_fences=state[3],
                at_terminal_state=state[4:])


def verify_at_terminal(mysql, db, env, out, rollback, business_rows=None):
    sql = ("SELECT (SELECT COUNT(*) FROM undo_work),"
           "(SELECT COUNT(*) FROM undo_log),"
           "(SELECT COUNT(*) FROM undo_log WHERE log_status=0),"
           "(SELECT COUNT(*) FROM undo_log WHERE log_status=1 AND HEX(rollback_info)='7B7D' "
           "AND context='serializer=jackson&compressorType=NONE' "
           "AND CAST(undo_log.xid AS BINARY)=seata_rs_at_branch_v1.xid "
           "AND undo_log.branch_id=seata_rs_at_branch_v1.branch_id),"
           "phase,terminal,commit_terminal FROM seata_rs_at_branch_v1")
    raw = guard.reg.command(mysql + [db, "-e", sql], env)
    guard.put(out / "at-terminal.tsv", raw)
    state = [int(value) for value in raw.decode().strip().split("\t")]
    expected = [0, 1, 0, 1, 2, 1, 0] if rollback else [1, 0, 0, 0, 1, 0, 1]
    if business_rows is not None:
        expected[0] = business_rows
    assert state == expected
    return state


def verify_at_retried_callback(mysql, db, env, out, result, resource, journal, *,
                               rollback, callback_reply_loss, db_fault=False, dirty_rollback=False,
                               mixed_at=False, missing_undo=False, report_claim_rollback=False,
                               armed_child=False, armed_child_ack=False, single_commit=False):
    business_child_exit = armed_child or armed_child_ack
    no_report_rollback = report_claim_rollback or armed_child
    spec = importlib.util.spec_from_file_location("at_callback_wire", ROOT / "tools/formal-rm-java/validate.py")
    wire = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(wire)

    def frames(role):
        rows = [json.loads(line) for line in (out / f"{role}-frames.jsonl").read_text().splitlines()]
        dropped = int(callback_reply_loss and role == "RM")
        assert rows[-1]["event"] == "summary" and rows[-1]["succeeded"] and rows[-1]["faults"] == dropped
        assert sum(row["event"] == "fault_close" for row in rows) == dropped
        assert all(row["index"] == index for index, row in enumerate(rows))
        rows = [wire.envelope(r) for r in rows if r.get("event") == "frame"]
        assert sum(r["action"] == "drop_and_close" for r in rows) == dropped
        assert all(r["action"] in ("forward", "drop_and_close") for r in rows)
        return rows

    def select(rows, kind, direction):
        return [r for r in rows if r["kind"] == kind and r["direction"] == direction]

    def correlation(frame):
        return frame["connection"], frame["request_id"]

    xid, branch = result["xid"], result["branch_id"]
    assert resource == result["resource_id"]
    rm, tm = frames("RM"), frames("TM")
    registration, = select(rm, 11, "c2s")
    registered, = select(rm, 12, "s2c")
    assert correlation(registration) == correlation(registered)
    assert guard.support.payload(registered)[2:] == b"\x01\x00" + struct.pack(">q", branch)
    registered_identity = guard.support.branch_identity(registration, False)
    assert (registered_identity["xid"], registered_identity["resource"], registered_identity["lock"],
            registered_identity["branch_type"]) == (xid, resource, result["lock_key"] if mixed_at else "undo_work:1", 0)
    reports, acknowledgments = select(rm, 13, "c2s"), select(rm, 14, "s2c")
    previous = registered["index"]
    if missing_undo or no_report_rollback:
        assert not reports and not acknowledgments
    else:
        report, = reports
        reported, = acknowledgments
        assert correlation(report) == correlation(reported)
        data = guard.support.payload(report)
        report_xid, at = guard.support.string(data, 2)
        assert (report_xid, struct.unpack_from(">q", data, at)[0], data[at + 8],
                guard.support.string(data, at + 9)[0]) == (xid, branch, 2, resource)
        assert guard.support.payload(reported)[2:] == b"\x01\x00"
        previous = reported["index"]
    callback_kind, opposite_callback = (5, 3) if rollback else (3, 5)
    callbacks, replies = select(rm, callback_kind, "s2c"), select(rm, callback_kind + 1, "c2s")
    assert len(callbacks) == len(replies)
    if armed_child:
        # 固定原 TC 的回滚重试任务可以与主动回滚并行投递同一分支。
        assert callbacks and all(callback["body"] == callbacks[0]["body"] for callback in callbacks)
        by_request = {correlation(reply): reply for reply in replies}
        assert set(by_request) == {correlation(callback) for callback in callbacks}
        replies = [by_request[correlation(callback)] for callback in callbacks]
    elif (dirty_rollback or mixed_at or no_report_rollback or armed_child_ack or single_commit
          or (missing_undo and not callback_reply_loss)):
        assert len(callbacks) == 1
    else:
        assert len(callbacks) >= 2
    assert len({correlation(r) for r in callbacks}) == len(callbacks)
    assert not select(rm, opposite_callback, "s2c")
    if business_child_exit:
        replacement = callbacks[0]["connection"]
        assert replacement != registration["connection"]
        request, = [row for row in select(rm, 103, "c2s") if row["connection"] == replacement]
        reply, = [row for row in select(rm, 104, "s2c") if row["connection"] == replacement]
        assert correlation(request) == correlation(reply)
        assert guard.support.payload(reply)[2] == 1
        assert all(callback["connection"] == replacement for callback in callbacks)
        assert registered["index"] < request["index"] < reply["index"] < callbacks[0]["index"]
        if armed_child_ack:
            assert report["connection"] == replacement and reply["index"] < report["index"]
    else:
        assert callbacks[0]["connection"] == registration["connection"]
    if callback_reply_loss:
        assert len(callbacks) == 2 and callbacks[0]["connection"] != callbacks[1]["connection"]
        reconnect = callbacks[1]["connection"]
        requests = [r for r in select(rm, 103, "c2s") if r["connection"] == reconnect]
        acknowledgments = [r for r in select(rm, 104, "s2c") if r["connection"] == reconnect]
        request, = requests
        acknowledgment, = acknowledgments
        assert correlation(request) == correlation(acknowledgment)
        assert guard.support.payload(acknowledgment)[2] == 1
        assert replies[0]["index"] < request["index"] < acknowledgment["index"] < callbacks[1]["index"]
        assert [reply["action"] for reply in replies] == ["drop_and_close", "forward"]
    elif not business_child_exit:
        assert all(r["connection"] == registration["connection"] for r in callbacks)
    statuses = []
    for callback, reply in zip(callbacks, replies, strict=True):
        assert correlation(callback) == correlation(reply)
        assert previous < callback["index"] < reply["index"]
        if not armed_child:
            previous = reply["index"]
        target = guard.support.branch_identity(callback, True)
        assert (target["xid"], target["branch_id"], target["resource"], target["branch_type"],
                target["data"]) == (xid, branch, resource, 0, "")
        data = guard.support.payload(reply)
        reply_xid, at = guard.support.string(data, 4)
        assert data[2:4] == b"\x01\x00" and len(data) == at + 9
        assert (reply_xid, struct.unpack_from(">q", data, at)[0]) == (xid, branch)
        statuses.append(data[-1])
    terminal_status = 8 if rollback else 5
    retryable_status = 9 if rollback else 6
    assert statuses == ([10] if dirty_rollback else [terminal_status] * len(callbacks)
                        if callback_reply_loss or armed_child
                        else [retryable_status] * (len(callbacks) - 1) + [terminal_status])
    begin, = select(tm, 2, "s2c")
    end_kind, opposite_end = (9, 7) if rollback else (7, 9)
    end, = select(tm, end_kind, "c2s")
    end_replies = select(tm, end_kind + 1, "s2c")
    assert len(select(tm, 1, "c2s")) == 1 and not select(tm, opposite_end, "c2s")
    assert guard.support.string(guard.support.payload(begin), 4)[0] == xid
    assert guard.support.string(guard.support.payload(end), 2)[0] == xid
    observed_end = (result["tm_status"] if dirty_rollback
                    else result["observed_end"] if rollback and (callback_reply_loss or db_fault or missing_undo or no_report_rollback)
                    else result["end_status"] if mixed_at else 5 if rollback else 9)
    if observed_end is None:
        assert not end_replies
    else:
        ended, = end_replies
        assert correlation(end) == correlation(ended)
        assert guard.support.payload(ended)[2:] == bytes((1, 0, observed_end))
        if rollback:
            assert callbacks[0]["monotonic_ns"] < ended["monotonic_ns"]
    assert end["monotonic_ns"] < callbacks[0]["monotonic_ns"]
    if dirty_rollback:
        assert observed_end == 12 and not select(tm, 15, "c2s")
        return verify_dirty_rollback(mysql, db, env, out, result, wire, journal, callbacks[0])
    query = select(tm, 15, "c2s")[-1]
    finished = select(tm, 16, "s2c")[-1]
    assert correlation(query) == correlation(finished)
    assert guard.support.string(guard.support.payload(query), 2)[0] == xid
    assert guard.support.payload(finished)[2:] == b"\x01\x00\x0f"
    assert max(reply["monotonic_ns"] for reply in replies) < finished["monotonic_ns"]
    if not (callback_reply_loss or db_fault or mixed_at or missing_undo or no_report_rollback
            or armed_child_ack or single_commit):
        assert result["before"] == result["after"] and result["first_recovery_state"] == [2, 0, 1]
        rows = result["before"]
        assert len(rows) == 1 + int(result["existing_guard"])
        orphan = rows[-1]
        assert orphan[:3] == [(1 << 63) - 1, xid, f"branchId={branch}"] and orphan[3] and orphan[4] == 0
        if result["existing_guard"]:
            assert rows[0][:5] == [branch, xid, "serializer=jackson&compressorType=NONE", [123, 125], 1]
        assert (result["first_journal_state"], result["final_journal_state"]) == ("Pending", "Completed")
    journal_states = [row["state"] for row in wire.journal_records(journal, callbacks[0]["body"])]
    if no_report_rollback or armed_child_ack or single_commit:
        assert journal_states == [1, 3]
    state = verify_at_terminal(mysql, db, env, out, rollback,
                               business_rows=(3 if rollback else 4) if mixed_at else None)
    raw = guard.reg.command(mysql + [db, "-e", "SELECT (SELECT COUNT(*) FROM global_table),"
        "(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table)"], env)
    guard.put(out / "final-tc-counts.tsv", raw)
    assert raw.strip().split(b"\t") == [b"0"] * 3
    return dict(xid=xid, branch_id=branch, callback_statuses=statuses, original_global_rollbacks=int(rollback),
                original_global_commits=int(not rollback), callback_replies_delivered=[r["delivery_count"] for r in replies],
                callback_connections=[r["connection"] for r in callbacks],
                observed_global_end_status=observed_end,
                phase_one_done_reports=len(reports), journal_states=journal_states, at_terminal_state=state,
                final_tc_counts=[0, 0, 0])


def verify_dirty_rollback(mysql, db, env, out, result, wire, journal, callback):
    assert result["before"] == result["after"] and result["branch_state"] == [2, 0, 0, 1]
    assert result["business"] == "external_change"
    root, = result["before"]
    xid, branch = result["xid"], result["branch_id"]
    assert tuple(root[:2]) == (branch, xid) and root[4] == 0
    records = wire.journal_records(journal, callback["body"], expected_states=(1,))
    raw = guard.reg.command(mysql + [db, "-e",
        "SELECT HEX(b.xid),b.branch_id,b.phase,b.terminal,b.commit_terminal,b.first_phase_committed,"
        "(SELECT v FROM undo_work WHERE id=1),"
        "u.context,HEX(u.rollback_info),u.log_status,CAST(u.log_created AS CHAR),CAST(u.log_modified AS CHAR) "
        "FROM seata_rs_at_branch_v1 b JOIN undo_log u ON CAST(u.xid AS BINARY)=b.xid "
        "AND u.branch_id=b.branch_id"], env)
    guard.put(out / "dirty-at-state.tsv", raw)
    assert raw.decode().strip().split("\t") == [xid.encode().hex().upper(), str(branch),
        "2", "0", "0", "1", "external_change", root[2], bytes(root[3]).hex().upper(), "0", root[5], root[6]]
    tc_log = (out / "tc-stdout.log").read_text(encoding="utf-8", errors="replace")
    assert any(xid in line and str(branch) in line and "fail and stop retry" in line
               for line in tc_log.splitlines())
    return dict(xid=xid, branch_id=branch, callback_statuses=[10], original_global_rollbacks=1,
                observed_global_end_status=12, journal_states=[r["state"] for r in records],
                original_undo_preserved=True, branch_state=result["branch_state"])


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3) or sys.argv[1] not in ("1.99", "1.85") or (len(sys.argv) == 3 and sys.argv[2] not in TESTS):
        raise SystemExit("Usage: java_tc_report.py {1.99|1.85} [scenario from TESTS]")
    run(sys.argv[1], sys.argv[2] if len(sys.argv) == 3 else "commit")
