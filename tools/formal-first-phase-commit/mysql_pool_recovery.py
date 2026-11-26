"""验证原 COMMIT OK 滞留后的业务池回收。"""
import json
import time

import live_guard as guard
from mysql_no_change import read_client_observation
from mysql_phase_two_fault import TOOLS, load

HOLDER_UPDATE = "UPDATE undo_work SET v='pool_holder_no_match' WHERE id=97"
PROBE_UPDATE = "UPDATE undo_work SET v='pool_probe_rolled_back' WHERE id=1"


def permit_pool_probe(client, observations, mysql, db, env, out, observer, *,
                      local_observation="java-no-change-local.json"):
    deadline = time.monotonic() + 12
    while True:
        with observer.lock:
            held = [row for row in observer.events if row["kind"] == "commit_ack_held"]
        if held:
            event, = held
            break
        assert client.poll() is None, (out / "client-stdout.log").read_text(errors="replace")
        assert time.monotonic() < deadline, "original MySQL COMMIT OK did not reach the observer"
        time.sleep(.02)
    old = event["mysql_connection_id"]
    assert type(old) is int and old > 0
    query = mysql + [db, "-e", f"SELECT COUNT(*) FROM performance_schema.threads WHERE PROCESSLIST_ID={old}"]
    active = guard.reg.command(query, env, timeout=4)
    guard.put(out / "mysql-pool-old-session-while-held.tsv", active)
    assert active.strip() == b"1"
    old_observed = time.perf_counter_ns()
    local = read_client_observation(client, observations / local_observation, out)
    assert local["local_outcome"] == "Unknown" and not (observations / "java-pool-probe.json").exists()

    deadline = time.monotonic() + 5
    while True:
        retired = guard.reg.command(query, env, timeout=4)
        if retired.strip() == b"0":
            break
        assert client.poll() is None and time.monotonic() < deadline, "old physical MySQL session still exists"
        time.sleep(.02)
    guard.put(out / "mysql-pool-old-session-after-timeout.tsv", retired)
    guard.save(out / "pool-recovery-timing.json", dict(
        mysql_connection_id=old, old_observed_monotonic_ns=old_observed, probe_permitted_monotonic_ns=time.perf_counter_ns(),
    ))
    guard.put(observations / "java-pool-probe-permitted", "Old physical session retired\n")
    probe = read_client_observation(client, observations / "java-pool-probe.json", out)
    assert probe["xid"] == local["xid"]
    guard.save(out / "pool-probe.json", probe)


