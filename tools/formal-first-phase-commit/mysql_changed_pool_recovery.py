"""核对有变化 COMMIT 确认滞留后的同池换租、恢复与报告。"""
import json
import time

import live_guard as guard
from armed_child_business_commit import AT_HOLD, ATTEMPT, GUARD, IDENTITY, NORMAL, attempt_identity
from mysql_no_change import read_client_observation
from mysql_phase_two_fault import TOOLS, load
from mysql_pool_recovery import permit_pool_probe, verify_pool_probe

BUSINESS = "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_report')"


def permit_changed_recovery(client, observations, mysql, db, env, out, observer):
    permit_pool_probe(client, observations, mysql, db, env, out, observer,
                      local_observation="java-changed-pool-local.json")
    local = read_client_observation(client, observations / "java-changed-pool-local.json", out)
    sql = mysql + [db, "-e"]
    state = guard.reg.command(sql + [AT_HOLD], env)
    guard.put(out / "mysql-changed-pool-before-recovery.tsv", state)
    assert state.decode().strip().split("\t") == [*attempt_identity(local), "1", "1", "1", "1", "1", "1"]
    rows = guard.reg.command(sql + ["SELECT id,v FROM undo_work"], env)
    guard.put(out / "mysql-changed-pool-before-business.tsv", rows)
    assert rows.strip() == b"1\toriginal_tc_report"
    guard.save(out / "changed-pool-local.json", dict(local, recovery_permitted_monotonic_ns=time.perf_counter_ns()))
    guard.put(observations / "java-changed-pool-recovery-permitted", "Original business and Undo observed\n")


def verify_changed_pool(mysql, db, env, out, wire, result):
    raw = load("changed_pool_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    assert wire["mode"] == "hold_commit_ack" and wire["claimed"]
    assert (result["recovery_outcome"], result["report_response"], result["global_result"], result["before_commit"]) == (
        "Applied", "Success", "Committed", {"business": 1, "normal_undo": 1, "witness": 1, "tc_lock": 1}
    )
    local = json.loads((out / "changed-pool-local.json").read_bytes())
    assert all(local[field] == result[field] for field in ("xid", "branch_id", "resource_id"))
    assert local["local_outcome"] == "Unknown" and local["source"] == "Deadline"
    assert (out / "mysql-changed-pool-before-recovery.tsv").read_text().strip().split("\t") == [
        *attempt_identity(result), "1", "1", "1", "1", "1", "1"
    ]
    assert (out / "mysql-changed-pool-before-business.tsv").read_bytes().strip() == b"1\toriginal_tc_report"

    events = wire["events"]
    assert not any(row["kind"] == "observer_error" or
                   row["kind"] == "execute" and row.get("sql") == GUARD for row in events)
    received, = [row for row in events if row["kind"] == "commit_received" and row["fault"]]
    old = [row for row in events if row["connection"] == received["connection"]]
    assert [row for row in old if row["kind"] == "commit_received"] == [received]
    forwarded, = [row for row in old if row["kind"] == "commit_forwarded"]
    ok, = [row for row in old if row["kind"] == "commit_reply"]
    held, = [row for row in old if row["kind"] == "commit_ack_held"]
    assert not any(row["kind"] == "commit_reply_forwarded" for row in old)
    assert raw.frame(received, 0) == raw.frame(forwarded, 0) == b"\x03COMMIT"
    reply = raw.frame(ok, 1)
    assert ok["ok"] and reply[:3] == b"\x00\x00\x00" and not (reply[3] & 1)
    assert received["qpc_ns"] <= forwarded["qpc_ns"] < ok["qpc_ns"] < held["qpc_ns"]

    business, = [row for row in events if row["kind"] == "execute" and row.get("sql") == BUSINESS]
    undo, = [row for row in events if row["kind"] == "execute" and row.get("sql") == NORMAL]
    undo_ok, = [row for row in events if row["kind"] == "dml_reply_forwarded" and row.get("sql") == NORMAL]
    assert business["connection"] == undo["connection"] == undo_ok["connection"] == received["connection"]
    assert raw.frame(undo_ok, 1)[:3] == b"\x00\x01\x00" and raw.frame(undo_ok, 1)[3] & 1
    arm, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(ATTEMPT)]
    arm_ok, = [row for row in events if row["kind"] == "commit_reply_forwarded"
               and row["connection"] == arm["connection"] and row["qpc_ns"] > arm["qpc_ns"]]
    assert arm["connection"] != received["connection"] and raw.frame(arm_ok, 1)[:3] == b"\x00\x00\x00"
    assert business["qpc_ns"] < undo["qpc_ns"] < undo_ok["qpc_ns"] < arm["qpc_ns"] < arm_ok["qpc_ns"] < received["qpc_ns"]

    permitted = local["recovery_permitted_monotonic_ns"]
    verified = verify_pool_probe(out, wire, result, pool_limit=2, completed_before_ns=permitted)
    rm = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    report, = [row for row in rm if row.get("kind") == 13 and row["direction"] == "c2s"]
    claim, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(
        "UPDATE seata_rs_at_commit_attempt SET report_started=1"
    )]
    claimed, = [row for row in events if row["kind"] == "commit_reply_forwarded"
                and row["connection"] == claim["connection"] and row["qpc_ns"] > claim["qpc_ns"]]
    branch_key = [result["xid"].encode(), result["branch_id"]]
    assert raw.parameters(claim, claim["statement"], 2) == branch_key
    assert raw.frame(claimed, 1)[:3] == b"\x00\x00\x00"
    assert permitted < claim["qpc_ns"] < claimed["qpc_ns"] < report["monotonic_ns"]
    witness_reads = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(
        "SELECT schema_version,xid,branch_id,resource_id,jdbc_profile,"
    ) and row["sql"].endswith("FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=? FOR UPDATE")
                     and permitted < row["qpc_ns"] < claim["qpc_ns"]]
    assert witness_reads
    assert all(raw.parameters(row, row["statement"], 2) == branch_key for row in witness_reads)
    state = guard.reg.command(mysql + [db, "-e", f"SELECT {IDENTITY} FROM seata_rs_at_commit_attempt a"], env)
    guard.put(out / "final-armed-attempt.tsv", state)
    assert state.decode().strip().split("\t") == [*attempt_identity(result)[:-1], "1"]
    rows = guard.reg.command(mysql + [db, "-e", "SELECT id,v FROM undo_work"], env)
    guard.put(out / "final-business-row.tsv", rows)
    assert rows.strip() == b"1\toriginal_tc_report"
    return dict(verified, original_business_commits=1, original_business_ack_held=True, first_recovery="Applied",
                original_receipt_preserved=True)
