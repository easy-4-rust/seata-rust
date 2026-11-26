"""Validate exact MySQL rollback-effect packets and transport fault results."""
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("managed_rollback_base", HERE.parent/"formal-commit-direction/validate.py")
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
need, frame = base.need, base.frame
TERMINAL = "UPDATE seata_rs_at_branch_v1 SET terminal=1,modified_at=now(6) WHERE xid=? AND branch_id=? AND phase=2 AND terminal=0"
COMPENSATE = "DELETE FROM `undo_work` WHERE `id` = ?"
spec = importlib.util.spec_from_file_location(
    "undo_delete_contract", HERE.parent/"formal-undo-rollback-loss/validate.py"
)
undo_delete_contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(undo_delete_contract)
GUARD = "INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))"


def verify(wire, observed, checkpoint_ns=None):
    events = wire["events"]
    mode = wire["mode"]
    need(mode in ("drop_ok", "block_commit", "delay_ok") and observed["fault"] == mode and wire["claimed"],
         "original intended fault was not taken")
    need(not any(e["kind"] == "observer_error" for e in events), "observer errored")
    xid = b"commit-check:consumed-guard"
    remove, identity = undo_delete_contract.root_delete(wire, xid, 42)
    expected = {COMPENSATE: [1], remove: identity,
                GUARD: [42, xid, b"serializer=jackson&compressorType=NONE", b"{}"],
                TERMINAL: [xid, 42]}
    attempts = 2 if mode == "block_commit" else 1
    executes = [e for e in events if e["kind"] == "execute"]
    business = [e["sql"] for e in events if e["kind"] in ("query", "execute")
                and e["sql"].upper().startswith(("INSERT ", "UPDATE ", "DELETE ", "REPLACE "))
                and "undo_work" in e["sql"].lower()]
    need(business == ["INSERT INTO undo_work(id,v) VALUES(1,'prepared_fixture')",
                      "INSERT INTO undo_work(id,v) VALUES(2,'stale-local')",
                      *([COMPENSATE]*attempts)], "business mutation stream was not limited to the fixture and compensation")
    for sql in expected:
        need(sum(e["sql"] == sql for e in executes) == attempts, f"unexpected execute count: {sql}")
    fault, = [e for e in events if e["kind"] == "commit_received" and e["fault"]]
    terminals = [e for e in executes if e["sql"] == TERMINAL]
    need(terminals[0]["connection"] == fault["connection"], "fault was not on the first rollback effect")
    need(len({e["connection"] for e in terminals}) == attempts, "rollback reused an uncertain session")
    for index, terminal in enumerate(terminals):
        session = [e for e in events if e["connection"] == terminal["connection"]]
        first = index == 0
        begin = base.one(session, "begin_reply")
        commit = base.one(session, "commit_received")
        changes = [e for e in session if e["kind"] == "execute"
                   and e["sql"].startswith(("DELETE ", "UPDATE ", "INSERT "))]
        need([e["sql"] for e in changes] == list(expected), "original rollback mutation order differs")
        need(begin["qpc_ns"] < changes[0]["qpc_ns"] < commit["qpc_ns"] and frame(commit,0) == b"\x03COMMIT",
             "original rollback transaction boundaries differ")
        for change in changes:
            need(base.values(events,change) == expected[change["sql"]], "original rollback mutation parameters differ")
            base.delivered(session,change)
            reply = base.one(session,"dml_reply",change["sql"])
            # The relay logs delivery after sendall, which can race the next client command.
            need(reply["qpc_ns"] < commit["qpc_ns"], "DML server reply must precede COMMIT")
        if first and mode == "block_commit":
            base.one(session,"commit_blocked")
            need(not any(e["kind"] in ("commit_forwarded","commit_reply") for e in session), "blocked COMMIT reached MySQL")
        else:
            forward = base.one(session,"commit_forwarded")
            reply = base.one(session,"commit_reply")
            base.base.ok_packet(reply,False,0)
            need(frame(forward,0) == frame(commit,0) and commit["qpc_ns"] < forward["qpc_ns"] < reply["qpc_ns"],
                 "actual COMMIT or MySQL OK differs")
            if first:
                if mode == "delay_ok":
                    held = base.one(session,"commit_reply_held")
                    late, = [e for e in session if e["kind"] in ("late_reply_sent","late_reply_peer_closed")]
                    need(checkpoint_ns is not None and reply["qpc_ns"] < held["qpc_ns"]
                         < checkpoint_ns < late["qpc_ns"] and late["wire_hex"] == reply["wire_hex"],
                         "retryable checkpoint did not precede the exact late OK")
                else:
                    base.one(session,"commit_reply_dropped")
                need(not any(e["kind"] == "commit_reply_forwarded" for e in session), "uncertain OK was forwarded on time")
            else:
                delivery = base.one(session,"commit_reply_forwarded")
                need(delivery["wire_hex"] == reply["wire_hex"], "recovery COMMIT OK differs")
    forwarded = sum(1 for e in events if e["kind"] == "commit_forwarded"
                    and e["connection"] in {t["connection"] for t in terminals})
    need(forwarded == 1, "expected exactly one executed rollback COMMIT")
    return dict(rollback_control_attempts=attempts,actual_rollback_commits=forwarded,
                compensation_executes=attempts,guard_executes=attempts,terminal_executes=attempts)
