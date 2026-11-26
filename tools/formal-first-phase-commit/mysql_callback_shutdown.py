"""Hold the real callback COMMIT OK until owned RM shutdown retires its worker."""
import json
import time
import live_guard as guard
from mysql_phase_two_fault import TOOLS, load


def permit_shutdown(client, observations, mysql, db, env, out, observer):
    rollback = env.get("SEATA_CALLBACK_ROLLBACK_SHUTDOWN") == "1"
    deadline = time.monotonic() + 15
    while True:
        with observer.lock:
            held = [row.copy() for row in observer.events if row["kind"] == "commit_ack_held"]
        if held:
            break
        assert client.poll() is None and time.monotonic() < deadline, "callback COMMIT OK was not held"
        time.sleep(.02)
    event, = held
    state = guard.reg.command(mysql + [db, "-e",
        "SELECT HEX(xid),branch_id,phase,commit_authorized,commit_terminal,first_phase_committed,"
        "(SELECT COUNT(*) FROM undo_log),(SELECT COUNT(*) FROM undo_work),OCTET_LENGTH(undo_sha256) "
        + (",terminal " if rollback else "") + "FROM seata_rs_at_branch_v1"], env)
    row = state.decode().strip().split("\t")
    assert row[2:] == (["2", "0", "0", "1", "1", "0", "32", "1"] if rollback else ["1", "1", "1", "1", "0", "1", "32"])
    guard.put(out / "callback-commit-before-shutdown.tsv", state)
    guard.save(out / "callback-shutdown-permitted.json", dict(
        commit_mysql_session=event["mysql_connection_id"], original_connection=event["connection"],
        held_ns=event["qpc_ns"], shutdown_permitted_ns=time.perf_counter_ns()))
    guard.put(observations / "java-callback-shutdown-permitted", "Original callback commit independently observed\n")


