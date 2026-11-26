"""Observe one terminal MySQL transaction from the owned Java TC AT fixture."""

import importlib.util
import json
from pathlib import Path

import live_guard as guard

TOOLS = Path(__file__).resolve().parents[1]
ROLLBACK_TERMINAL = ("UPDATE seata_rs_at_branch_v1 SET terminal=1,modified_at=now(6) "
                     "WHERE xid=? AND branch_id=? AND phase=2 AND terminal=0")


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def delay_handler_commit(mysql, env):
    initial = guard.reg.command(mysql + ["-e",
        "SELECT @@log_bin,@@sync_binlog,@@binlog_group_commit_sync_delay,"
        "@@binlog_group_commit_sync_no_delay_count"], env, timeout=5)
    assert initial.strip().split(b"\t") == [b"1", b"1", b"0", b"0"]
    guard.reg.command(mysql + ["-e", "SET GLOBAL binlog_group_commit_sync_delay=1000000"], env, timeout=5)


def start_observer(upstream, mysql, db, env, out, phase, mode):
    proxy = load("phase_two_mysql_proxy", TOOLS / "formal-commit-direction/proxy.py")
    terminal = ROLLBACK_TERMINAL if phase == "rollback" else proxy.COMMIT_TERMINAL

    def snapshot(physical_id=None, *, prepare=False):
        if prepare:
            delay_handler_commit(mysql, env)
            return
        # Direct observation while the relay has not yet told the RM that its connection failed.
        query = ("SELECT HEX(xid),branch_id,(SELECT COUNT(*) FROM undo_work),"
                 "(SELECT COUNT(*) FROM undo_log WHERE log_status=0),"
                 "(SELECT COUNT(*) FROM undo_log WHERE log_status=1),"
                 "phase,terminal,commit_terminal,first_phase_committed FROM seata_rs_at_branch_v1")
        if physical_id is None:
            assert mode != "in_flight"
            rows = guard.reg.command(mysql + [db, "-e", query], env, timeout=5)
        else:
            assert mode == "in_flight" and type(physical_id) is int and physical_id > 0
            # The stage and old state are sampled in the same independent statement.
            columns = ("SELECT PROCESSLIST_ID,PROCESSLIST_COMMAND,PROCESSLIST_INFO,PROCESSLIST_STATE,"
                       + query.removeprefix("SELECT "))
            packet = guard.reg.command(mysql + [db, "-e", columns
                + " JOIN performance_schema.threads WHERE PROCESSLIST_ID=" + str(physical_id)], env, timeout=5)
            stage, _, rows = packet.partition(b"\t")
            assert int(stage) == physical_id
            prefix = b"Query\tCOMMIT\twaiting for handler commit\t"
            assert rows.startswith(prefix), packet
            guard.put(out / "mysql-in-flight.tsv", packet)
            rows = rows.removeprefix(prefix)
        guard.put(out / "mysql-at-fault.tsv", rows)

    observer = proxy.UndoRollbackCommitObserver(upstream, mode, terminal, on_fault=snapshot)
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               phase=phase, mode=mode, terminal_sql=terminal))
    return observer


