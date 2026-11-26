"""在截留业务 COMMIT 请求或成功响应时终止原业务进程。"""
import json
import time

import live_guard as guard
from mysql_phase_two_fault import TOOLS, delay_handler_commit, load

NORMAL = (
    "INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) "
    "VALUES (?,?,?,?,0,now(6),now(6))"
)
ATTEMPT = "INSERT INTO seata_rs_at_commit_attempt "
GUARD = NORMAL.replace("?,?,?,?,0,", "?,?,?,?,1,")
IDENTITY = "HEX(a.xid),a.branch_id,HEX(a.resource_id),HEX(a.lock_key),a.attempt_state,a.report_started"
AT_HOLD = (
    f"SELECT {IDENTITY},g.status,b.status,w.first_phase_committed,"
    "(SELECT COUNT(*) FROM undo_work),"
    "(SELECT COUNT(*) FROM undo_log WHERE log_status=0 AND BINARY xid=b.xid AND branch_id=b.branch_id),"
    "(SELECT COUNT(*) FROM lock_table WHERE xid=b.xid AND branch_id=b.branch_id AND pk='1') "
    "FROM seata_rs_at_commit_attempt AS a "
    "JOIN branch_table AS b ON BINARY b.xid=a.xid AND b.branch_id=a.branch_id "
    "JOIN global_table AS g ON g.xid=b.xid "
    "JOIN seata_rs_at_branch_v1 AS w ON w.xid=a.xid AND w.branch_id=a.branch_id"
)


def start_business_commit_observer(upstream, mysql, env, out, *, hold_ack=False, in_flight=False,
                                   drop_fence_ack=False, before_commit=None):
    mode = "hold_commit_ack" if hold_ack else "hold_commit"
    proxy = load("held_business_commit_proxy", TOOLS / "formal-commit-direction/proxy.py")

    def prepare_handler_commit(physical_id, *, prepare):
        assert prepare and type(physical_id) is int and physical_id > 0
        delay_handler_commit(mysql, env)

    follow_up = GUARD if drop_fence_ack else None
    observer = proxy.UndoRollbackCommitObserver(upstream, mode, NORMAL,
                                                on_fault=prepare_handler_commit if in_flight else before_commit,
                                                follow_up_drop_ok_sql=follow_up)
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               mode=mode, terminal_sql=NORMAL, in_flight=in_flight, follow_up_drop_ok_sql=follow_up))
    return observer


def spawn_armed_business_child(artifact, parent, observations, env, out, *, deadline):
    while True:
        try:
            handoff = json.loads((observations / "java-armed-child-ready.json").read_bytes())
            break
        except (FileNotFoundError, ValueError):
            assert parent.poll() is None and time.monotonic() < deadline, "TM parent did not begin"
            time.sleep(.02)
    assert handoff["parent_pid"] == parent.pid
    child_env = env | dict(SEATA_JAVA_TC_COMMIT_CHILD="1", SEATA_JAVA_TC_COMMIT_XID=handoff["xid"])
    child = guard.support.spawn(
        [artifact, "original_java_tc_cross_process_commit_child", "--ignored", "--exact", "--test-threads=1"],
        guard.ROOT, child_env, out, "armed-child"
    )
    return handoff, child