def verify_shutdown(rows, result, out, mysql_wire, *, rollback=False, in_flight=False):
    decoder = load("shutdown_callback_native", TOOLS / "formal-rm-java/validate.py")
    raw = load("shutdown_callback_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")
    frames = [decoder.envelope(row) for row in rows if row.get("event") == "frame"]

    def select(kind, direction):
        return [row for row in frames if row["kind"] == kind and row["direction"] == direction]

    old, new = select(103, "c2s")
    identified = select(104, "s2c")
    assert len(identified) == 2
    for request, response in zip((old, new), identified, strict=True):
        assert (request["connection"], request["request_id"]) == (response["connection"], response["request_id"])
        assert response["body"][2] == 1
    assert old["connection"] != new["connection"]
    first, retry = select(5 if rollback else 3, "s2c")
    assert first["connection"] == old["connection"] and retry["connection"] == new["connection"]
    assert first["body"] == retry["body"]
    completed, = select(6 if rollback else 4, "c2s")
    assert (completed["connection"], completed["request_id"]) == (retry["connection"], retry["request_id"])
    assert completed["body"][2:4] == b"\x01\x00" and completed["body"][-1] == (8 if rollback else 5)
    report, = select(13, "c2s")
    assert report["connection"] == old["connection"]
    identity = guard.support.branch_identity(retry, True)
    assert (identity["xid"], identity["branch_id"], identity["resource"]) == (
        result["xid"], result["branch_id"], result["resource_id"])
    binding = json.loads((out / "callback-shutdown-permitted.json").read_bytes())
    assert first["monotonic_ns"] < binding["held_ns"] < binding["shutdown_permitted_ns"] < new["monotonic_ns"]
    restarted = json.loads((out / "graceful-rm-restart.json").read_bytes())
    if in_flight:
        before = (out / "mysql-before-rm-shutdown.tsv").read_bytes()
        assert before == (out / "mysql-after-rm-shutdown.tsv").read_bytes()
        state = before.decode().strip().split("\t")
        assert state[:4] == [str(binding["commit_mysql_session"]), "Query", "COMMIT", "waiting for handler commit"]
        assert state[4:] == [result["xid"].encode().hex().upper(), str(result["branch_id"]), "1","1","0","1","0","0","1"]
    else:
        state = (out / "callback-commit-before-shutdown.tsv").read_text().strip().split("\t")
        assert state == [result["xid"].encode().hex().upper(), str(result["branch_id"]),
                         * (["2", "0", "0", "1", "1", "0", "32", "1"] if rollback else ["1", "1", "1", "1", "0", "1", "32"])]
    assert restarted["pending_after_shutdown"] and restarted["old_pool_closed"]
    assert mysql_wire["mode"] == ("in_flight" if in_flight else "hold_commit_ack") and mysql_wire["claimed"]
    events = mysql_wire["events"]
    assert not any(row["kind"] == "observer_error" for row in events)
    terminal_sql = "UPDATE seata_rs_at_branch_v1 SET terminal=1" if rollback else "UPDATE seata_rs_at_branch_v1 SET commit_terminal=1"
    terminal, = [row for row in events if row["kind"] == "execute" and (row.get("sql") or "").startswith(
        terminal_sql)]
    received, = [row for row in events if row["kind"] == "commit_received" and row["fault"]]
    held, = [row for row in events if row["kind"] == ("commit_in_flight_closed" if in_flight else "commit_ack_held")]
    original_connection = held["connection"] if in_flight else binding["original_connection"]
    assert terminal["connection"] == received["connection"] == held["connection"] == original_connection
    if in_flight:
        assert held["mysql_connection_id"] == binding["commit_mysql_session"]
        assert raw.frame(received, 0) == b"\x03COMMIT"
        forwarded, = [row for row in events if row["kind"] == "commit_forwarded"
                      and row["connection"] == held["connection"] and row["qpc_ns"] > received["qpc_ns"]]
        assert raw.frame(forwarded, 0) == b"\x03COMMIT"
        greeting, = [row for row in events if row["kind"] == "greeting" and row["connection"] == held["connection"]]
        packet = raw.frame(greeting, 0)
        version_end = packet.index(0, 1)
        assert int.from_bytes(packet[version_end+1:version_end+5], "little") == binding["commit_mysql_session"]

    assert raw.parameters(terminal, terminal["statement"], 2) == [result["xid"].encode(), result["branch_id"]]
    if in_flight:
        assert not any(row["kind"] == "commit_reply" and row["connection"] == held["connection"]
                       and row["qpc_ns"] > received["qpc_ns"] for row in events)
    else:
        original_ok, = [row for row in events if row["kind"] == "commit_reply"
                        and row["connection"] == held["connection"] and row["qpc_ns"] > received["qpc_ns"]]
        assert raw.frame(original_ok, 1)[:3] == b"\x00\x00\x00"
        assert original_ok["qpc_ns"] < held["qpc_ns"]
    assert not any(row["kind"] == "commit_reply_forwarded" and row["connection"] == held["connection"]
                   and row["qpc_ns"] > received["qpc_ns"] for row in events)
    deleted, = [row for row in events if row["kind"] == "dml_reply_forwarded"
                and (row.get("sql") or "") == (raw.EXACT_REMOVE if rollback else "DELETE FROM undo_log WHERE xid=? AND branch_id=?")]
    assert deleted["connection"] == terminal["connection"] and raw.frame(deleted, 1)[:3] == b"\x00\x01\x00"
    assert deleted["qpc_ns"] < terminal["qpc_ns"] < held["qpc_ns"] < retry["monotonic_ns"]
    if rollback:
        compensation, = [row for row in events if row["kind"] == "execute" and row.get("sql") == raw.COMPENSATE]
        fence, = [row for row in events if row["kind"] == "execute" and row.get("sql") == raw.GUARD]
        assert raw.parameters(compensation, compensation["statement"], 1) == [1]
        assert raw.parameters(fence, fence["statement"], 4)[:2] == [result["branch_id"], result["xid"].encode()]
        assert compensation["connection"] == fence["connection"] == terminal["connection"]
        assert compensation["qpc_ns"] < deleted["qpc_ns"] < fence["qpc_ns"] < terminal["qpc_ns"]
        tm = [decoder.envelope(json.loads(line)) for line in (out / "TM-frames.jsonl").read_text().splitlines()
              if json.loads(line).get("event") == "frame"]
        ends = [row for row in tm if row["kind"] == 9 and row["direction"] == "c2s"]
        end, = ends
        responses = [row for row in tm if row["kind"] == 10 and row["direction"] == "s2c"]
        assert len(responses) <= 1
        if result["observed_end"] is not None:
            ended, = responses
            assert ended["body"][2:] == bytes((1,0,result["observed_end"]))
        for ended in responses:
            assert (end["connection"],end["request_id"]) == (ended["connection"],ended["request_id"])
            assert ended["body"][2:4] == b"\x01\x00"
        final_status = [row for row in tm if row["kind"] == 16 and row["direction"] == "s2c"][-1]
        assert final_status["body"][2:] == b"\x01\x00\x0f"
        assert guard.support.string(end["body"],2)[0] == result["xid"]
        assert not any(row["kind"] == 7 and row["direction"] == "c2s" for row in tm)
        states = [record["state"] for record in decoder.journal_records((out / "branches.log").read_bytes(), first["body"])]
        assert states == [1,3]
    guard.save(out / "callback-shutdown-wire.json", dict(
        original_connection=old["connection"], replacement_connection=new["connection"],
        callback_requests=2, callback_success_replies=1, original_undo_deletions=1,
        original_terminal_writes=1, original_commit_ack_held=not in_flight, original_commit_in_flight=in_flight, durable_pending_after_shutdown=True))
