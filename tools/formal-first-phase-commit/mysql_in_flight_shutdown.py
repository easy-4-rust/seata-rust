"""Independently sample an original MySQL COMMIT before and after RM shutdown."""
import json
import time
import live_guard as guard
from mysql_phase_two_fault import ROLLBACK_TERMINAL, TOOLS, delay_handler_commit, load


def start_observer(upstream, mysql, db, env, out, observations):
    rollback = env.get("SEATA_CALLBACK_ROLLBACK_SHUTDOWN") == "1"
    proxy = load("shutdown_in_flight_proxy", TOOLS / "formal-commit-direction/proxy.py")

    def snapshot(physical_id=None, *, prepare=False):
        if prepare:
            delay_handler_commit(mysql, env)
            return
        assert type(physical_id) is int and physical_id > 0
        sql = (
            "SELECT PROCESSLIST_ID,PROCESSLIST_COMMAND,PROCESSLIST_INFO,PROCESSLIST_STATE,"
            "HEX(xid),branch_id,(SELECT COUNT(*) FROM undo_work),"
            "(SELECT COUNT(*) FROM undo_log WHERE log_status=0),"
            "(SELECT COUNT(*) FROM undo_log WHERE log_status=1),"
            "phase,terminal,commit_terminal,first_phase_committed "
            "FROM seata_rs_at_branch_v1 JOIN performance_schema.threads "
            f"WHERE PROCESSLIST_ID={physical_id}"
        )

        def observe():
            packet = guard.reg.command(mysql + [db, "-e", sql], env, timeout=4)
            fields = packet.decode().strip().split("\t")
            assert fields[:4] == [str(physical_id), "Query", "COMMIT", "waiting for handler commit"], packet
            assert fields[6:] == ["1", "1", "0", "2" if rollback else "1", "0", "0", "1"], packet
            return packet

        before = observe()
        guard.put(out / "mysql-before-rm-shutdown.tsv", before)
        observed_ns = time.perf_counter_ns()
        permitted_ns = time.perf_counter_ns()
        guard.put(observations / "java-callback-shutdown-permitted", "Original MySQL COMMIT is executing\n")
        deadline = time.monotonic() + 4
        closed = observations / "java-rm-owner-closed.json"
        while not closed.is_file():
            assert time.monotonic() < deadline, "RM did not close during original COMMIT"
            time.sleep(.002)
        owner = json.loads(closed.read_bytes())
        after = observe()
        assert after == before
        fields = before.decode().strip().split("\t")
        assert fields[4] == owner["xid"].encode().hex().upper() and int(fields[5]) == owner["branch_id"]
        guard.put(out / "mysql-after-rm-shutdown.tsv", after)
        guard.save(out / "callback-shutdown-permitted.json", dict(
            commit_mysql_session=physical_id, held_ns=observed_ns,
            shutdown_permitted_ns=permitted_ns, owner=owner, in_flight=True))
        guard.reg.command(mysql + ["-e", "SET GLOBAL binlog_group_commit_sync_delay=0"], env, timeout=4)
        guard.put(observations / "java-rm-closed-snapshot-recorded", "Original COMMIT still executing after RM shutdown\n")

    return proxy.UndoRollbackCommitObserver(upstream, "in_flight",
        ROLLBACK_TERMINAL if rollback else proxy.COMMIT_TERMINAL, on_fault=snapshot)