def terminate_with_original_commit_held(artifact, parent, observations, mysql, db, env, out, observer, *,
                                        in_flight=False, before_finish=None):
    deadline = time.monotonic() + 15
    handoff, child = spawn_armed_business_child(artifact, parent, observations, env, out, deadline=deadline)
    sql = mysql + [db, "-e"]
    event_kind = ("commit_forwarded" if in_flight else
                  "commit_ack_held" if observer.mode == "hold_commit_ack" else "commit_held")
    try:
        if before_finish is not None:
            before_finish(child, parent, handoff, observer)
        held = []
        while not held:
            with observer.lock:
                if in_flight:
                    fault_requests = {event["connection"] for event in observer.events
                                      if event["kind"] == "commit_received" and event["fault"]}
                    held = [event for event in observer.events if event["kind"] == event_kind
                            and event["connection"] in fault_requests]
                else:
                    held = [event for event in observer.events if event["kind"] == event_kind]
            if not held:
                assert child.poll() is None and parent.poll() is None and time.monotonic() < deadline, (
                    "Original business COMMIT did not arrive at the observer"
                )
                time.sleep(.02)
        event, = held
        if in_flight:
            with observer.lock:
                greeting, = [row for row in observer.events if row["kind"] == "greeting"
                             and row["connection"] == event["connection"]]
            physical = greeting["mysql_connection_id"]
        else:
            physical = event["mysql_connection_id"]
        assert type(physical) is int and physical > 0
        existence = f"SELECT COUNT(*) FROM performance_schema.threads WHERE PROCESSLIST_ID={physical}"
        stage_times = {}
        if in_flight:
            stage_query = (AT_HOLD.replace("SELECT ", "SELECT PROCESSLIST_ID,PROCESSLIST_COMMAND,"
                           "PROCESSLIST_INFO,PROCESSLIST_STATE,", 1)
                           + f" JOIN performance_schema.threads WHERE PROCESSLIST_ID={physical}")
            packet = guard.reg.command(sql + [stage_query], env, timeout=4)
            stage_prefix = f"{physical}\tQuery\tCOMMIT\twaiting for handler commit\t".encode()
            guard.put(out / "mysql-business-commit-stage-before-kill.tsv", packet)
            assert packet.startswith(stage_prefix), packet
            state = packet.removeprefix(stage_prefix)
            stage_times["stage_before_kill_monotonic_ns"] = time.perf_counter_ns()
        else:
            assert guard.reg.command(sql + [existence], env, timeout=4).strip() == b"1"
            state = guard.reg.command(sql + [AT_HOLD], env, timeout=4)
        guard.put(out / "mysql-business-commit-held.tsv", state)
        held_fields = state.decode().strip().split("\t")
        expected_local = ["1", "1", "1"] if event_kind == "commit_ack_held" else ["0", "0", "0"]
        assert held_fields[0] == handoff["xid"].encode().hex().upper()
        assert held_fields[4:] == ["1", "0", "1", "1", *expected_local, "1"]
        assert child.poll() is None and parent.poll() is None
        kill_started = time.perf_counter_ns()
        child.kill()
        assert child.wait(timeout=5) != 0
        reaped = time.perf_counter_ns()
        if in_flight:
            stage_times["stage_after_reap_started_monotonic_ns"] = time.perf_counter_ns()
            after_reap = guard.reg.command(sql + [stage_query], env, timeout=4)
            guard.put(out / "mysql-business-commit-stage-after-reap.tsv", after_reap)
            assert after_reap == packet, after_reap
        deadline = time.monotonic() + 5
        while True:
            sessions = guard.reg.command(sql + [existence], env, timeout=4)
            if sessions.strip() == b"0":
                break
            assert time.monotonic() < deadline, "Original MySQL connection still exists"
            time.sleep(.03)
        guard.put(out / "mysql-owner-sessions-after-exit.tsv", sessions)
        closed = time.perf_counter_ns()
        if in_flight:
            committed = guard.reg.command(sql + [AT_HOLD], env, timeout=4)
            guard.put(out / "mysql-business-commit-after-exit.tsv", committed)
            assert committed.decode().strip().split("\t") == [*held_fields[:8], "1", "1", "1", "1"]
            guard.reg.command(mysql + ["-e", "SET GLOBAL binlog_group_commit_sync_delay=0"], env, timeout=5)
            stage_times["committed_after_exit_monotonic_ns"] = time.perf_counter_ns()
        guard.save(out / "armed-child-ownership.json", dict(
            handoff, child_pid=child.pid, exit_code=child.returncode, stopped=True,
            mysql_connection_id=physical, kill_started_monotonic_ns=kill_started,
            reaped_monotonic_ns=reaped, mysql_closed_monotonic_ns=closed, **stage_times,
        ))
        guard.put(observations / "java-armed-child-exited", "Original business session closed\n")
    finally:
        guard.support.stop(child)


