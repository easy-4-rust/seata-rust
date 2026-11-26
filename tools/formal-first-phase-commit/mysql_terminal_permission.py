"""Verify an original Java TC retry after MySQL rejects the terminal UPDATE."""

import json
import time

import live_guard as guard
from mysql_no_change import read_client_observation
from mysql_phase_two_fault import TOOLS, load

DELETE_UNDO = "DELETE FROM undo_log WHERE xid=? AND branch_id=?"


def restore_after_denial(client, observations, mysql, db, env, out):
    ready = read_client_observation(client, observations / "java-terminal-permission-ready.json", out)
    sql = mysql + [db, "-e"]
    snapshot = ("SELECT HEX(b.xid),b.branch_id,b.phase,b.commit_authorized,b.commit_terminal,"
                "b.first_phase_committed,u.log_status,SHA2(u.rollback_info,256),"
                "(SELECT v FROM undo_work WHERE id=1),a.report_started "
                "FROM seata_rs_at_branch_v1 b JOIN undo_log u ON CAST(u.xid AS BINARY)=b.xid AND u.branch_id=b.branch_id "
                "JOIN seata_rs_at_commit_attempt a ON a.xid=b.xid AND a.branch_id=b.branch_id")
    before = guard.reg.command(sql + [snapshot], env)
    guard.put(out / "permission-before-callback.tsv", before)
    xid, branch, phase, authorization, terminal, witness, status, undo_sha, business, started = before.decode().strip().split("\t")
    assert [xid, branch] == [ready["xid"].encode().hex().upper(), str(ready["branch_id"])]
    assert [phase, authorization, terminal, witness, status, business, started] == [
        "0", "0", "0", "1", "0", "original_tc_report", "1"]
    assert len(undo_sha) == 64
    guard.put(observations / "java-terminal-permission-end-permitted", "Independent Undo snapshot recorded\n")

    deadline = time.monotonic() + 12
    while True:
        rows = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines(keepends=True)
                if line.endswith("\n")]
        replies = [row for row in rows if row.get("kind") == 4 and row["direction"] == "c2s"]
        if replies:
            assert guard.support.payload(replies[0])[-1] == 6, "first callback must be retryable after terminal permission denial"
            break
        assert client.poll() is None and time.monotonic() < deadline, "original TC callback response missing"
        time.sleep(.02)

    retained = guard.reg.command(sql + [snapshot], env)
    guard.put(out / "permission-after-denial.tsv", retained)
    assert retained.decode().strip().split("\t") == [
        xid, branch, "1", "1", "0", "1", "0", undo_sha, "original_tc_report", "1"]
    grant_after = time.perf_counter_ns()
    guard.reg.command(sql + [
        f"GRANT UPDATE (commit_terminal) ON `{db}`.seata_rs_at_branch_v1 TO 'seata_rm_terminal'@'%'"
    ], env)
    binding = dict(ready, undo_sha256=undo_sha, first_retryable_ns=replies[0]["monotonic_ns"],
                   grant_started_ns=grant_after)
    guard.save(out / "permission-restored.json", binding)
    return binding


def verify_permission_commit(mysql, db, env, out, wire, verified, binding, terminal_sql):
    raw = load("permission_mysql_wire", TOOLS / "formal-undo-rollback-loss/validate.py")
    assert wire["mode"] == "observe" and not wire["claimed"]
    events = wire["events"]
    assert not any(row["kind"] == "observer_error" for row in events)
    assert (verified["xid"], verified["branch_id"]) == (binding["xid"], binding["branch_id"])
    assert verified["callback_statuses"][0] == 6 and verified["callback_statuses"][-1] == 5
    assert verified["journal_states"] == [1, 3]
    denials = [row for row in events if row["kind"] == "prepare_error" and row.get("sql") == terminal_sql]
    assert denials
    xid = binding["xid"].encode()
    branch = binding["branch_id"]

    def verify_deleted(rows):
        deleted, = [row for row in rows if row["kind"] == "execute" and row.get("sql") == DELETE_UNDO]
        replied, = [row for row in rows if row["kind"] == "dml_reply_forwarded" and row.get("sql") == DELETE_UNDO]
        assert raw.parameters(deleted, deleted["statement"], 2) == [xid, branch]
        assert raw.frame(replied, 1)[:4] == b"\x00\x01\x00\x03"

    for denied in denials:
        error = raw.frame(denied, 1)
        assert error[:9] == b"\xff\x77\x04#42000" and b"commit_terminal" in error[9:]
        connection = [row for row in events if row["connection"] == denied["connection"]]
        at = connection.index(denied)
        begin = max(i for i, row in enumerate(connection[:at]) if row["kind"] == "begin_reply_forwarded")
        verify_deleted(connection[begin:at])
        ended = next(row for row in connection[at+1:]
                     if row["kind"] in ("rollback_forwarded", "commit_received"))
        assert ended["kind"] == "rollback_forwarded" and raw.frame(ended, 0) == b"\x03ROLLBACK"
    assert denials[0]["qpc_ns"] < binding["first_retryable_ns"] < binding["grant_started_ns"]

    terminal, = [row for row in events if row["kind"] == "execute" and row.get("sql") == terminal_sql]
    assert terminal["qpc_ns"] > binding["grant_started_ns"]
    assert raw.parameters(terminal, terminal["statement"], 2) == [xid, branch]
    connection = [row for row in events if row["connection"] == terminal["connection"]]
    at = connection.index(terminal)
    begin = max(i for i, row in enumerate(connection[:at]) if row["kind"] == "begin_reply_forwarded")
    verify_deleted(connection[begin:at])
    recovered = connection[at+1:]
    updated, = [row for row in recovered if row["kind"] == "dml_reply_forwarded" and row.get("sql") == terminal_sql]
    acknowledged = next(row for row in recovered if row["kind"] == "commit_reply_forwarded")
    assert raw.frame(updated, 1)[:4] == b"\x00\x01\x00\x03"
    assert updated["qpc_ns"] < acknowledged["qpc_ns"] and raw.frame(acknowledged, 1)[:4] == b"\x00\x00\x00\x02"
    result = guard.reg.command(mysql + [db, "-e", "SELECT id,v FROM undo_work"], env)
    guard.put(out / "permission-final-business.tsv", result)
    assert result.strip() == b"1\toriginal_tc_report"
    return dict(mysql_permission_error=1143, denied_callback_attempts=len(denials),
                undo_sha256_preserved_until_grant=binding["undo_sha256"], terminal_commit_confirmed=True)
