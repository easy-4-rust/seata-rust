"""Observe an original Java owner and the Rust contender on one MySQL row."""

import json
import os
from pathlib import Path
import struct
import subprocess
import time

from live import DRIVER, ROOT, module, need, put, reg, save, sha, support

ACTOR = ROOT / "tools/java-go-at-shared-lock"
STAGES = ("java-held-rust-conflict", "java-released-rust-admitted")
BOUND_STAGES = (STAGES[0], "tinyint-binding-unset", STAGES[1])
MISMATCH_STAGE = "tinyint-mismatched-profiles"
DECIMAL_KEYS = {
    "decimal-plain": dict(precision=38, scale=8, pk="9007199254740993.12500000",
                         target="9007199254740993.12500000",
                         lower="9007199254740993.12499999", upper="9007199254740993.12500001"),
    "decimal-scientific": dict(precision=30, scale=20, pk="1.0000000000000E-7",
                              target="0.00000010000000000000",
                              lower="0.00000009999999999999", upper="0.00000010000000000001"),
}


class AccountCase:
    def __init__(self, key_shape):
        self.key_shape = key_shape
        self.composite = key_shape == "composite"
        self.binary = key_shape == "binary"
        self.tinyint = key_shape.startswith("tinyint-")
        self.decimal = DECIMAL_KEYS.get(key_shape)
        if self.decimal:
            self.pk = self.decimal["pk"]
        else:
            self.pk = {"integer": "1", "composite": "north_9007199254740993", "binary": "[-1]",
                       "tinyint-boolean": "true", "tinyint-numeric": "2", "tinyint-mismatch": "1"}[key_shape]
        self.lock_key = f"accounts:{self.pk}"
        if self.composite:
            self.select_business = "SELECT id,segment,balance FROM accounts ORDER BY id,segment"
        elif self.binary:
            self.select_business = "SELECT HEX(id),balance FROM accounts ORDER BY id"
        else:
            self.select_business = "SELECT id,balance FROM accounts ORDER BY id"
        self.select_state = ("SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table;"
                             + self.select_business + ";"
                             "SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log")

    def business_rows(self, balance):
        if self.composite:
            return [f"9007199254740993\tnorth\t{balance}",
                    "9007199254740993\tsouth\t2000", "9007199254740994\tnorth\t3000"]
        if self.decimal:
            values = self.decimal
            return [f"{values['lower']}\t2000", f"{values['target']}\t{balance}", f"{values['upper']}\t3000"]
        if self.binary:
            return ["00FF\t2000", "FE\t3000", f"FF\t{balance}"]
        if self.key_shape in ("tinyint-boolean", "tinyint-mismatch"):
            return ["0\t2000", f"1\t{balance}", "2\t3000"]
        if self.key_shape == "tinyint-numeric":
            return ["0\t2000", "1\t3000", f"2\t{balance}"]
        return [f"1\t{balance}"]

    def lock_row(self, phase):
        resource = phase["resource"]
        return "\t".join([phase["xid"], str(phase["branch_id"]), resource, "accounts", self.pk,
                          f"{resource}^^^accounts^^^{self.pk}", "0"])


