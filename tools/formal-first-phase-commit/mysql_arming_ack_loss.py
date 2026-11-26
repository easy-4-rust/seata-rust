"""核对原 006 控制 COMMIT 请求或成功确认丢失后的业务回滚。"""
import json
import time

import live_guard as guard
from armed_child_business_commit import (
    AT_HOLD, ATTEMPT, NORMAL, attempt_identity, spawn_armed_business_child, verify_fenced_recovery,
)
from mysql_missing_undo import verify_fence_with_rollback_terminal
from mysql_phase_two_fault import TOOLS, load

AT_ARMING_ACK_LOST = AT_HOLD.replace(
    "SELECT ", "SELECT (SELECT COUNT(*) FROM undo_log WHERE BINARY xid=b.xid AND branch_id=b.branch_id AND log_status=1),", 1
)


AT_ARMING_COMMIT_BLOCKED = (
    "SELECT (SELECT COUNT(*) FROM seata_rs_at_commit_attempt),"
    "(SELECT COUNT(*) FROM undo_log WHERE BINARY xid=b.xid AND branch_id=b.branch_id),"
    "HEX(w.xid),w.branch_id,HEX(w.resource_id),HEX(w.lock_key),g.status,b.status,w.first_phase_committed,"
    "(SELECT COUNT(*) FROM undo_work),"
    "(SELECT COUNT(*) FROM lock_table WHERE xid=b.xid AND branch_id=b.branch_id AND pk='1') "
    "FROM seata_rs_at_branch_v1 w JOIN branch_table b ON BINARY b.xid=w.xid AND b.branch_id=w.branch_id "
    "JOIN global_table g ON g.xid=b.xid"
)


def expected_snapshot(result, request_blocked):
    if request_blocked:
        return ["0", "0", *attempt_identity(result)[:4], "1", "1", "0", "0", "1"]
    return ["0", *attempt_identity(result), "1", "1", "0", "0", "0", "1"]


def start_arming_observer(upstream, mysql, db, env, out, *, request_blocked=False):
    proxy = load("arming_ack_proxy", TOOLS / "formal-commit-direction/proxy.py")

    def snapshot_before_disconnect():
        # 拦截原请求或原 OK 后，先从独立连接取快照，再关闭控制连接。
        query = AT_ARMING_COMMIT_BLOCKED if request_blocked else AT_ARMING_ACK_LOST
        state = guard.reg.command(mysql + [db, "-e", query], env, timeout=4)
        guard.put(out / "mysql-arming-before-disconnect.tsv", state)
        guard.save(out / "arming-snapshot-timing.json", {"monotonic_ns": time.perf_counter_ns()})

    mode = "block_commit" if request_blocked else "drop_ok"
    observer = proxy.UndoRollbackCommitObserver(upstream, mode, ATTEMPT, on_fault=snapshot_before_disconnect)
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               mode=observer.mode, terminal_sql=ATTEMPT))
    return observer


def original_sessions(events):
    undo_ok, = [row for row in events if row["kind"] == "dml_reply_forwarded" and row["sql"] == NORMAL]
    arm, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(ATTEMPT)]
    assert undo_ok["connection"] != arm["connection"]
    return undo_ok, arm


