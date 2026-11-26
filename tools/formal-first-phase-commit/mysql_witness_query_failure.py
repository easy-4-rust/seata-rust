"""核对原 004 查询的服务端锁等待超时与解除故障后的同一分支恢复。"""
import json
import time

import live_guard as guard
from armed_child_business_commit import AT_HOLD, GUARD
from mysql_phase_two_fault import TOOLS, load


def allow_witness_reobservation(parent, observations, mysql, db, env, out):
    deadline = time.monotonic() + 12
    while True:
        try:
            unknown = json.loads((observations / "java-witness-query-unknown.json").read_bytes())
            break
        except (FileNotFoundError, ValueError):
            assert parent.poll() is None and time.monotonic() < deadline, "first witness outcome missing"
            time.sleep(.02)
    thread = unknown["waiting_thread"]
    blocker = unknown["blocking_connection"]
    errno = unknown["mysql_errno"]
    assert errno in (1205, 3024) and unknown["sql_state"] == "HY000"
    assert type(thread) is int and thread > 0 and type(blocker) is int and blocker > 0
    sql = mysql + [db, "-e"]
    history = guard.reg.command(sql + [
        "SELECT THREAD_ID,EVENT_NAME,MYSQL_ERRNO,RETURNED_SQLSTATE "
        "FROM performance_schema.events_statements_history_long "
        f"WHERE THREAD_ID={thread} AND CURRENT_SCHEMA=DATABASE() AND MYSQL_ERRNO<>0"
    ], env)
    guard.put(out / "mysql-witness-query-error.tsv", history)
    assert history.strip().split(b"\t") == [str(thread).encode(), b"statement/com/Execute", str(errno).encode(), b"HY000"]
    lock_count = guard.reg.command(sql + [
        "SELECT COUNT(*) FROM performance_schema.data_locks l "
        "JOIN performance_schema.threads t ON t.THREAD_ID=l.THREAD_ID "
        f"WHERE t.PROCESSLIST_ID={blocker} AND l.OBJECT_SCHEMA=DATABASE() "
        "AND l.OBJECT_NAME='seata_rs_at_branch_v1' AND l.INDEX_NAME='PRIMARY' "
        "AND l.LOCK_TYPE='RECORD' AND l.LOCK_STATUS='GRANTED'"
    ], env)
    guard.put(out / "mysql-witness-lock-still-held.tsv", lock_count)
    assert lock_count.strip() == b"1"
    no_guard = "(SELECT COUNT(*) FROM undo_log WHERE BINARY xid=b.xid AND branch_id=b.branch_id AND log_status=1)"
    state = guard.reg.command(sql + [AT_HOLD.replace("SELECT ", f"SELECT {no_guard},", 1)], env)
    guard.put(out / "mysql-witness-query-unknown.tsv", state)
    assert state == b"0\t" + (out / "mysql-business-commit-held.tsv").read_bytes(), state
    assert parent.poll() is None
    guard.save(out / "witness-query-unknown.json", dict(unknown, reobserve_permitted_monotonic_ns=time.perf_counter_ns()))
    guard.put(observations / "java-witness-query-reobserve", "Independent MySQL snapshot recorded\n")


def verify_witness_query_failure(out, wire, result):
    raw = load("witness_query_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    unknown = json.loads((out / "witness-query-unknown.json").read_bytes())
    errno = unknown["mysql_errno"]
    assert errno in (1205, 3024) and unknown["sql_state"] == "HY000"
    assert (unknown["xid"], unknown["branch_id"], unknown["first_recovery"]) == (
        result["xid"], result["branch_id"], "ObservationUnknown"
    )
    assert (out / "mysql-witness-query-unknown.tsv").read_bytes() == b"0\t" + (
        out / "mysql-business-commit-held.tsv").read_bytes()
    assert (out / "mysql-witness-lock-still-held.tsv").read_bytes().strip() == b"1"
    assert (out / "mysql-witness-query-error.tsv").read_bytes().strip().split(b"\t") == [
        str(unknown["waiting_thread"]).encode(), b"statement/com/Execute", str(errno).encode(), b"HY000"
    ]
    owner = json.loads((out / "armed-child-ownership.json").read_bytes())
    assert len({owner["mysql_connection_id"], unknown["waiting_connection"], unknown["blocking_connection"]}) == 3
    events = wire["events"]
    greeting, = [row for row in events if row["kind"] == "greeting"
                 and row["mysql_connection_id"] == unknown["waiting_connection"]]
    packet = raw.frame(greeting, 0)
    server_id = packet.index(0, 1) + 1
    assert packet[0] == 10 and int.from_bytes(packet[server_id:server_id + 4], "little") == unknown["waiting_connection"]

    closed = owner["mysql_closed_monotonic_ns"]
    permitted = unknown["reobserve_permitted_monotonic_ns"]
    inserted, = [row for row in events if row["kind"] == "execute" and row.get("sql") == GUARD]
    assert closed < permitted < inserted["qpc_ns"]
    reads = [row for row in events if row["kind"] == "execute" and row.get("sql")
             and row["sql"].startswith("SELECT schema_version,xid,branch_id,resource_id,jdbc_profile,")
             and row["sql"].endswith("FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=? FOR UPDATE")
             and closed < row["qpc_ns"] < inserted["qpc_ns"]]
    initial, reobserved = reads
    assert initial["connection"] == greeting["connection"] != reobserved["connection"]
    assert initial["qpc_ns"] < permitted < reobserved["qpc_ns"]
    assert initial["sql"] == reobserved["sql"] and "first_phase_committed" in initial["sql"]
    key = [result["xid"].encode(), result["branch_id"]]
    for read in reads:
        assert raw.parameters(read, read["statement"], 2) == key
    assert not any(row["kind"] == "commit_received" and closed < row["qpc_ns"] < permitted for row in events)
    return dict(first_recovery="ObservationUnknown", original_witness_mysql_errno=errno,
                original_witness_sql_state="HY000", original_004_lock_wait=True, guard_before_reobservation=0)