def compile_actor(work, out, env):
    modules = ("rm", "rm-datasource", "tm", "sqlparser/seata-sqlparser-core",
               "sqlparser/seata-sqlparser-druid")
    jars = list(sorted((reg.ORACLE / "server/target/lib").glob("*.jar")))
    for name in modules:
        artifact = name.rsplit("/", 1)[-1].removeprefix("seata-")
        jars.append(reg.ORACLE / name / "target" / f"seata-{artifact}-2.8.0-SNAPSHOT.jar")
    repository = Path.home() / ".m2/repository"
    for relative in (
        "org/projectlombok/lombok/1.18.36/lombok-1.18.36.jar",
        "com/github/ben-manes/caffeine/caffeine/2.9.3/caffeine-2.9.3.jar",
        "com/fasterxml/jackson/datatype/jackson-datatype-jsr310/2.21.2/jackson-datatype-jsr310-2.21.2.jar",
    ):
        jar = repository / relative
        target = work / "client-dependencies" / jar.name
        put(target, jar.read_bytes())
        jars.append(target)
    jars.append(DRIVER)
    save(out / "java-client-dependencies.json", [
        dict(path=str(jar), sha256=sha(jar)) for jar in jars
    ])
    classes = work / "java-client-classes"
    classes.mkdir()
    admission_source = ROOT / ("compatibility/seata-java/writer-admission/src/main/java/"
                               "io/seatarust/compat/mysql/JavaAtWriterAdmission.java")
    args = ["-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, jars)),
            "-d", classes, ACTOR / "SharedLockJavaActor.java", admission_source]
    compile_args = work / "java-client.args"
    put(compile_args, "\n".join(f'"{str(arg).replace(chr(92), "/")}"' for arg in args) + "\n")
    support.command([reg.JAVA.with_name("javac.exe"), "@" + str(compile_args)], env,
                    out / "java-client-compile.log")
    put(out / "SharedLockJavaActor.class", (classes / "SharedLockJavaActor.class").read_bytes())
    put(out / "JavaAtWriterAdmission.class",
        (classes / "io/seatarust/compat/mysql/JavaAtWriterAdmission.class").read_bytes())
    return os.pathsep.join(map(str, [classes, *jars]))


def wait(check, process, detail, seconds=35):
    deadline = time.monotonic() + seconds
    while not check():
        need(process.poll() is None and time.monotonic() < deadline, detail)
        time.sleep(.05)


def seed_accounts(sql, env, account, out):
    if account.composite:
        table = ("CREATE TABLE accounts(id BIGINT NOT NULL,segment VARCHAR(16) CHARACTER SET utf8mb4 "
                 "COLLATE utf8mb4_0900_ai_ci NOT NULL,balance INT NOT NULL,PRIMARY KEY(id,segment)) ENGINE=InnoDB;"
                 "INSERT INTO accounts VALUES(9007199254740993,'north',1000),"
                 "(9007199254740993,'south',2000),(9007199254740994,'north',3000);")
    elif account.decimal:
        values = account.decimal
        table = (f"CREATE TABLE accounts(id DECIMAL({values['precision']},{values['scale']}) PRIMARY KEY,"
                 "balance INT NOT NULL) ENGINE=InnoDB;"
                 f"INSERT INTO accounts VALUES({values['lower']},2000),"
                 f"({values['target']},1000),({values['upper']},3000);")
    elif account.binary:
        table = ("CREATE TABLE accounts(id VARBINARY(2) PRIMARY KEY,balance INT NOT NULL) ENGINE=InnoDB;"
                 "INSERT INTO accounts VALUES(X'FF',1000),(X'00FF',2000),(X'FE',3000);")
    elif account.tinyint:
        values = ("(0,2000),(1,1000),(2,3000)" if account.key_shape != "tinyint-numeric"
                  else "(0,2000),(1,3000),(2,1000)")
        table = ("CREATE TABLE accounts(id TINYINT(1) PRIMARY KEY,balance INT NOT NULL) ENGINE=InnoDB;"
                 f"INSERT INTO accounts VALUES{values};")
    else:
        table = ("CREATE TABLE accounts(id INT PRIMARY KEY,balance INT NOT NULL) ENGINE=InnoDB;"
                 "INSERT INTO accounts VALUES(1,1000);")
    reg.command(sql + [table + "CREATE TABLE undo_work(id INT PRIMARY KEY,v VARCHAR(1000)) ENGINE=InnoDB"], env)
    if account.composite:
        shape = reg.command(sql + [
            "SELECT COLUMN_NAME,SEQ_IN_INDEX FROM information_schema.STATISTICS "
            "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='accounts' AND INDEX_NAME='PRIMARY' "
            "ORDER BY SEQ_IN_INDEX;"
            "SELECT COLLATION_NAME FROM information_schema.COLUMNS "
            "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='accounts' AND COLUMN_NAME='segment';"
            "SELECT id,segment FROM accounts WHERE id=9007199254740993 AND segment='NORTH'"
        ], env)
        put(out / "account-key-profile.tsv", shape)
        need(shape.decode().splitlines() == ["id\t1", "segment\t2", "utf8mb4_0900_ai_ci",
                                             "9007199254740993\tnorth"],
             "Physical composite key order, collation or alias differs")
    elif account.decimal:
        values = account.decimal
        alias = values["target"].rstrip("0")
        shape = reg.command(sql + [
            "SELECT NUMERIC_PRECISION,NUMERIC_SCALE,COLUMN_KEY FROM information_schema.COLUMNS "
            "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='accounts' AND COLUMN_NAME='id';"
            f"SELECT id FROM accounts WHERE id={alias}"
        ], env)
        put(out / "account-key-profile.tsv", shape)
        need(shape.decode().splitlines() == [f"{values['precision']}\t{values['scale']}\tPRI", values["target"]],
             "Physical decimal precision, scale or equality differs")
    elif account.binary:
        shape = reg.command(sql + [
            "SELECT COLUMN_TYPE,COLUMN_KEY FROM information_schema.COLUMNS "
            "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='accounts' AND COLUMN_NAME='id';"
            "SELECT HEX(id),OCTET_LENGTH(id) FROM accounts ORDER BY id"
        ], env)
        put(out / "account-key-profile.tsv", shape)
        need(shape.decode().splitlines() == ["varbinary(2)\tPRI", "00FF\t2", "FE\t1", "FF\t1"],
             "Physical binary key layout differs")
    elif account.tinyint:
        shape = reg.command(sql + [
            "SELECT COLUMN_TYPE,COLUMN_KEY FROM information_schema.COLUMNS "
            "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='accounts' AND COLUMN_NAME='id';"
            "SELECT id FROM accounts ORDER BY id"
        ], env)
        put(out / "account-key-profile.tsv", shape)
        need(shape.decode().splitlines() == ["tinyint(1)\tPRI", "0", "1", "2"],
             "Physical signed TINYINT(1) key layout differs")


