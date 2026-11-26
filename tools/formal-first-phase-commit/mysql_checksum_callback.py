"""Keep corrupted Undo until the original TC receives a retryable callback."""
import json
import time
import live_guard as guard
from mysql_no_change import read_client_observation


def restore_after_callback(client, observations, mysql, db, env, out):
    recovery = read_client_observation(client, observations / "java-undo-checksum.json", out)
    assert recovery["modified_recovery_rejected"] and recovery["original_bytes_restored"]
    assert recovery["missing_checksum_report_rejected"] and recovery["modified_undo_report_rejected"]
    guard.save(out / "checksum-recovery.json", recovery)
    state = read_client_observation(client, observations / "java-checksum-callback.json", out)
    rollback = env.get("SEATA_CHECKSUM_ROLLBACK") == "1"
    deadline = time.monotonic() + 12
    while True:
        rows = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines(keepends=True)
                if line.endswith("\n")]
        replies = [row for row in rows if row.get("kind") == (6 if rollback else 4) and row["direction"] == "c2s"]
        if replies:
            assert guard.support.payload(replies[0])[-1] == (9 if rollback else 6)
            break
        assert client.poll() is None and time.monotonic() < deadline
        time.sleep(.02)
    original = bytes(state.pop("original"))
    snapshot = guard.reg.command(mysql + [db, "-e",
        "SELECT schema_version,HEX(undo_sha256),first_phase_committed,commit_terminal,"
        "(SELECT SHA2(rollback_info,256) FROM undo_log WHERE log_status=0) FROM seata_rs_at_branch_v1"], env)
    version, digest, witness, terminal, actual = snapshot.decode().strip().split("\t")
    import hashlib
    assert version == "4" and witness == "1" and terminal == "0"
    assert digest.lower() == hashlib.sha256(original).hexdigest() and digest.lower() != actual
    business = guard.reg.command(mysql + [db, "-e",
        "SELECT (SELECT COUNT(*) FROM undo_work WHERE id=1 AND v='original_tc_report'),"
        "(SELECT terminal FROM seata_rs_at_branch_v1),"
        "(SELECT COUNT(*) FROM undo_log WHERE log_status=0)"], env)
    assert business.strip().split(b"\t") == [b"1", b"0", b"1"]
    guard.put(out / "checksum-rejected-business.tsv", business)
    guard.put(out / "checksum-rejected-callback.tsv", snapshot)
    guard.reg.command(mysql + [db, "-e",
        "UPDATE undo_log SET rollback_info=UNHEX('" + original.hex() + "') WHERE log_status=0"], env)
    guard.save(out / "checksum-restored.json", dict(state, callback_rejected=True, original_sha256=digest.lower()))
