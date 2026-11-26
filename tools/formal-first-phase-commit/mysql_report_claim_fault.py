"""核对 007 门闩的原 MySQL OK 被截留，以及原 TC 的后续回滚。"""
import json

import live_guard as guard
from mysql_phase_two_fault import TOOLS, load

CLAIM = ("UPDATE seata_rs_at_commit_attempt SET report_started=1,modified_at=now(6) "
         "WHERE xid=? AND branch_id=? AND report_started=0")
ATTEMPT_COLUMNS = "HEX(a.xid),a.branch_id,HEX(a.resource_id),a.attempt_state,a.report_started"
ATTEMPT = f"SELECT {ATTEMPT_COLUMNS} FROM seata_rs_at_commit_attempt AS a"
AT_FAULT = (
    f"SELECT {ATTEMPT_COLUMNS},b.status,"
    "(SELECT COUNT(*) FROM undo_work),"
    "(SELECT COUNT(*) FROM undo_log WHERE xid=b.xid AND branch_id=b.branch_id AND log_status=0),"
    "(SELECT COUNT(*) FROM lock_table WHERE xid=b.xid AND branch_id=b.branch_id) "
    "FROM seata_rs_at_commit_attempt AS a "
    "JOIN branch_table AS b ON BINARY b.xid=a.xid AND b.branch_id=a.branch_id"
)

def start_report_claim_observer(upstream, mysql, db, env, out):
    proxy = load("report_claim_mysql_proxy", TOOLS / "formal-commit-direction/proxy.py")

    def snapshot():
        # 先读取另一连接已可见的事实，再关闭 Rust 的原连接。
        result = guard.reg.command(mysql + [db, "-e", AT_FAULT], env, timeout=5)
        guard.put(out / "mysql-report-claim-at-fault.tsv", result)

    observer = proxy.UndoRollbackCommitObserver(upstream, "drop_ok", CLAIM, on_fault=snapshot)
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               mode="drop_ok", terminal_sql=CLAIM))
    return observer


def verify_claim_wire(wire, result, *, delivered=False):
    raw = load("report_claim_mysql_parameters", TOOLS / "formal-undo-rollback-loss/validate.py")
    events = wire["events"]
    assert wire["mode"] == ("observe" if delivered else "drop_ok")
    assert wire["claimed"] is (not delivered)
    assert not any(event["kind"] == "observer_error" for event in events)

    execute, = [event for event in events if event["kind"] == "execute" and event["sql"] == CLAIM]
    connection = execute["connection"]
    session = [event for event in events if event["connection"] == connection]
    statement = execute["statement"]
    prepare, = [event for event in session if event["kind"] == "prepare_ok" and event["sql"] == CLAIM]
    packet = raw.frame(prepare, 1)
    assert packet[0] == 0 and int.from_bytes(packet[1:5], "little") == statement
    assert int.from_bytes(packet[7:9], "little") == 2
    xid, branch = result["xid"], result["branch_id"]
    assert raw.parameters(execute, statement, 2) == [xid.encode(), branch]
    applied, = [event for event in session if event["kind"] == "dml_reply" and event["sql"] == CLAIM]
    assert raw.frame(applied, 1)[:3] == b"\x00\x01\x00"

    after_update = [event for event in session if event["qpc_ns"] > applied["qpc_ns"]]
    sent, = [event for event in after_update if event["kind"] == "commit_received"]
    forwarded, = [event for event in after_update if event["kind"] == "commit_forwarded"]
    ok, = [event for event in after_update if event["kind"] == "commit_reply"]
    assert sent["fault"] is (not delivered)
    assert raw.frame(sent, 0) == raw.frame(forwarded, 0) == b"\x03COMMIT"
    assert ok["ok"] is True and raw.frame(ok, 1)[:3] == b"\x00\x00\x00"
    begun = [event for event in session if event["kind"] == "begin_reply_forwarded"
             and event["qpc_ns"] < execute["qpc_ns"]][-1]
    assert raw.frame(begun, 1)[0] == 0
    assert not any(event["kind"] in ("commit_received", "rollback_received")
                   and begun["qpc_ns"] < event["qpc_ns"] < execute["qpc_ns"] for event in session)
    assert prepare["qpc_ns"] < execute["qpc_ns"] < applied["qpc_ns"] < sent["qpc_ns"]
    assert sent["qpc_ns"] < forwarded["qpc_ns"] < ok["qpc_ns"]
    deliveries = [event for event in after_update if event["kind"] == "commit_reply_forwarded"]
    faults = [event for event in events if event["kind"] == "commit_reply_dropped"]
    if delivered:
        delivered_ok, = deliveries
        assert not faults and delivered_ok["wire_hex"] == ok["wire_hex"]
        quit_command, = [event for event in after_update
                         if event["kind"] == "client_command" and event["opcode"] == 1]
        assert raw.frame(quit_command, 0) == b"\x01"
        assert ok["qpc_ns"] < delivered_ok["qpc_ns"] < quit_command["qpc_ns"]
        return quit_command["qpc_ns"]

    fault, = faults
    assert fault["connection"] == connection and ok["qpc_ns"] < fault["qpc_ns"]
    assert not deliveries
    return fault["qpc_ns"]


def verify_report_claim_loss(mysql, db, env, out, wire, result):
    assert result["report_claim_unknown_with_started_marker"] is True
    fault_time = verify_claim_wire(wire, result)
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
    rollback, = [event for event in tm if event.get("kind") == 9 and event["direction"] == "c2s"]
    assert fault_time < rollback["monotonic_ns"]
    verify_claim_rows(mysql, db, env, out, result, "mysql-report-claim-at-fault.tsv")
    return dict(mysql_report_claim_commits=1, mysql_report_claim_ok_delivered=0,
                durable_report_started=1, phase_one_report_recovery="AlreadyStarted")


def verify_claim_rows(mysql, db, env, out, result, snapshot):
    xid, branch = result["xid"], result["branch_id"]
    identity = [xid.encode().hex().upper(), str(branch), result["resource_id"].encode().hex().upper(), "1", "1"]
    assert (out / snapshot).read_text().strip().split("\t") == [
        *identity, "1", "1", "1", "1"
    ], "Registered branch, original lock and committed SQL/Undo must survive until global rollback"
    final = guard.reg.command(mysql + [db, "-e", ATTEMPT], env)
    guard.put(out / "final-report-claim.tsv", final)
    assert final.decode().strip().split("\t") == identity