def verify_pool_probe(out, wire, ended, *, pool_limit=1, completed_before_ns=None):
    raw = load("pool_recovery_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    timing = json.loads((out / "pool-recovery-timing.json").read_bytes())
    probe = json.loads((out / "pool-probe.json").read_bytes())
    if completed_before_ns is None:
        local = json.loads((out / "no-change-local.json").read_bytes())
        completed_before_ns = local["end_permitted_monotonic_ns"]
    assert (probe["xid"], probe["pool_limit"], probe["probe_affected_rows"], probe["probe_rollback"]) == (
        ended["xid"], pool_limit, 1, "Confirmed"
    )
    assert (out / "mysql-pool-old-session-while-held.tsv").read_bytes().strip() == b"1"
    assert (out / "mysql-pool-old-session-after-timeout.tsv").read_bytes().strip() == b"0"

    events = wire["events"]
    held, = [row for row in events if row["kind"] == "commit_ack_held"]
    assert held["mysql_connection_id"] == timing["mysql_connection_id"]
    old = [row for row in events if row["connection"] == held["connection"]]
    cleanup, = [row for row in old if row["kind"] == "rollback_received"]
    cleanup_sent, = [row for row in old if row["kind"] == "rollback_forwarded"]
    old_quit, = [row for row in old if row["kind"] == "client_command" and row["opcode"] == 1]
    assert raw.frame(cleanup, 0) == raw.frame(cleanup_sent, 0) == b"\x03ROLLBACK"
    assert raw.frame(old_quit, 0) == b"\x01"
    assert held["qpc_ns"] < timing["old_observed_monotonic_ns"] < cleanup["qpc_ns"]
    assert cleanup["qpc_ns"] <= cleanup_sent["qpc_ns"] < old_quit["qpc_ns"] < timing["probe_permitted_monotonic_ns"]
    for reply in [row for row in old if row["kind"] == "rollback_reply_forwarded"]:
        ok = raw.frame(reply, 1)
        assert ok[:3] == b"\x00\x00\x00" and not (ok[3] & 1)
    physical, began, rolled_back = verify_rolled_back_update(raw, events, PROBE_UPDATE, 1)
    permitted = timing["probe_permitted_monotonic_ns"]
    assert physical != timing["mysql_connection_id"]
    assert permitted < began < rolled_back < completed_before_ns
    verified = dict(pool_limit=pool_limit, local_commit_deadline=True, old_mysql_session=timing["mysql_connection_id"],
                    old_session_cleanup_rollbacks=1, replacement_mysql_session=physical,
                    probe_affected_rows=1, probe_rollback_confirmed=True)
    if pool_limit == 2:
        holder, holder_begin, holder_end = verify_rolled_back_update(raw, events, HOLDER_UPDATE, 0)
        assert len({holder, physical, timing["mysql_connection_id"]}) == 3
        assert permitted < holder_begin < began < rolled_back < holder_end < completed_before_ns
        verified["second_replacement_mysql_session"] = holder
        verified["simultaneous_pool_leases"] = 2
    return verified


def verify_rolled_back_update(raw, events, sql, affected):
    execution, = [row for row in events if row["kind"] == "execute" and row.get("sql") == sql]
    fresh = [row for row in events if row["connection"] == execution["connection"]]
    greeting, = [row for row in fresh if row["kind"] == "greeting"]
    packet = raw.frame(greeting, 0)
    start = packet.index(0, 1) + 1
    physical = int.from_bytes(packet[start:start + 4], "little")
    assert packet[0] == 10 and greeting["mysql_connection_id"] == physical > 0
    begin, = [row for row in fresh if row["kind"] == "begin_reply_forwarded"]
    prepare, = [row for row in fresh if row["kind"] == "prepare" and row["sql"] == sql]
    prepared, = [row for row in fresh if row["kind"] == "prepare_ok" and row["sql"] == sql]
    dml_ok, = [row for row in fresh if row["kind"] == "dml_reply_forwarded" and row["sql"] == sql]
    rollback, = [row for row in fresh if row["kind"] == "rollback_forwarded"]
    rolled_back, = [row for row in fresh if row["kind"] == "rollback_reply_forwarded"]
    assert raw.frame(begin, 1)[3] & 1
    assert raw.frame(prepare, 0) == b"\x16" + sql.encode()
    prepare_ok = raw.frame(prepared, 1)
    assert prepare_ok[0] == 0 and prepare_ok[7:9] == b"\x00\x00"
    assert raw.frame(execution, 0) == b"\x17" + prepare_ok[1:5] + b"\x00\x01\x00\x00\x00"
    assert raw.frame(dml_ok, 1)[:3] == bytes((0, affected, 0)) and raw.frame(dml_ok, 1)[3] & 1
    assert raw.frame(rollback, 0) == b"\x03ROLLBACK"
    ok = raw.frame(rolled_back, 1)
    assert ok[:3] == b"\x00\x00\x00" and not (ok[3] & 1)
    assert begin["qpc_ns"] < prepare["qpc_ns"] < prepared["qpc_ns"]
    assert prepared["qpc_ns"] < execution["qpc_ns"] < dml_ok["qpc_ns"] < rollback["qpc_ns"] < rolled_back["qpc_ns"]
    return physical, begin["qpc_ns"], rolled_back["qpc_ns"]