def verify_mysql_fault(wire, out, result, rollback):
    raw = load("phase_two_mysql_parameters", TOOLS / "formal-undo-rollback-loss/validate.py")
    binding = json.loads((out / "mysql-fault-binding.json").read_bytes())
    events = wire["events"]
    mode = wire["mode"]
    terminal = binding["terminal_sql"]
    assert binding["mode"] == mode and wire["claimed"]
    assert not any(event["kind"] == "observer_error" for event in events)
    marker = {"block_terminal": "terminal_execute_blocked", "block_commit": "commit_blocked",
              "in_flight": "commit_in_flight_closed",
              "drop_ok": "commit_reply_dropped"}[mode]
    fault, = [event for event in events if event["kind"] == marker]
    rm = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    callback_kind = 5 if rollback else 3
    callbacks = [event for event in rm if event.get("kind") == callback_kind and event["direction"] == "s2c"]
    callback_replies = [event for event in rm if event.get("kind") == callback_kind + 1
                        and event["direction"] == "c2s"]
    assert callbacks[0]["monotonic_ns"] < fault["qpc_ns"] < callback_replies[0]["monotonic_ns"]

    # Resolve each EXECUTE against the original prepare OK on the same physical connection.
    statements = {}
    terminals = []
    effects = []
    xid, branch = result["xid"], result["branch_id"]
    identity = [xid.encode(), branch]
    expected = ({raw.COMPENSATE: [1], raw.EXACT_REMOVE: [*identity, xid.encode()],
                 raw.GUARD: [branch, xid.encode(), b"serializer=jackson&compressorType=NONE", b"{}"]}
                if rollback else {"DELETE FROM undo_log WHERE xid=? AND branch_id=?": identity})
    for event in events:
        connection = event["connection"]
        if event["kind"] == "prepare_ok":
            packet = raw.frame(event, 1)
            assert len(packet) == 12 and packet[0] == 0
            statement = int.from_bytes(packet[1:5], "little")
            count = int.from_bytes(packet[7:9], "little")
            statements[connection, statement] = event["sql"], count
        elif event["kind"] == "execute" and (event["sql"] == terminal or event["sql"] in expected):
            statement = event["statement"]
            sql = event["sql"]
            values = identity if sql == terminal else expected[sql]
            assert statements[connection, statement] == (sql, len(values))
            assert raw.parameters(event, statement, len(values)) == values
            (terminals if sql == terminal else effects).append(event)

    already_committed = mode == "drop_ok"
    in_flight = mode == "in_flight"
    assert len(terminals) in ((1, 2) if in_flight else (1,) if already_committed else (2,))
    connections = [event["connection"] for event in terminals]
    assert len(set(connections)) == len(terminals) and connections[0] == fault["connection"]
    assert all(effect["connection"] in connections for effect in effects)
    affected = [raw.frame(event, 1) for event in events if event["kind"] == "dml_reply"
                and event["sql"] == terminal]
    assert len(affected) == len(terminals) - int(mode == "block_terminal")
    assert all(len(reply) >= 7 and reply[:3] == b"\x00\x01\x00" and reply[3] & 1
               for reply in affected)

    physical_connections = []
    for execution in terminals:
        connection = execution["connection"]
        session = [event for event in events if event["connection"] == connection]
        greeting, = [event for event in session if event["kind"] == "greeting"]
        packet = raw.frame(greeting, 0)
        id_start = packet.index(0, 1) + 1
        physical_id = int.from_bytes(packet[id_start:id_start + 4], "little")
        assert packet[0] == 10 and physical_id == greeting["mysql_connection_id"] > 0
        physical_connections.append(physical_id)
        begun, = [event for event in session if event["kind"] == "begin_reply_forwarded"]
        assert raw.frame(begun, 1)[0] == 0 and begun["qpc_ns"] < execution["qpc_ns"]
        original_effects = [effect for effect in effects if effect["connection"] == connection]
        assert [effect["sql"] for effect in original_effects] == list(expected)
        assert all(begun["qpc_ns"] < effect["qpc_ns"] < execution["qpc_ns"] for effect in original_effects)
        effect_replies = [event for event in session if event["kind"] == "dml_reply_forwarded"
                          and event["sql"] in expected]
        assert [reply["sql"] for reply in effect_replies] == list(expected)
        assert all(len(packet := raw.frame(reply, 1)) >= 7
                   and packet[:3] == b"\x00\x01\x00" and packet[3] & 1
                   and effect["qpc_ns"] < reply["qpc_ns"] < execution["qpc_ns"]
                   for effect, reply in zip(original_effects, effect_replies, strict=True))
        local = [event for event in session if event["qpc_ns"] > execution["qpc_ns"]]
        requests = [event for event in local if event["kind"] == "commit_received"]
        forwarded = [event for event in local if event["kind"] == "commit_forwarded"]
        replies = [event for event in local if event["kind"] == "commit_reply"]
        first = execution is terminals[0]
        assert len(requests) == int(not (first and mode == "block_terminal"))
        assert len(forwarded) == int(not first or already_committed or in_flight)
        assert len(replies) == int(not first or already_committed)
        assert all(raw.frame(event, 0) == b"\x03COMMIT" for event in requests + forwarded)
        assert all(event["ok"] and len(packet := raw.frame(event, 1)) >= 7
                   and packet[:3] == b"\x00\x00\x00" and not packet[3] & 1 for event in replies)
        if first:
            assert execution["qpc_ns"] < fault["qpc_ns"]
            if already_committed:
                assert forwarded[0]["qpc_ns"] < replies[0]["qpc_ns"] < fault["qpc_ns"]
            if in_flight:
                # The relay closes before the configured binlog group-commit wait elapses.
                assert 0 < fault["qpc_ns"] - forwarded[0]["qpc_ns"] < 1_000_000_000
                stage = (out / "mysql-in-flight.tsv").read_text().strip().split("\t")
                assert int(stage[0]) == fault["mysql_connection_id"] == physical_id
                assert stage[1:4] == ["Query", "COMMIT", "waiting for handler commit"]
        else:
            assert callback_replies[0]["monotonic_ns"] < callbacks[1]["monotonic_ns"] < execution["qpc_ns"]
    assert len(set(physical_connections)) == len(physical_connections)

    first_state = (out / "mysql-at-fault.tsv").read_text().strip().split("\t")
    if in_flight:
        assert stage[4:] == first_state
    assert first_state[:2] == [xid.encode().hex().upper(), str(branch)]
    state = [int(value) for value in first_state[2:]]
    if already_committed:
        assert state == ([0, 0, 1, 2, 1, 0, 1] if rollback else [1, 0, 0, 1, 0, 1, 1])
    else:
        assert state == [1, 1, 0, 2 if rollback else 1, 0, 0, 1]
    return dict(mysql_fault=mode, mysql_at_fault=state, terminal_update_attempts=len(terminals),
                bound_phase_two_effects=len(effects), physical_mysql_connections=physical_connections,
                actual_terminal_commits=1, committed_before_mysql_disconnect=None if in_flight else already_committed,
                commit_in_flight_stage=stage[3] if in_flight else None)