def observe_arming_child(artifact, parent, observations, mysql, db, env, out, observer, *, request_blocked=False):
    deadline = time.monotonic() + 15
    handoff, child = spawn_armed_business_child(
        artifact, parent, observations, env | {"SEATA_JAVA_TC_ARM_ACK_LOSS": "1"}, out, deadline=deadline,
    )
    try:
        assert child.wait(timeout=15) == 0, (out / "armed-child-stdout.log").read_text(errors="replace")
        reaped = time.perf_counter_ns()
        result = json.loads((observations / "java-arming-ack-loss-child.json").read_bytes())
        assert (result["xid"], result["child_pid"]) == (handoff["xid"], child.pid)
        with observer.lock:
            undo, arm = original_sessions(observer.events)
            physical = [row["mysql_connection_id"] for row in observer.events if row["kind"] == "greeting"
                        and row["connection"] in (undo["connection"], arm["connection"])]
        assert len(set(physical)) == len(physical) == 2
        assert all(type(value) is int and value > 0 for value in physical)
        sql = mysql + [db, "-e"]
        existence = ("SELECT COUNT(*) FROM performance_schema.threads "
                     f"WHERE PROCESSLIST_ID IN ({physical[0]},{physical[1]})")
        deadline = time.monotonic() + 5
        while True:
            sessions = guard.reg.command(sql + [existence], env, timeout=4)
            if sessions.strip() == b"0":
                break
            assert time.monotonic() < deadline, "Original MySQL sessions still exist"
            time.sleep(.03)
        closed = time.perf_counter_ns()
        guard.put(out / "mysql-arming-sessions-after-exit.tsv", sessions)
        query = AT_ARMING_COMMIT_BLOCKED if request_blocked else AT_ARMING_ACK_LOST
        state = guard.reg.command(sql + [query], env, timeout=4)
        guard.put(out / "mysql-arming-after-rollback.tsv", state)
        assert state == (out / "mysql-arming-before-disconnect.tsv").read_bytes()
        assert state.decode().strip().split("\t") == expected_snapshot(result, request_blocked)
        assert parent.poll() is None
        guard.save(out / "arming-child-outcome.json", dict(
            result, parent_pid=parent.pid, exit_code=child.returncode, mysql_connections=physical,
            reaped_monotonic_ns=reaped, mysql_closed_monotonic_ns=closed,
            recovery_permitted_monotonic_ns=time.perf_counter_ns(),
        ))
        guard.put(observations / "java-armed-child-exited", "Original business rollback confirmed\n")
    finally:
        guard.support.stop(child)


