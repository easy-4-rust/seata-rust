"""Verify the rejected Rust handshake and the original Java TC's TLS sessions."""

import json
import time

import live_guard as guard
from mysql_no_change import read_client_observation

JAVA_USER = "seata_tc_tls"


def aborted_connects(mysql, env):
    row = guard.reg.command(mysql + ["-e", "SHOW GLOBAL STATUS LIKE 'Aborted_connects'"], env)
    name, count = row.decode().strip().split("\t")
    assert name == "Aborted_connects"
    return int(count)


def permit_trusted_ca(client, observations, mysql, db, env, out, before):
    rejected = read_client_observation(client, observations / "java-mysql-tls-rejected.json", out)
    assert rejected["rust_connect_error"] == "Driver/connect"
    after = aborted_connects(mysql, env)
    assert after > before, "MySQL did not record an aborted handshake"
    sql = mysql + [db, "-e"]
    state = guard.reg.command(sql + [
        "SELECT HEX(xid),status,(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table),"
        "(SELECT COUNT(*) FROM undo_work),(SELECT COUNT(*) FROM undo_log),"
        "(SELECT COUNT(*) FROM seata_rs_at_branch_v1),(SELECT COUNT(*) FROM seata_rs_at_commit_attempt) "
        "FROM global_table"
    ], env)
    guard.put(out / "tls-rejected-state.tsv", state)
    assert state.decode().strip().split("\t") == [rejected["xid"].encode().hex().upper(), "1", *["0"] * 6]
    transport = guard.reg.command(sql + ["SELECT @@require_secure_transport; SHOW SESSION STATUS LIKE 'Ssl_cipher'"], env)
    guard.put(out / "tls-mysql-transport.tsv", transport)
    required, observer = transport.decode().strip().splitlines()
    cipher = observer.removeprefix("Ssl_cipher\t")
    assert required == "1" and cipher.startswith("TLS_")
    java_sessions = guard.reg.command(sql + [
        "SELECT t.PROCESSLIST_ID,s.VARIABLE_VALUE FROM performance_schema.threads t "
        "JOIN performance_schema.status_by_thread s USING (THREAD_ID) "
        f"WHERE t.PROCESSLIST_USER='{JAVA_USER}' AND s.VARIABLE_NAME='Ssl_cipher' ORDER BY t.PROCESSLIST_ID"
    ], env)
    guard.put(out / "tls-java-sessions.tsv", java_sessions)
    sessions = java_sessions.decode().splitlines()
    assert sessions and all(row.split("\t")[1].startswith("TLS_") for row in sessions)
    binding = dict(rejected, aborted_connects_before=before, aborted_connects_after=after,
                   java_tls_sessions=len(sessions), trusted_ca_permitted_ns=time.perf_counter_ns())
    guard.save(out / "tls-admission.json", binding)
    guard.put(observations / "java-mysql-tls-permitted", "Independent MySQL snapshot recorded\n")
    return binding


def verify_original_tls_transaction(mysql, db, env, out, result, admission):
    assert result["xid"] == admission["xid"]
    rows = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    registration, = [row for row in rows if row.get("event") == "frame"
                     and row["direction"] == "c2s" and row["kind"] == 11]
    assert registration["monotonic_ns"] > admission["trusted_ca_permitted_ns"]
    sql = mysql + [db, "-e"]
    journal = guard.reg.command(sql + [
        "SELECT HEX(b.xid),b.branch_id,b.first_phase_committed,a.attempt_state,a.report_started "
        "FROM seata_rs_at_branch_v1 b JOIN seata_rs_at_commit_attempt a USING (xid,branch_id)"
    ], env)
    guard.put(out / "tls-final-local-state.tsv", journal)
    assert journal.decode().strip().split("\t") == [
        result["xid"].encode().hex().upper(), str(result["branch_id"]), "1", "1", "1"]
    business = guard.reg.command(sql + ["SELECT id,v FROM undo_work"], env)
    guard.put(out / "tls-final-business.tsv", business)
    assert business.strip() == b"1\toriginal_tc_report"
    return dict(rust_rejected_wrong_ca=True, mysql_require_secure_transport=True,
                mysql_aborted_handshakes=admission["aborted_connects_after"] - admission["aborted_connects_before"],
                java_tls_sessions=admission["java_tls_sessions"], branch_registrations_before_trusted_ca=0,
                durable_first_phase_witness=1, durable_report_started=1,
                final_business_rows=[[1, "original_tc_report"]])