def allow_fence_reobservation(parent, observations, mysql, db, env, out, observer):
    deadline = time.monotonic() + 10
    while True:
        try:
            unknown = json.loads((observations / "java-fence-ack-unknown.json").read_bytes())
            break
        except (FileNotFoundError, ValueError):
            assert parent.poll() is None and time.monotonic() < deadline, "first fence outcome missing"
            time.sleep(.02)
    assert unknown["first_recovery"] == "FenceCommitOutcomeUnknown"
    with observer.lock:
        dropped, = [row for row in observer.events if row["kind"] == "commit_reply_dropped"]
    fence_count = ("(SELECT COUNT(*) FROM undo_log WHERE log_status=1 AND BINARY xid=b.xid "
                   "AND branch_id=b.branch_id AND HEX(rollback_info)='7B7D' "
                   "AND BINARY context='serializer=jackson&compressorType=NONE')")
    state = guard.reg.command(mysql + [db, "-e", AT_HOLD.replace("SELECT ", f"SELECT {fence_count},", 1)], env)
    guard.put(out / "mysql-fence-ack-unknown.tsv", state)
    assert state == b"1\t" + (out / "mysql-business-commit-held.tsv").read_bytes(), state
    assert parent.poll() is None
    guard.save(out / "fence-ack-unknown.json", dict(
        unknown, dropped_monotonic_ns=dropped["qpc_ns"], reobserve_permitted_monotonic_ns=time.perf_counter_ns(),
    ))
    guard.put(observations / "java-fence-ack-reobserve", "Independent MySQL snapshot recorded\n")


def attempt_identity(result):
    return [result["xid"].encode().hex().upper(), str(result["branch_id"]),
            result["resource_id"].encode().hex().upper(), b"undo_work:1".hex().upper(), "1", "0"]


def verify_blocked_business_commit(mysql, db, env, out, wire, result, *, fence_ack_lost=False, competing=False):
    raw = load("blocked_business_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    events = wire["events"]
    assert wire["mode"] == "hold_commit" and wire["claimed"]
    assert not any(row["kind"] == "observer_error" for row in events)
    held, = [row for row in events if row["kind"] == "commit_held"]
    original = [row for row in events if row["connection"] == held["connection"]]
    sent, = [row for row in original if row["kind"] == "commit_received"]
    assert sent["fault"] and raw.frame(sent, 0) == b"\x03COMMIT" and sent["qpc_ns"] < held["qpc_ns"]
    undo_ok, = [row for row in original if row["kind"] == "dml_reply_forwarded" and row["sql"] == NORMAL]
    assert raw.frame(undo_ok, 1)[:3] == b"\x00\x01\x00" and undo_ok["qpc_ns"] < sent["qpc_ns"]
    assert not any(row["kind"] in ("commit_forwarded", "commit_reply", "rollback_forwarded") for row in original)
    assert not any(row["kind"] == "client_command" and row["qpc_ns"] > held["qpc_ns"] for row in original)
    arm, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(ATTEMPT)]
    assert arm["connection"] != held["connection"]
    control_ok, = [row for row in events if row["connection"] == arm["connection"]
                   and row["kind"] == "commit_reply_forwarded" and row["qpc_ns"] > arm["qpc_ns"]]
    assert raw.frame(control_ok, 1)[:3] == b"\x00\x00\x00"
    assert control_ok["qpc_ns"] < sent["qpc_ns"]
    ownership = json.loads((out / "armed-child-ownership.json").read_bytes())
    assert ownership["stopped"] and ownership["exit_code"] and ownership["child_pid"] != ownership["parent_pid"]
    assert held["mysql_connection_id"] == ownership["mysql_connection_id"] and ownership["xid"] == result["xid"]
    assert held["qpc_ns"] < ownership["reaped_monotonic_ns"] <= ownership["mysql_closed_monotonic_ns"]
    assert (out / "mysql-business-commit-held.tsv").read_text().strip().split("\t") == [
        *attempt_identity(result), "1", "1", "0", "0", "0", "1"
    ]
    assert (out / "mysql-owner-sessions-after-exit.tsv").read_bytes().strip() == b"0"
    recovery_bound = "kill_started_monotonic_ns" if competing else "mysql_closed_monotonic_ns"
    verified = verify_fenced_recovery(mysql, db, env, out, events, result, raw,
                                      recovery_after_ns=ownership[recovery_bound],
                                      fence_ack_lost=fence_ack_lost)
    return dict(verified, held_business_commit_requests=1, forwarded_business_commits=0,
                durable_arm_commits=1, original_owner_disconnected=True)