def start_actor(work, out, env, classpath, listener, tc, resource, password, *,
                label="java-owner", expect_lock_conflict=False, account_key="integer", admission=None, user="root"):
    work = work / label
    work.mkdir()
    out = out / label
    out.mkdir()
    jdbc_url = resource + "?useSSL=false&allowPublicKeyRetrieval=true"
    if account_key.startswith("tinyint-"):
        jdbc_url += "&tinyInt1isBit=" + str(account_key != "tinyint-numeric").lower()
    save(work / "database.json", dict(jdbc_url=jdbc_url,
                                      user=user, password=password, expect_lock_conflict=expect_lock_conflict,
                                      account_key=account_key, writer_admission=admission))
    relay = module("real_java_lock_passthrough", ACTOR / "proxy.py").WireProxy(out, listener, tc)
    relay.start()
    try:
        actor = support.spawn([reg.JAVA, "-Xlog:class+load=info:file=java-owner-classes.log",
            "-XX:ActiveProcessorCount=2", "-Xmx512m", "-Djava.io.tmpdir=D:/Temp",
            "-Duser.home=" + str(work), f"-Dfixture.tc.endpoint=127.0.0.1:{listener}",
            "-cp", classpath, "SharedLockJavaActor", work], work, env, out, "client")
    except Exception:
        relay.close()
        raise
    return work, out, relay, actor


def held(owner, out):
    work, _, _, actor = owner
    phase = work / "phase-one.json"
    wait(phase.exists, actor, "Original Java client did not reach business commit")
    value = json.loads(phase.read_bytes())
    need(value["process_id"] == actor.pid and value["branch_id"] > 0, "Java phase-one binding differs")
    save(out / "java-phase-one.json", value)
    return value


