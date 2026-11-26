"""核对固定原 Java TC 事务中的一次无变化本地 COMMIT。"""
import json
from pathlib import Path
import time

import live_guard as guard
from mysql_phase_two_fault import TOOLS, load

NO_CHANGE_UPDATE = "UPDATE undo_work SET v='absent' WHERE id=99"
BRANCH_COUNTS = (
    "(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),"
    "(SELECT COUNT(*) FROM undo_log),(SELECT COUNT(*) FROM seata_rs_at_branch_v1),"
    "(SELECT COUNT(*) FROM seata_rs_at_commit_attempt)"
)


def start_no_change_observer(upstream, out, *, lose_ack, recycled=False):
    proxy = load("no_change_mysql_proxy", TOOLS / "formal-commit-direction/proxy.py")
    mode = "hold_commit_ack" if recycled else "drop_ok" if lose_ack else "observe"
    observer = proxy.UndoRollbackCommitObserver(upstream, mode, NO_CHANGE_UPDATE)
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               mode=observer.mode, terminal_sql=NO_CHANGE_UPDATE))
    return observer


def read_client_observation(client, path, out):
    deadline = time.monotonic() + 12
    while True:
        try:
            return json.loads(path.read_bytes())
        except (FileNotFoundError, ValueError):
            assert client.poll() is None, (out / "client-stdout.log").read_text(errors="replace")
            assert time.monotonic() < deadline, f"missing client observation {path.name}"
            time.sleep(.02)


def allow_global_end(client, observations, mysql, db, env, out, lose_ack):
    local = read_client_observation(client, observations / "java-no-change-local.json", out)
    assert local["local_outcome"] == ("Unknown" if lose_ack else "Confirmed") and local["affected_rows"] == 0
    sql = mysql + [db, "-e"]
    state = guard.reg.command(sql + [
        f"SELECT HEX(xid),status,{BRANCH_COUNTS} FROM global_table"
    ], env)
    guard.put(out / "mysql-before-global-end.tsv", state)
    assert state.decode().strip().split("\t") == [local["xid"].encode().hex().upper(), "1", *["0"] * 5]
    original = guard.reg.command(sql + ["SELECT id,v FROM undo_work"], env)
    guard.put(out / "mysql-business-before-global-end.tsv", original)
    assert original.strip() == b"1\tuntouched"
    guard.save(out / "no-change-local.json", dict(local, end_permitted_monotonic_ns=time.perf_counter_ns()))
    guard.put(observations / "java-no-change-end-permitted", "Independent MySQL snapshot recorded\n")