def verify_fenced_recovery(mysql, db, env, out, events, result, raw, *, recovery_after_ns, fence_ack_lost=False):
    assert result["recovery_outcome"] == "NotAppliedFenced" and result["fenced_before_global_rollback"] == [0, 1, 0, 0]
    guard_insert, = [row for row in events if row["kind"] == "execute" and row["sql"] == GUARD]
    assert raw.parameters(guard_insert, guard_insert["statement"], 4) == [
        result["branch_id"], result["xid"].encode(), b"serializer=jackson&compressorType=NONE", b"{}"
    ]
    fence_events = [row for row in events if row["connection"] == guard_insert["connection"]
                    and row["qpc_ns"] > guard_insert["qpc_ns"]]
    fence_commit, = [row for row in fence_events if row["kind"] == "commit_reply"]
    assert fence_commit["ok"] and raw.frame(fence_commit, 1)[:3] == b"\x00\x00\x00"
    deliveries = [row for row in fence_events if row["kind"] == "commit_reply_forwarded"]
    drops = [row for row in fence_events if row["kind"] == "commit_reply_dropped"]
    if fence_ack_lost:
        assert not deliveries
        dropped, = drops
        requested, = [row for row in fence_events if row["kind"] == "commit_received"]
        forwarded, = [row for row in fence_events if row["kind"] == "commit_forwarded"]
        assert requested["fault"] and raw.frame(requested, 0) == raw.frame(forwarded, 0) == b"\x03COMMIT"
        assert requested["qpc_ns"] <= forwarded["qpc_ns"] < fence_commit["qpc_ns"] < dropped["qpc_ns"]
    else:
        assert not drops
        delivered, = deliveries
        assert raw.frame(delivered, 1) == raw.frame(fence_commit, 1)
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
    rollback, = [row for row in tm if row.get("kind") == 9 and row["direction"] == "c2s"]
    terminal, = [row for row in events if row["kind"] == "execute" and
                 (row.get("sql") or "").startswith("UPDATE seata_rs_at_branch_v1 SET terminal=1,")]
    assert raw.parameters(terminal, terminal["statement"], 2) == [result["xid"].encode(), result["branch_id"]]
    terminal_ack, = [row for row in events if row["connection"] == terminal["connection"]
                     and row["kind"] == "commit_reply_forwarded" and row["qpc_ns"] > terminal["qpc_ns"]]
    assert raw.frame(terminal_ack, 1)[:3] == b"\x00\x00\x00"
    assert recovery_after_ns < guard_insert["qpc_ns"] < fence_commit["qpc_ns"]
    assert fence_commit["qpc_ns"] < rollback["monotonic_ns"] < terminal["qpc_ns"] < terminal_ack["qpc_ns"]
    if fence_ack_lost:
        unknown = json.loads((out / "fence-ack-unknown.json").read_bytes())
        assert (unknown["xid"], unknown["branch_id"], unknown["first_recovery"]) == (
            result["xid"], result["branch_id"], "FenceCommitOutcomeUnknown"
        )
        permitted = unknown["reobserve_permitted_monotonic_ns"]
        assert unknown["dropped_monotonic_ns"] == dropped["qpc_ns"] < permitted < rollback["monotonic_ns"]
        assert (out / "mysql-fence-ack-unknown.tsv").read_bytes() == b"1\t" + (
            out / "mysql-business-commit-held.tsv").read_bytes()
        # 恢复端在独立快照后重新锁定同一个 Undo 键，不重发围栏 COMMIT。
        recovery_events = [row for row in events if dropped["qpc_ns"] < row["qpc_ns"] < rollback["monotonic_ns"]]
        assert not any(row["kind"] == "commit_received" for row in recovery_events)
        key = [result["xid"].encode(), result["branch_id"]]
        reads = [row for row in recovery_events if row["kind"] == "execute" and row.get("sql")
                 and row["sql"].endswith("FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE")]
        locks = [row for row in reads if row["sql"].startswith("SELECT xid,branch_id,log_status,context,")]
        bodies = [row for row in reads if row["sql"].startswith("SELECT rollback_info ")]
        assert len(locks) == len(bodies) == 2
        for lock, body in zip(locks, bodies, strict=True):
            assert lock["connection"] == body["connection"] != guard_insert["connection"]
            assert permitted < lock["qpc_ns"] < body["qpc_ns"]
            assert raw.parameters(lock, lock["statement"], 2) == raw.parameters(body, body["statement"], 2) == key
            rolled_back, = [row for row in recovery_events if row["connection"] == lock["connection"]
                            and row["kind"] == "rollback_reply_forwarded" and row["qpc_ns"] > body["qpc_ns"]]
            assert raw.frame(rolled_back, 1)[:3] == b"\x00\x00\x00"
    final = guard.reg.command(mysql + [db, "-e", f"SELECT {IDENTITY} FROM seata_rs_at_commit_attempt a"], env)
    guard.put(out / "final-armed-attempt.tsv", final)
    assert final.decode().strip().split("\t") == attempt_identity(result)
    return dict(recovery="NotAppliedFenced", fence_commits=1, rollback_terminal_commits=1,
                fence_ack_dropped=fence_ack_lost,
                first_recovery="FenceCommitOutcomeUnknown" if fence_ack_lost else None)