def observe(owner, phase, out, stage, expected, sql, env, account, bound_tinyint=False):
    work, _, _, actor = owner
    if stage == BOUND_STAGES[1]:
        state = reg.command(sql + [account.select_state], env)
        put(out / "tinyint-binding-unset-database.tsv", state)
        need(state.decode().splitlines() == account.business_rows(900), "Unset profile changed business or lock state")
        profile = reg.command(sql + [
            "UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=0 WHERE slot=1;"
            "SELECT resource_id,tinyint1_is_bit FROM seata_rs_at_resource_binding"
        ], env)
        put(out / "tinyint-binding-after-drain.tsv", profile)
        need(profile.decode().strip() == f"{phase['resource']}\t0", "Numeric profile was not provisioned")
        return
    raw = reg.command(sql + [account.select_state], env)
    put(out / f"{stage}-database.tsv", raw)
    rows = raw.decode().splitlines()
    source = phase if stage == STAGES[0] else expected
    owner_account = AccountCase("tinyint-boolean") if bound_tinyint and stage == STAGES[0] else account
    expected_rows = [owner_account.lock_row(source), *account.business_rows(900)]
    if stage == STAGES[0]:
        undo = [phase["xid"], str(phase["branch_id"]), "0", phase["undo_hex"]]
        need(rows == [*expected_rows, "\t".join(undo)], "Java lock, business row or Normal Undo differs")
        if bound_tinyint:
            raw_profile = reg.command(sql + ["SELECT resource_id,tinyint1_is_bit FROM seata_rs_at_resource_binding"], env)
            put(out / "tinyint-binding-before-release.tsv", raw_profile)
            need(raw_profile.decode().strip() == f"{phase['resource']}\t1" and expected["status"] is None,
                 "Bound Boolean profile or local rejection differs")
        put(work / "release.ready", "Rust conflict observed\n")
        wait((work / "finished.ready").exists, actor, "Original Java global commit did not finish")
        need((work / "finished.ready").read_text() == phase["xid"], "Java owner changed XID")
        counts = ("SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
                  "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)")
        wait(lambda: reg.command(sql + [counts], env).strip() == b"0\t0\t0\t0",
             actor, "Original Java commit did not release lock and Undo")
        put(out / "java-released.tsv", reg.command(sql + [counts], env))
        if bound_tinyint:
            profile = reg.command(sql + [
                "UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=NULL WHERE slot=1;"
                "SELECT resource_id,tinyint1_is_bit FROM seata_rs_at_resource_binding"
            ], env)
            put(out / "tinyint-binding-cleared.tsv", profile)
            need(profile.decode().strip() == f"{phase['resource']}\tNULL", "Profile was not cleared")
    else:
        need(rows == expected_rows, "Rust lock, business row or Undo visibility differs")
        if bound_tinyint:
            concurrent = subprocess.run(
                sql + ["SET innodb_lock_wait_timeout=1;"
                       "UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=1 WHERE slot=1"],
                cwd=ROOT, env=env, timeout=7, capture_output=True,
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
            put(out / "tinyint-binding-concurrent-update.log", concurrent.stderr)
            need(concurrent.returncode != 0 and b"ERROR 1205 (HY000)" in concurrent.stderr,
                 "Prepared Rust transaction did not prevent changing its bound profile")


def stop_actor(owner):
    work, java_out, relay, actor = owner
    # 完成采集后先关闭观察器，JVM 的退出连接重置不属于业务观察区间。
    relay.close()
    put(work / "stop.ready", "Observation complete\n")
    need(actor.wait(timeout=10) == 0, "Original Java actor exited unsuccessfully")
    put(java_out / "class-load.log", (work / "java-owner-classes.log").read_bytes())
    text = (java_out / "class-load.log").read_text("utf-8")
    for name, jar in (("org.apache.seata.rm.datasource.DataSourceProxy", "seata-rm-datasource-"),
                      ("org.apache.seata.rm.datasource.ConnectionProxy", "seata-rm-datasource-"),
                      ("org.apache.seata.tm.api.DefaultGlobalTransaction", "seata-tm-")):
        need(any(f" {name} source: " in line and jar in line for line in text.splitlines()),
             f"Original Java client class origin differs: {name}")
    wire = module("real_java_lock_decode", ACTOR / "wire.py")
    result = json.loads((java_out / "proxy-result.json").read_bytes())
    need(not result["errors"] and not result["field_translation"], "Java passthrough incomplete")
    return [(row["direction"], wire.decode(row))
            for row in json.loads((java_out / "proxy-events.json").read_bytes()) if row["event"] == "wire"]


def observe_mismatch(owner, phase, expected, out, sql, env, account):
    work, _, _, actor = owner
    original = AccountCase("tinyint-boolean")
    snapshot = reg.command(sql + [account.select_state], env)
    put(out / "tinyint-two-locks.tsv", snapshot)
    rows = snapshot.decode().splitlines()
    undo = "\t".join([phase["xid"], str(phase["branch_id"]), "0", phase["undo_hex"]])
    need(phase["resource"] == expected["resource"] and phase["xid"] != expected["xid"]
         and sorted(rows[:2]) == sorted([original.lock_row(phase), account.lock_row(expected)])
         and rows[2:] == [*account.business_rows(900), undo], "Original TC mixed-profile locks differ")
    put(work / "release.ready", "Two original lock records observed\n")
    wait((work / "finished.ready").exists, actor, "Original Java global commit did not finish")
    need((work / "finished.ready").read_text() == phase["xid"], "Java owner changed XID")
    counts = ("SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
              "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)")
    wait(lambda: reg.command(sql + [counts], env).strip() == b"1\t1\t1\t0",
         actor, "Original Java branch did not clean up independently")
    put(out / "tinyint-rust-lock-after-java-commit.tsv", reg.command(sql + [account.select_state], env))


def finish_mismatch(owner, out, phase, observed, rust, sql, env, account, verify_java_frames):
    need(rust == observed and rust["lock_key"] == account.lock_key, "Rust mismatch identity differs")
    java_account = AccountCase("tinyint-boolean")
    verify_java_frames(stop_actor(owner), phase, True, java_account)
    frames = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text("utf-8").splitlines()]
    need(frames[-1]["succeeded"] and frames[-1]["faults"] == 0, "Rust RM passthrough incomplete")
    for request_kind, response_kind, tail in (
        (21, 22, b"\x01\x00\x00\x01"), (11, 12, b"\x01\x00" + struct.pack(">q", rust["branch_id"])),
    ):
        request, = [row for row in frames if row.get("kind") == request_kind and row["direction"] == "c2s"]
        reply, = [row for row in frames if row.get("kind") == response_kind and row["direction"] == "s2c"]
        identity = support.branch_identity(request, False)
        need((request["connection"], request["request_id"]) == (reply["connection"], reply["request_id"])
             and (identity["xid"], identity["resource"], identity["lock"])
             == (rust["xid"], rust["resource"], account.lock_key)
             and support.payload(reply)[2:] == tail, "Original TC numeric-profile reply differs")
    final = reg.command(sql + [account.select_business + ";"
        "SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log"], env)
    put(out / "mixed-final.tsv", final)
    need(final.decode().splitlines() == [*account.business_rows(900), f"{rust['xid']}\t{rust['branch_id']}\t1\t7B7D"],
         "Final mixed-profile state differs")
    return dict(java_lock_key=java_account.lock_key, rust_lock_key=account.lock_key,
                same_physical_row_registered_twice=True, rust_query_lockable=True,
                interoperability_gate_passed=False)