def verify_arming_fault(mysql, db, env, out, wire, result, *, request_blocked=False):
    raw = load("arming_ack_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    events = wire["events"]
    assert wire["mode"] == ("block_commit" if request_blocked else "drop_ok") and wire["claimed"]
    assert not any(row["kind"] == "observer_error" for row in events)
    child = json.loads((out / "arming-child-outcome.json").read_bytes())
    assert all(child[field] == result[field] for field in ("xid", "branch_id", "resource_id"))
    assert child["arming"] == "CommitOutcomeUnknown" and child["business_rollback"] == "Confirmed"
    assert child["exit_code"] == 0 and child["child_pid"] != child["parent_pid"]
    undo_ok, arm = original_sessions(events)
    assert raw.frame(undo_ok, 1)[:3] == b"\x00\x01\x00"
    business = [row for row in events if row["connection"] == undo_ok["connection"]]
    control = [row for row in events if row["connection"] == arm["connection"]]
    physical = []
    for session in (business, control):
        greeting, = [row for row in session if row["kind"] == "greeting"]
        packet = raw.frame(greeting, 0)
        start = packet.index(0, 1) + 1
        connection = int.from_bytes(packet[start:start + 4], "little")
        assert packet[0] == 10 and greeting["mysql_connection_id"] == connection > 0
        physical.append(connection)
    assert sorted(physical) == sorted(child["mysql_connections"]) and physical[0] != physical[1]

    received, = [row for row in control if row["kind"] == "commit_received"]
    assert received["fault"] and raw.frame(received, 0) == b"\x03COMMIT"
    if request_blocked:
        interrupted, = [row for row in events if row["kind"] == "commit_blocked"]
        assert not any(row["kind"] in ("commit_forwarded", "commit_reply") for row in control)
        assert received["qpc_ns"] < interrupted["qpc_ns"]
    else:
        forwarded, = [row for row in control if row["kind"] == "commit_forwarded"]
        ok, = [row for row in control if row["kind"] == "commit_reply"]
        interrupted, = [row for row in events if row["kind"] == "commit_reply_dropped"]
        assert raw.frame(forwarded, 0) == b"\x03COMMIT"
        assert ok["ok"] and raw.frame(ok, 1)[:3] == b"\x00\x00\x00"
        assert received["qpc_ns"] <= forwarded["qpc_ns"] < ok["qpc_ns"] < interrupted["qpc_ns"]
    assert interrupted["connection"] == arm["connection"]
    assert not any(row["kind"] == "commit_reply_forwarded" for row in control)
    assert not any(row["kind"] in ("commit_received", "commit_forwarded", "commit_reply") for row in business)
    undo, = [row for row in business if row["kind"] == "execute" and row.get("sql") == NORMAL]
    arm_identity, = [row for row in control if row["kind"] == "execute" and (row.get("sql") or "").endswith(
        "FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=? FOR UPDATE"
    )]
    assert raw.parameters(arm_identity, arm_identity["statement"], 2) == [result["xid"].encode(), result["branch_id"]]
    assert arm["qpc_ns"] < arm_identity["qpc_ns"] < received["qpc_ns"]
    sent, = [row for row in business if row["kind"] == "rollback_received"]
    rolled_back, = [row for row in business if row["kind"] == "rollback_reply_forwarded"]
    assert raw.frame(sent, 0) == b"\x03ROLLBACK" and raw.frame(rolled_back, 1)[:3] == b"\x00\x00\x00"
    snap = json.loads((out / "arming-snapshot-timing.json").read_bytes())["monotonic_ns"]
    assert undo["qpc_ns"] < undo_ok["qpc_ns"] < arm["qpc_ns"] < received["qpc_ns"]
    assert interrupted["qpc_ns"] < snap < sent["qpc_ns"]
    assert sent["qpc_ns"] < rolled_back["qpc_ns"] < child["reaped_monotonic_ns"] <= child["mysql_closed_monotonic_ns"]
    assert child["mysql_closed_monotonic_ns"] < child["recovery_permitted_monotonic_ns"]
    assert (out / "mysql-arming-sessions-after-exit.tsv").read_bytes().strip() == b"0"
    before = (out / "mysql-arming-before-disconnect.tsv").read_bytes()
    after = (out / "mysql-arming-after-rollback.tsv").read_bytes()
    assert before == after
    assert after.decode().strip().split("\t") == expected_snapshot(result, request_blocked)
    permitted = child["recovery_permitted_monotonic_ns"]
    if request_blocked:
        verified = verify_rollback_without_attempt(mysql, db, env, out, events, result, permitted)
    else:
        verified = verify_fenced_recovery(mysql, db, env, out, events, result, raw, recovery_after_ns=permitted)
    return dict(verified, first_recovery="ArmingCommitOutcomeUnknown", arm_ack_dropped=not request_blocked,
                arm_commit_blocked=request_blocked, durable_arm_commits=int(not request_blocked),
                business_commit_requests=0, business_rollback_confirmed=True)


def verify_rollback_without_attempt(mysql, db, env, out, events, result, permitted):
    assert result["recovery_outcome"] == "MissingAttempt" and result["fenced_before_global_rollback"] == [0, 0, 0, None]
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
    rm = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    rollback, = [row for row in tm if row.get("kind") == 9 and row["direction"] == "c2s"]
    callbacks = [row["monotonic_ns"] for row in rm if row.get("kind") == 5 and row["direction"] == "s2c"]
    replies = [row["monotonic_ns"] for row in rm if row.get("kind") == 6 and row["direction"] == "c2s"]
    fence, completed = verify_fence_with_rollback_terminal(events, result["xid"], result["branch_id"])
    assert permitted < rollback["monotonic_ns"] < min(callbacks) < fence < completed < min(replies)
    final = guard.reg.command(mysql + [db, "-e", "SELECT COUNT(*) FROM seata_rs_at_commit_attempt"], env)
    guard.put(out / "final-armed-attempt-count.tsv", final)
    assert final.strip() == b"0"
    return dict(recovery="MissingAttempt", final_attempt_rows=0, fence_and_terminal_in_one_commit=True,
                fence_commits=1, rollback_terminal_commits=1)