def verify_applied_business_commit(mysql, db, env, out, wire, result, *, in_flight=False, competing=False):
    raw = load("applied_business_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    events = wire["events"]
    assert wire["mode"] == "hold_commit_ack" and wire["claimed"]
    assert not any(row["kind"] == "observer_error" for row in events)
    assert (result["recovery_outcome"], result["global_result"], result["child_exited_before_recovery"]) == (
        "Applied", "Committed", not competing
    )
    sent, = [row for row in events if row["kind"] == "commit_received" and row["fault"]]
    original = [row for row in events if row["connection"] == sent["connection"]]
    assert [row for row in original if row["kind"] == "commit_received"] == [sent]
    forwarded, = [row for row in original if row["kind"] == "commit_forwarded"]
    replies = [row for row in original if row["kind"] == "commit_reply"]
    undo_ok, = [row for row in original if row["kind"] == "dml_reply_forwarded" and row["sql"] == NORMAL]
    assert sent["fault"] and raw.frame(sent, 0) == raw.frame(forwarded, 0) == b"\x03COMMIT"
    assert raw.frame(undo_ok, 1)[:3] == b"\x00\x01\x00" and undo_ok["qpc_ns"] < sent["qpc_ns"]
    assert sent["qpc_ns"] <= forwarded["qpc_ns"]
    assert all(reply["ok"] and raw.frame(reply, 1)[:3] == b"\x00\x00\x00" for reply in replies)
    assert not any(row["kind"] in ("commit_reply_forwarded", "rollback_forwarded") for row in original)
    assert not any(row["kind"] == "client_command" and row["qpc_ns"] > forwarded["qpc_ns"] for row in original)
    assert not any(row["kind"] == "execute" and row["sql"] == GUARD for row in events)
    arm, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(ATTEMPT)]
    assert arm["connection"] != sent["connection"]
    control_ok, = [row for row in events if row["connection"] == arm["connection"]
                   and row["kind"] == "commit_reply_forwarded" and row["qpc_ns"] > arm["qpc_ns"]]
    assert raw.frame(control_ok, 1)[:3] == b"\x00\x00\x00" and control_ok["qpc_ns"] < sent["qpc_ns"]
    rm = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
    report, = [row for row in rm if row.get("kind") == 13 and row["direction"] == "c2s"]
    commit, = [row for row in tm if row.get("kind") == 7 and row["direction"] == "c2s"]
    ownership = json.loads((out / "armed-child-ownership.json").read_bytes())
    assert ownership["stopped"] and ownership["exit_code"] and ownership["child_pid"] != ownership["parent_pid"]
    greeting, = [row for row in original if row["kind"] == "greeting"]
    physical = ownership["mysql_connection_id"]
    assert greeting["mysql_connection_id"] == physical and ownership["xid"] == result["xid"]
    reaped = ownership["reaped_monotonic_ns"]
    closed = ownership["mysql_closed_monotonic_ns"]
    assert forwarded["qpc_ns"] < reaped <= closed < report["monotonic_ns"] < commit["monotonic_ns"]
    assert (out / "mysql-owner-sessions-after-exit.tsv").read_bytes().strip() == b"0"
    identity = [result["xid"].encode().hex().upper(), str(result["branch_id"]),
                result["resource_id"].encode().hex().upper(), b"undo_work:1".hex().upper(), "1"]
    held_state = (out / "mysql-business-commit-held.tsv").read_bytes()
    committed_state = [*identity, "0", "1", "1", "1", "1", "1", "1"]
    ack_events = [row for row in original if row["kind"] == "commit_ack_held"]
    assert len(replies) == len(ack_events) <= 1
    if replies:
        reply, = replies
        ack_held, = ack_events
        assert ack_held["mysql_connection_id"] == physical
        assert forwarded["qpc_ns"] < reply["qpc_ns"] < ack_held["qpc_ns"]
    if in_flight:
        assert all(reaped < reply["qpc_ns"] for reply in replies)
        stage = f"{physical}\tQuery\tCOMMIT\twaiting for handler commit\t".encode() + held_state
        assert (out / "mysql-business-commit-stage-before-kill.tsv").read_bytes() == stage
        assert (out / "mysql-business-commit-stage-after-reap.tsv").read_bytes() == stage
        assert held_state.decode().strip().split("\t") == [*committed_state[:8], "0", "0", "0", "1"]
        after_exit = (out / "mysql-business-commit-after-exit.tsv").read_text().strip().split("\t")
        assert after_exit == committed_state
        assert forwarded["qpc_ns"] < ownership["stage_before_kill_monotonic_ns"] < reaped
        assert reaped <= ownership["stage_after_reap_started_monotonic_ns"] < closed
        assert closed <= ownership["committed_after_exit_monotonic_ns"] < report["monotonic_ns"]
    else:
        ack_held, = ack_events
        assert ack_held["qpc_ns"] < reaped
        assert held_state.decode().strip().split("\t") == committed_state
    final = guard.reg.command(mysql + [db, "-e", f"SELECT {IDENTITY} FROM seata_rs_at_commit_attempt a"], env)
    guard.put(out / "final-armed-attempt.tsv", final)
    assert final.decode().strip().split("\t") == [*identity, "1"]
    business = guard.reg.command(mysql + [db, "-e", "SELECT id,v FROM undo_work"], env)
    guard.put(out / "final-business-row.tsv", business)
    assert business.strip() == b"1\toriginal_tc_cross_process"
    return dict(recovery="Applied", forwarded_business_commits=1, actual_business_commit_acks_held=len(ack_events),
                durable_arm_commits=1, original_owner_disconnected=True, fence_commits=0,
                original_commit_executing_after_owner_exit=in_flight)