def finish(owner, out, phase, stages, observation, sql, env, account, bound_tinyint=False):
    java = stop_actor(owner)
    java_key = "accounts:true" if bound_tinyint else account.lock_key
    register, = [row for direction, row in java if direction == "sdk-to-tc" and row["code"] == 11]
    reply, = [row for direction, row in java if direction == "tc-to-sdk" and row["code"] == 12]
    end, = [row for direction, row in java if direction == "sdk-to-tc" and row["code"] == 7]
    ended, = [row for direction, row in java if direction == "tc-to-sdk" and row["code"] == 8]
    need(register["connection"] == reply["connection"] and register["id"] == reply["id"]
         and (register["xid"], register["resource"], register["lock_key"], register["branch_type"],
              reply["branch_id"], reply["result"], end["xid"])
         == (phase["xid"], phase["resource"], java_key, 0, phase["branch_id"], 1, phase["xid"]),
         "Java BranchRegister and commit do not bind the original lock")
    need((end["connection"], end["id"]) == (ended["connection"], ended["id"])
         and (ended["result"], ended["error"], ended["status"]) == (1, 0, 9),
         "Java commit response differs")
    rm = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text("utf-8").splitlines()]
    need(rm[-1]["succeeded"] and rm[-1]["faults"] == 0, "Rust RM passthrough incomplete")
    rm = [row for row in rm if row["event"] == "frame"]
    requests = [row for row in rm if row["kind"] == 11 and row["direction"] == "c2s"]
    expected_stages = stages[-1:] if bound_tinyint else stages
    expected_results = (True,) if bound_tinyint else (False, True)
    need(len(requests) == len(expected_stages), "Rust sent an unexpected registration")
    for request, expected, accepted in zip(requests, expected_stages, expected_results, strict=True):
        identity = support.branch_identity(request, False)
        need((identity["xid"], identity["resource"], identity["lock"], identity["branch_type"])
             == (expected["xid"], phase["resource"], account.lock_key, 0), "Rust wire lock differs from Java")
        response, = [row for row in rm if row["kind"] == 12 and row["direction"] == "s2c"
                     and (row["connection"], row["request_id"]) == (request["connection"], request["request_id"])]
        raw = support.payload(response)
        wanted = ((1, 0, expected["branch_id"]) if accepted else (0, expected["status"], 0))
        need((raw[2], raw[-9], struct.unpack(">q", raw[-8:])[0]) == wanted, "Rust branch result differs")
    query, = [row for row in rm if row["kind"] == 21 and row["direction"] == "c2s"]
    queried, = [row for row in rm if row["kind"] == 22 and row["direction"] == "s2c"]
    identity = support.branch_identity(query, False)
    need((query["connection"], query["request_id"]) == (queried["connection"], queried["request_id"])
         and (identity["xid"], identity["resource"], identity["lock"])
         == (stages[0]["xid"], phase["resource"], account.lock_key)
         and support.payload(queried)[2:] == bytes((1, 0, 0, int(bound_tinyint))),
         "Rust global lock query differs")
    callbacks = [row for row in rm if row["kind"] == 5 and row["direction"] == "s2c"]
    replies = [row for row in rm if row["kind"] == 6 and row["direction"] == "c2s"]
    need(callbacks and len(callbacks) == len(replies), "Rust rollback callback/reply count differs")
    ids = {(row["connection"], row["request_id"]) for row in callbacks}
    need(len(ids) == len(callbacks), "Rust rollback callback wire ID repeated")
    body = support.payload(callbacks[0])
    for callback in callbacks:
        returned, = [row for row in replies if (row["connection"], row["request_id"])
                     == (callback["connection"], callback["request_id"])]
        target = support.branch_identity(callback, True)
        raw = support.payload(returned)
        xid, at = support.string(raw, 4)
        need(support.payload(callback) == body and callback["index"] < returned["index"]
             and (target["xid"], target["branch_id"], target["resource"])
             == (stages[-1]["xid"], stages[-1]["branch_id"], phase["resource"])
             and raw[2:4] == b"\x01\x00" and xid == target["xid"]
             and raw[at:] == struct.pack(">qB", target["branch_id"], 8), "Rust rollback callback outcome differs")
    journal = module("java_rust_lock_journal", ROOT / "tools/formal-rm-java/validate.py")
    records = journal.journal_records((out / "guard-branches.log").read_bytes(), body)
    need(observation["recovery_state"] == [2, 1, 0], "Rust durable rollback terminal differs")
    final = reg.command(sql + [account.select_business + ";"
        "SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log"], env)
    put(out / "mixed-final.tsv", final)
    need(final.decode().splitlines() == [*account.business_rows(900), f"{stages[-1]['xid']}\t{stages[-1]['branch_id']}\t1\t7B7D"],
         "Original Java effect or Rust rollback fence differs")
    return dict(java_registrations=1, rust_registrations=len(requests), rust_lock_conflicts=int(not bound_tinyint),
                rust_profile_rejections=2 if bound_tinyint else 0,
                rust_admitted_after_java_commit=1, rust_rollback_callbacks=len(callbacks),
                rust_journal_states=[record["state"] for record in records],
                lock_key=account.lock_key, end_balance=900)
