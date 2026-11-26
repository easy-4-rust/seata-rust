"""Verify the ordered physical AT compensation of a compound-key branch."""

import json

import live_guard as guard
from mysql_phase_two_fault import TOOLS, load

KEY_A = 9007199254740993
KEY_B = 9007199254740994


def verify_mixed_sql(mysql, db, env, out, wire, result, rollback):
    packets = load("mixed_undo_mysql_parameters", TOOLS / "formal-undo-rollback-loss/validate.py")
    binding = json.loads((out / "mysql-fault-binding.json").read_bytes())
    assert wire["mode"] == "observe" and not wire["claimed"]
    assert not any(event["kind"] == "observer_error" for event in wire["events"])
    xid, branch = result["xid"], result["branch_id"]

    # Java 组合主键串使用原排序；分别核对两列完整值、分隔及锁成员。
    allowed = ({f"{KEY_A}_1", f"{KEY_B}_2"}, {f"1_{KEY_A}", f"2_{KEY_B}"})
    assert len(result["physical_pks"]) == 2 and set(result["physical_pks"]) in allowed
    expected_before = [[KEY_A, 1, "replacement"], [KEY_A, 2, None],
                       [KEY_B, 1, "neighbor"], [KEY_B, 2, "transient"]]
    expected_after = ([[KEY_A, 1, "original"], [KEY_A, 2, None], [KEY_B, 1, "neighbor"]]
                      if rollback else expected_before)
    assert result["before"] == expected_before and result["after"] == expected_after
    physical = guard.reg.command(mysql + [db, "-e", "SELECT id,segment,v FROM undo_work ORDER BY id,segment"], env)
    guard.put(out / "mixed-business-terminal.tsv", physical)
    expected_tsv = ["\t".join(str(v) if v is not None else "NULL" for v in row) for row in expected_after]
    assert physical.decode().splitlines() == expected_tsv

    terminal = binding["terminal_sql"]
    executed, = [event for event in wire["events"] if event["kind"] == "execute" and event["sql"] == terminal]
    connection = executed["connection"]
    session = [event for event in wire["events"] if event["connection"] == connection]
    begun, = [event for event in session if event["kind"] == "begin_reply_forwarded"]
    commit, = [event for event in session if event["kind"] == "commit_forwarded"]
    reply, = [event for event in session if event["kind"] == "commit_reply_forwarded"]
    assert packets.frame(begun, 1)[3] & 1
    assert packets.frame(commit, 0) == b"\x03COMMIT"
    commit_ok = packets.frame(reply, 1)
    assert commit_ok[:3] == b"\x00\x00\x00" and not (commit_ok[3] & 1)
    assert begun["qpc_ns"] < executed["qpc_ns"] < commit["qpc_ns"] < reply["qpc_ns"]
    transactional = [event for event in session if begun["qpc_ns"] < event["qpc_ns"] < commit["qpc_ns"]]
    reads = [event for event in transactional if event["kind"] == "execute"
             and event["sql"] and event["sql"].startswith("SELECT ")]
    root_query = ("SELECT rollback_info" if rollback else "SELECT xid,branch_id,context")
    root_query += " FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE"
    root, = [event for event in reads if event["sql"] == root_query]
    assert packets.parameters(root, root["statement"], 2) == [xid.encode(), branch]
    # Exclude metadata-only reads. Each compensation must be preceded by a current read of its own tuple.
    image_locks = [
        (event, packets.parameters(event, event["statement"], 2))
        for event in reads
        if "FROM `undo_work` WHERE (`id` = ? AND `segment` = ?)" in event["sql"]
        and event["sql"].endswith("FOR UPDATE")
    ]
    assert all(values in ([KEY_A, 1], [KEY_B, 2]) for _, values in image_locks)

    removed = "DELETE FROM `undo_work` WHERE `id` = ? AND `segment` = ?"
    if rollback:
        expected_dml = [
            (removed, [KEY_B, 2]),
            (removed, [KEY_A, 1]),
            ("INSERT INTO `undo_work` (`v`, `id`, `segment`) VALUES (?, ?, ?)", [b"middle", KEY_A, 1]),
            ("UPDATE `undo_work` SET `v` = ? WHERE `id` = ? AND `segment` = ?", [b"original", KEY_A, 1]),
            (packets.EXACT_REMOVE, [xid.encode(), branch, xid.encode()]),
            (packets.GUARD, [branch, xid.encode(), b"serializer=jackson&compressorType=NONE", b"{}"])
        ]
    else:
        expected_dml = [("DELETE FROM undo_log WHERE xid=? AND branch_id=?", [xid.encode(), branch])]
    expected_dml.append((terminal, [xid.encode(), branch]))

    prepared = {}
    for event in session:
        if event["kind"] == "prepare_ok":
            response = packets.frame(event, 1)
            statement = int.from_bytes(response[1:5], "little")
            prepared[statement] = event["sql"], int.from_bytes(response[7:9], "little")
    changes = [event for event in transactional if event["kind"] == "execute" and event["sql"]
               and event["sql"].startswith(("UPDATE ", "DELETE ", "INSERT "))]
    acknowledgments = [event for event in transactional if event["kind"] == "dml_reply_forwarded"
                       and event["sql"].startswith(("UPDATE ", "DELETE ", "INSERT "))]
    if rollback:
        preceding_reply = root["qpc_ns"]
        for change, acknowledgment, values in zip(
                changes[:4], acknowledgments[:4], ([KEY_B, 2], [KEY_A, 1], [KEY_A, 1], [KEY_A, 1]), strict=True):
            assert any(preceding_reply < lock["qpc_ns"] < change["qpc_ns"] and args == values
                       for lock, args in image_locks)
            preceding_reply = acknowledgment["qpc_ns"]
    else:
        assert not image_locks
    for change, acknowledgment, (sql, values) in zip(changes, acknowledgments, expected_dml, strict=True):
        statement = change["statement"]
        assert change["sql"] == acknowledgment["sql"] == sql and prepared[statement] == (sql, len(values))
        assert packets.parameters(change, statement, len(values)) == values
        ok = packets.frame(acknowledgment, 1)
        assert change["qpc_ns"] < acknowledgment["qpc_ns"] and ok[:3] == b"\x00\x01\x00" and ok[3] & 1
    return dict(original_compound_keys=result["physical_pks"], business_rows=expected_after,
                compensation_statements=4 if rollback else 0, locked_current_image_reads=len(image_locks),
                undo_cleanup_with_terminal_in_one_commit=True)