def verify_no_change(mysql, db, env, out, wire, ended, journal, *, lose_ack, artifact, recycled=False, rejected_register=False):
    verification = load("no_change_native_frames", TOOLS / "formal-rm-java/validate.py")
    raw = load("no_change_raw_mysql", TOOLS / "formal-undo-rollback-loss/validate.py")

    def frames(role):
        rows = [json.loads(line) for line in (out / f"{role}-frames.jsonl").read_text().splitlines()]
        assert rows[-1]["event"] == "summary" and rows[-1]["succeeded"] and rows[-1]["faults"] == 0
        assert all(row["index"] == index for index, row in enumerate(rows))
        observed = [verification.envelope(row) for row in rows if row["event"] == "frame"]
        assert all(row["action"] == "forward" and row["delivery_count"] == 1 for row in observed)
        assert len({row["connection"] for row in observed}) == 1
        return [row for row in observed if row["request_type"] not in (3, 4)]

    def pair(request, response, request_kind, response_kind):
        assert (request["direction"], request["request_type"], request["kind"]) == ("c2s", 0, request_kind)
        assert (response["direction"], response["request_type"], response["kind"]) == ("s2c", 1, response_kind)
        assert (request["connection"], request["request_id"]) == (response["connection"], response["request_id"])

    # RM 本身已登记，随后没有任何分支操作或 TC 分支回调。
    rm_request, rm_reply, *rm_calls = frames("RM")
    pair(rm_request, rm_reply, 103, 104)
    if rejected_register:
        lock, lock_reply, branch, branch_reply = rm_calls
        pair(lock, lock_reply, 21, 22)
        pair(branch, branch_reply, 11, 12)
        assert lock_reply["body"][2:] == b"\x01\x00\x00\x01"
        assert branch_reply["body"][2] == 0
        assert guard.support.string(lock["body"],2)[0] == ended["xid"]
        assert guard.support.string(branch["body"],2)[0] == ended["xid"]
    else:
        assert not rm_calls
    assert rm_reply["body"][2] == 1 and journal == b"SRMJNL01"
    register_tm, tm_registered, begin, begun, end, finished, *queries = frames("TM")
    pair(register_tm, tm_registered, 101, 102)
    pair(begin, begun, 1, 2)
    end_kind = 9 if lose_ack else 7
    pair(end, finished, end_kind, end_kind + 1)
    xid = ended["xid"]
    assert begun["body"][2:4] == b"\x01\x00" and guard.support.string(begun["body"], 4)[0] == xid
    assert guard.support.string(end["body"], 2)[0] == xid
    assert finished["body"][2:] == bytes((1, 0, 11 if lose_ack else 9))
    assert ended["observed_global_end"] == finished["body"][-1]
    observed_statuses = []
    for query, answer in zip(queries[::2], queries[1::2], strict=True):
        pair(query, answer, 15, 16)
        assert guard.support.string(query["body"], 2)[0] == xid
        assert len(answer["body"]) == 5 and answer["body"][2:4] == b"\x01\x00"
        observed_statuses.append(answer["body"][-1])
    assert observed_statuses == ended["status_queries"] and observed_statuses[-1] == 15
    if rejected_register:
        assert lock_reply["monotonic_ns"] < end["monotonic_ns"]
        assert queries[-1]["monotonic_ns"] < branch["monotonic_ns"]
    expected_interim = (4, 11) if lose_ack else (2, 8, 9)
    assert all(code in expected_interim for code in observed_statuses[:-1])

    local = json.loads((out / "no-change-local.json").read_bytes())
    assert local["xid"] == xid and local["local_outcome"] == ("Unknown" if lose_ack else "Confirmed")
    assert local["affected_rows"] == 0
    assert (out / "mysql-before-global-end.tsv").read_text().strip().split("\t") == [
        xid.encode().hex().upper(), "1", *["0"] * 5
    ]
    assert (out / "mysql-business-before-global-end.tsv").read_bytes().strip() == b"1\tuntouched"
    events = wire["events"]
    mode = "hold_commit_ack" if recycled else "drop_ok" if lose_ack else "observe"
    assert wire["mode"] == mode and wire["claimed"] is lose_ack
    assert not any(row["kind"] == "observer_error" for row in events)
    received, = [row for row in events if row["kind"] == "commit_received"]
    forwarded, = [row for row in events if row["kind"] == "commit_forwarded"]
    server_ok, = [row for row in events if row["kind"] == "commit_reply"]
    control = [row for row in events if row["connection"] == received["connection"]]
    assert received["connection"] == forwarded["connection"] == server_ok["connection"]
    assert received["fault"] is lose_ack and raw.frame(received, 0) == raw.frame(forwarded, 0) == b"\x03COMMIT"
    ok = raw.frame(server_ok, 1)
    assert server_ok["ok"] and ok[:3] == b"\x00\x00\x00" and not (ok[3] & 1)
    assert received["qpc_ns"] <= forwarded["qpc_ns"] < server_ok["qpc_ns"]
    if not recycled:
        assert not any(row["kind"] == "rollback_forwarded" for row in control)
    deliveries = [row for row in events if row["kind"] == "commit_reply_forwarded"]
    withheld = "commit_ack_held" if recycled else "commit_reply_dropped"
    drops = [row for row in events if row["kind"] == withheld]
    if lose_ack:
        assert not deliveries
        delivery, = drops
    else:
        assert not drops
        delivery, = deliveries
        assert raw.frame(delivery, 1) == ok
    assert delivery["connection"] == received["connection"]
    assert server_ok["qpc_ns"] < delivery["qpc_ns"] < local["end_permitted_monotonic_ns"] < end["monotonic_ns"]

    writes = [row for row in events if row["kind"] in ("execute", "query")
              and (row.get("sql") or "").startswith(("INSERT ", "UPDATE ", "DELETE ", "REPLACE "))]
    assert len(writes) == (2 if recycled else 1)
    dml = writes[0]
    assert dml["kind"] == "execute" and dml["connection"] == received["connection"] and dml["sql"] == NO_CHANGE_UPDATE
    prepare, = [row for row in control if row["kind"] == "prepare" and row["sql"] == NO_CHANGE_UPDATE]
    prepared, = [row for row in control if row["kind"] == "prepare_ok" and row["sql"] == NO_CHANGE_UPDATE]
    start_ok, = [row for row in control if row["kind"] == "begin_reply_forwarded"]
    dml_ok, = [row for row in control if row["kind"] == "dml_reply_forwarded" and row["sql"] == NO_CHANGE_UPDATE]
    assert raw.frame(prepare, 0) == b"\x16" + NO_CHANGE_UPDATE.encode()
    prepare_ok = raw.frame(prepared, 1)
    assert prepare_ok[0] == 0 and prepare_ok[7:9] == b"\x00\x00"
    assert raw.frame(dml, 0) == b"\x17" + prepare_ok[1:5] + b"\x00\x01\x00\x00\x00"
    assert raw.frame(start_ok, 1)[3] & 1
    assert raw.frame(dml_ok, 1)[:3] == b"\x00\x00\x00" and raw.frame(dml_ok, 1)[3] & 1
    assert begun["monotonic_ns"] < start_ok["qpc_ns"] < prepare["qpc_ns"] < prepared["qpc_ns"]
    assert prepared["qpc_ns"] < dml["qpc_ns"] < dml_ok["qpc_ns"] < received["qpc_ns"]

    compiled, = [row for line in (out / "cargo-build.jsonl").read_text().splitlines() if line.startswith("{")
                 and (row := json.loads(line)).get("executable") and Path(row["executable"]) == artifact]
    assert "experimental_changed_commit" not in compiled["features"]
    sql = mysql + [db, "-e"]
    counts = guard.reg.command(sql + [f"SELECT (SELECT COUNT(*) FROM global_table),{BRANCH_COUNTS}"], env)
    guard.put(out / "mysql-final-counts.tsv", counts)
    assert counts.strip().split(b"\t") == [b"0"] * 6
    original = guard.reg.command(sql + ["SELECT id,v FROM undo_work"], env)
    guard.put(out / "mysql-final-business.tsv", original)
    assert original.strip() == b"1\tuntouched"
    guard.put(out / "branches.log", journal)
    return dict(xid=xid, local_outcome=local["local_outcome"], original_global_end=ended["observed_global_end"],
                final_global_status="Finished", local_commits=1, commit_acks_delivered=int(not lose_ack),
                branch_registrations=0, branch_reports=0, branch_callbacks=0, default_build=True,
                final_database_counts=[0] * 6)
