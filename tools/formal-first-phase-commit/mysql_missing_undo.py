"""核对缺失 Normal Undo 时，原分支围栏的实际 MySQL 事务。"""

import live_guard as guard
from mysql_phase_two_fault import ROLLBACK_TERMINAL, TOOLS, load


def verify_missing_undo(mysql, db, env, out, wire, result):
    assert wire["mode"] == "observe" and not wire["claimed"]
    events = wire["events"]
    assert not any(event["kind"] == "observer_error" for event in events)
    xid, branch = result["xid"], result["branch_id"]
    context = "serializer=jackson&compressorType=NONE"
    assert result["first_phase_committed"] == 0 and result["late_normal_error"] == 1062
    before = result["before_late_normal"]
    assert before == result["after_late_normal"] and before[:5] == [branch, xid, context, "7B7D", 1]
    physical = guard.reg.command(mysql + [db, "-e",
        "SELECT branch_id,xid,context,HEX(rollback_info),log_status,CAST(log_created AS CHAR),"
        "CAST(log_modified AS CHAR),(SELECT first_phase_committed FROM seata_rs_at_branch_v1) FROM undo_log"], env)
    guard.put(out / "missing-undo-terminal.tsv", physical)
    assert physical.decode().splitlines() == ["\t".join(map(str, [*before, 0]))]

    verify_fence_with_rollback_terminal(events, xid, branch)
    return dict(first_phase_committed=0, compensation_statements=0, late_normal_mysql_error=1062,
                physical_fence_inserts=1, fence_and_terminal_in_one_commit=True)


def verify_fence_with_rollback_terminal(events, xid, branch):
    packets = load("missing_undo_mysql_parameters", TOOLS / "formal-undo-rollback-loss/validate.py")
    context = "serializer=jackson&compressorType=NONE"
    changes = [event for event in events if event["kind"] == "execute"
               and event["sql"] in (packets.GUARD, ROLLBACK_TERMINAL)]
    fence, terminal = changes
    assert fence["connection"] == terminal["connection"]
    session = [event for event in events if event["connection"] == terminal["connection"]]
    begin = max((event for event in session if event["kind"] == "begin_reply_forwarded"
                 and event["qpc_ns"] < fence["qpc_ns"]), key=lambda event: event["qpc_ns"])
    commit, = [event for event in session if event["kind"] == "commit_forwarded"]
    reply, = [event for event in session if event["kind"] == "commit_reply_forwarded"]
    assert packets.frame(begin, 1)[3] & 1 and packets.frame(commit, 0) == b"\x03COMMIT"
    ok = packets.frame(reply, 1)
    assert ok[:3] == b"\x00\x00\x00" and not (ok[3] & 1)
    assert begin["qpc_ns"] < fence["qpc_ns"] < terminal["qpc_ns"] < commit["qpc_ns"] < reply["qpc_ns"]
    commands = [event for event in session if event["kind"] == "execute" and event["sql"]
                and begin["qpc_ns"] < event["qpc_ns"] < commit["qpc_ns"]]
    assert [event for event in commands if event["sql"].startswith(("INSERT ", "UPDATE ", "DELETE "))] == changes
    root_sql = ("SELECT xid,branch_id,log_status,context,CAST(OCTET_LENGTH(rollback_info) AS SIGNED)"
                " AS bytes FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE")
    child_sql = "SELECT branch_id FROM undo_log WHERE xid=? AND context=? LIMIT 1 FOR UPDATE"
    checks = [
        (next(event for event in commands if event["sql"] == root_sql), root_sql, [xid.encode(), branch]),
        (fence, packets.GUARD, [branch, xid.encode(), context.encode(), b"{}"]),
        (next(event for event in commands if event["sql"] == child_sql), child_sql,
         [xid.encode(), f"branchId={branch}".encode()]),
        (terminal, ROLLBACK_TERMINAL, [xid.encode(), branch]),
    ]
    assert [event["qpc_ns"] for event, _, _ in checks] == sorted(event["qpc_ns"] for event, _, _ in checks)
    for event, sql, expected in checks:
        assert event["sql"] == sql and packets.parameters(event, event["statement"], len(expected)) == expected
    return fence["qpc_ns"], reply["qpc_ns"]
