"""核对恢复事务与原 MySQL 业务会话的真实主键行锁等待。"""
import json
import threading
import time

import live_guard as guard
from armed_child_business_commit import AT_HOLD, GUARD, attempt_identity
from mysql_phase_two_fault import TOOLS, load

LOCK_WAIT = (
    "SELECT r.PROCESSLIST_ID,b.PROCESSLIST_ID,l.OBJECT_NAME,l.INDEX_NAME,l.LOCK_STATUS "
    "FROM performance_schema.data_lock_waits w "
    "JOIN performance_schema.data_locks l ON l.ENGINE_LOCK_ID=w.REQUESTING_ENGINE_LOCK_ID "
    "JOIN performance_schema.threads r ON r.THREAD_ID=w.REQUESTING_THREAD_ID "
    "JOIN performance_schema.threads b ON b.THREAD_ID=w.BLOCKING_THREAD_ID "
    "WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_at_branch_v1' "
    "AND l.INDEX_NAME='PRIMARY' AND l.LOCK_TYPE='RECORD' AND b.PROCESSLIST_ID="
)


def create_commit_gate(observations):
    release = threading.Event()

    def before_forward(physical, *, prepare):
        assert prepare and type(physical) is int and physical > 0
        assert release.wait(12) and (observations / "java-armed-child-permit-commit").is_file()

    return release, before_forward


def wait_for_competing_recovery(observations, mysql, db, env, out, child, parent, handoff, observer, *,
                               commit_gate=None):
    deadline = time.monotonic() + 10
    while True:
        with observer.lock:
            requests = [row for row in observer.events if row["kind"] == "commit_received" and row["fault"]]
            if requests:
                request, = requests
                greeting, = [row for row in observer.events if row["kind"] == "greeting"
                             and row["connection"] == request["connection"]]
                old = greeting["mysql_connection_id"]
                assert not any(row["connection"] == request["connection"] and row["kind"] == "commit_forwarded"
                               for row in observer.events)
                break
        assert child.poll() is None and parent.poll() is None and time.monotonic() < deadline, "original COMMIT missing"
        time.sleep(.02)
    assert type(old) is int and old > 0
    sql = mysql + [db, "-e"]
    state = guard.reg.command(sql + [AT_HOLD], env, timeout=4)
    guard.put(out / "mysql-competing-recovery-before.tsv", state)
    fields = state.decode().strip().split("\t")
    assert fields[0] == handoff["xid"].encode().hex().upper()
    assert fields[4:] == ["1", "0", "1", "1", "0", "0", "0", "1"]
    started = time.perf_counter_ns()
    guard.put(observations / "java-armed-child-compete", "Original business COMMIT is held\n")
    deadline = time.monotonic() + 4
    while True:
        wait = guard.reg.command(sql + [LOCK_WAIT + str(old)], env, timeout=4)
        if wait.strip():
            break
        assert child.poll() is None and parent.poll() is None and time.monotonic() < deadline, (
            "recovery did not wait on the original 004 row"
        )
        time.sleep(.02)
    guard.put(out / "mysql-competing-recovery-wait.tsv", wait)
    fields = wait.decode().strip().split("\t")
    assert len(fields) == 5 and int(fields[0]) > 0 and int(fields[0]) != old
    assert fields[1:] == [str(old), "seata_rs_at_branch_v1", "PRIMARY", "WAITING"]
    assert child.poll() is None and parent.poll() is None
    assert not (observations / "java-armed-child-race-result.json").exists()
    stamps = dict(recovery_started_monotonic_ns=started, wait_seen_monotonic_ns=time.perf_counter_ns())
    if commit_gate is not None:
        stamps["commit_permitted_monotonic_ns"] = time.perf_counter_ns()
        guard.put(observations / "java-armed-child-permit-commit", "Observed competing recovery row lock\n")
    guard.save(out / "competing-recovery-timing.json", stamps)
    if commit_gate is not None:
        commit_gate.set()


def save_first_recovery(parent, observations, out):
    deadline = time.monotonic() + 10
    while True:
        try:
            result = json.loads((observations / "java-armed-child-race-result.json").read_bytes())
            break
        except (FileNotFoundError, ValueError):
            assert parent.poll() is None and time.monotonic() < deadline, "first competing recovery result missing"
            time.sleep(.02)
    guard.save(out / "competing-first-recovery.json", result)


def verify_competing_recovery(out, wire, result, *, committed):
    raw = load("competing_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    ownership = json.loads((out / "armed-child-ownership.json").read_bytes())
    timing = json.loads((out / "competing-recovery-timing.json").read_bytes())
    first = json.loads((out / "competing-first-recovery.json").read_bytes())
    expected = "Applied" if committed else "NotAppliedFenced"
    assert (first["xid"], first["branch_id"], first["first_recovery"]) == (result["xid"], result["branch_id"], expected)
    state = (out / "mysql-competing-recovery-before.tsv").read_text().strip().split("\t")
    assert state == [*attempt_identity(result), "1", "1", "0", "0", "0", "1"]
    wait = (out / "mysql-competing-recovery-wait.tsv").read_text().strip().split("\t")
    old = ownership["mysql_connection_id"]
    assert int(wait[0]) > 0 and int(wait[0]) != old
    assert wait[1:] == [str(old), "seata_rs_at_branch_v1", "PRIMARY", "WAITING"]

    events = wire["events"]
    request, = [row for row in events if row["kind"] == "commit_received" and row["fault"]]
    waiter, = [row for row in events if row["kind"] == "greeting" and row["mysql_connection_id"] == int(wait[0])]
    greeting, = [row for row in events if row["kind"] == "greeting" and row["connection"] == request["connection"]]
    assert greeting["mysql_connection_id"] == old
    payload = raw.frame(waiter, 0)
    start = payload.index(0, 1) + 1
    assert payload[0] == 10 and int.from_bytes(payload[start:start + 4], "little") == int(wait[0])
    witness, = [row for row in events if row["kind"] == "execute" and row["connection"] == waiter["connection"]
                and row.get("sql") and row["sql"].startswith("SELECT schema_version,xid,branch_id,resource_id,jdbc_profile,")
                and row["sql"].endswith("FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=? FOR UPDATE")]
    assert raw.parameters(witness, witness["statement"], 2) == [result["xid"].encode(), result["branch_id"]]
    started, waited = timing["recovery_started_monotonic_ns"], timing["wait_seen_monotonic_ns"]
    killed = ownership["kill_started_monotonic_ns"]
    assert request["qpc_ns"] < started < witness["qpc_ns"] < waited < killed
    if committed:
        forwarded, = [row for row in events if row["kind"] == "commit_forwarded"
                      and row["connection"] == request["connection"]]
        replied, = [row for row in events if row["kind"] == "commit_reply"
                    and row["connection"] == request["connection"]]
        assert waited <= timing["commit_permitted_monotonic_ns"] < forwarded["qpc_ns"] < replied["qpc_ns"] < killed
        assert not any(row["kind"] == "execute" and row.get("sql") == GUARD for row in events)
    else:
        fence, = [row for row in events if row["kind"] == "execute" and row.get("sql") == GUARD]
        assert killed < fence["qpc_ns"]
    return dict(recovery_while_old_owner_alive=True, original_004_lock_wait=True, first_recovery=expected)

