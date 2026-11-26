"""原 Java 写入器按真实物理库、原 JDBC ID 与 TINYINT(1) 映射准入。"""
import json
import subprocess

import mixed_java_lock as actor
from live import PASSWORD, ROOT, need, put, reg, save, support

MIGRATIONS = ROOT / "crates/seata_db_mysql/migrations"
BINDING = "seata_rs_at_resource_binding"
CONNECTION_FACTS = "SELECT @@server_uuid,DATABASE()"


def install_binding(sql, env, resource):
    migrations = [MIGRATIONS / name for name in (
        "005_seata_rs_at_resource_binding.sql", "011_seata_rs_at_tinyint_profile_binding.sql"
    )]
    ddl = "\n".join(path.read_text(encoding="utf-8") for path in migrations)
    statement = f"INSERT INTO {BINDING}(slot,resource_id,tinyint1_is_bit) VALUES(1,X'{resource.encode().hex()}',1)"
    reg.command(sql + [ddl + "\n" + statement], env)


def verify_rejected(work, out, env, sql, classpath, listener, tc, resource, account, *,
                    user="root", password=PASSWORD):
    facts = reg.command(sql + [CONNECTION_FACTS], env)
    put(out / "physical-binding.tsv", facts)
    uuid, schema = facts.decode().strip().split("\t")
    good = dict(server_uuid=uuid, schema=schema)
    for label, endpoint, shape, settings, reason in (
        ("wrong-mapping", resource, "tinyint-numeric", good, "tinyint-profile"),
        ("wrong-resource", resource.replace("127.0.0.1", "localhost"), "tinyint-boolean", good, "jdbc-resource"),
        ("wrong-server", resource, "tinyint-boolean", dict(good, server_uuid="00000000-0000-0000-0000-000000000000"),
         "physical-schema"),
    ):
        run = actor.start_actor(work, out, env, classpath, listener, tc, endpoint, password,
                                label=label, account_key=shape, admission=settings, user=user)
        run_work, run_out, relay, process = run
        try:
            rejected = run_work / "admission-rejected.json"
            held = run_work / "phase-one.json"
            actor.wait(lambda: rejected.exists() or held.exists(), process, "Java admission did not return")
            raw = reg.command(sql + [account.select_state], env)
            put(run_out / "database.tsv", raw)
            if held.exists():
                put(run_out / "unguarded-phase-one.json", held.read_bytes())
                raise AssertionError(f"{label} bypassed admission and reached the original Java business commit")
            observation = json.loads(rejected.read_bytes())
            save(run_out / "admission-rejected.json", observation)
            need(observation == {"reason": reason, "process_id": process.pid}, "Original Java rejection reason differs")
            need(raw.decode().splitlines() == account.business_rows(1000), "Rejected Java writer modified SQL or Undo")
            relay.close()
            put(run_work / "stop.ready", "Observation complete\n")
            need(process.wait(timeout=8) == 0, "Rejected Java writer failed to shut down")
            decoder = actor.module("java_admission_decode", actor.ACTOR / "wire.py")
            frames = json.loads((run_out / "proxy-events.json").read_bytes())
            need(not any(decoder.decode(row)["code"] in (1, 7, 9, 11, 13)
                         for row in frames if row["event"] == "wire" and row["direction"] == "sdk-to-tc"),
                 "Rejected Java writer contacted the TC for a transaction or branch")
        finally:
            relay.close()
            support.stop(process)
    return good


def check_binding_lock(owner, out, sql, env):
    work, _, _, process = owner
    actor.wait((work / "admission-confirmed").exists, process, "Matching Java writer was not admitted")
    blocked = subprocess.run(sql + ["SET SESSION innodb_lock_wait_timeout=1;"
                                     f"UPDATE {BINDING} SET tinyint1_is_bit=0 WHERE slot=1"],
                             env=env, capture_output=True, timeout=5)
    put(out / "binding-concurrent-update.log", blocked.stderr)
    need(blocked.returncode != 0 and b"1205" in blocked.stderr,
         "Provisioner unexpectedly changed mapping inside the admitted Java transaction")
    put(work / "admission-release", "Original binding lock verified\n")
