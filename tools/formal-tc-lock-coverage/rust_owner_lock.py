"""原 Rust AT 分支持锁时，用真实 Java 客户端核对拒绝与本地回滚。"""

import json
import struct

import mixed_java_lock as java_lock
from live import need, put, reg, save, support

STAGES = ("rust-held-java-conflict", "rust-released-java-admitted")
COUNTS = ("SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
          "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log)")
def verify_java_frames(frames, phase, accepted, account):
    request, = [row for direction, row in frames if direction == "sdk-to-tc" and row["code"] == 11]
    reply, = [row for direction, row in frames if direction == "tc-to-sdk" and row["code"] == 12]
    end_kind = 7 if accepted else 9
    end, = [row for direction, row in frames if direction == "sdk-to-tc" and row["code"] in (7, 9)]
    outcome, = [row for direction, row in frames if direction == "tc-to-sdk" and row["code"] == end_kind + 1]
    need((request["connection"], request["id"]) == (reply["connection"], reply["id"])
         and (request["xid"], request["resource"], request["lock_key"], request["branch_type"])
         == (phase["xid"], phase["resource"], account.lock_key, 0), "Original Java lock identity differs")
    need((end["code"], end["xid"], end["connection"], end["id"])
         == (end_kind, phase["xid"], outcome["connection"], outcome["id"])
         and outcome["result"] == 1 and outcome["error"] == 0,
         "Original Java end reply differs")
    if accepted:
        need((reply["result"], reply["error"], reply["branch_id"], outcome["status"])
             == (1, 0, phase["branch_id"], 9), "Original Java success reply differs")
    else:
        need(reply["result"] == 0 and reply["branch_id"] == 0 and reply["error"] in (2, 20),
             "Original Java lock conflict response differs")
    return reply["error"]


def observe(stage, expected, out, sql, env, start_actor, account):
    conflict = stage == STAGES[0]
    if conflict:
        before = reg.command(sql + [account.select_state], env)
        put(out / "rust-held-before-java.tsv", before)
        rows = before.decode().splitlines()
        rust_id = [expected["xid"], str(expected["branch_id"])]
        need(rows[:-1] == [account.lock_row(expected), *account.business_rows(990)]
             and rows[-1].split("\t")[:3] == rust_id + ["0"]
             and len(rows[-1].split("\t")[3]) > 0, "Rust owner lacks durable original lock and Normal Undo")
        witness = reg.command(sql + [
            "SELECT b.first_phase_committed,a.report_started FROM seata_rs_at_branch_v1 b "
            "JOIN seata_rs_at_commit_attempt a USING(xid,branch_id)"
        ], env)
        put(out / "rust-commit-witness.tsv", witness)
        need(witness.strip() == b"1\t1", "Rust owner commit witness differs")
    else:
        counts = reg.command(sql + [COUNTS], env)
        put(out / "rust-released-before-java.tsv", counts)
        need(counts.strip() == b"0\t0\t0\t0", "Rust owner did not release lock and Undo")
    actor = start_actor(conflict)
    work, _, relay, process = actor
    try:
        if conflict:
            path = work / "conflict.json"
            java_lock.wait(path.exists, process, "Original Java client did not report the lock conflict")
            phase = json.loads(path.read_bytes())
            need(phase["process_id"] == process.pid and phase["resource"] == expected["resource"]
                 and phase["xid"] != expected["xid"] and phase["pending_balance"] == 890
                 and phase["balance_after_rejection"] == 990
                 and phase["lock_conflict"] in ("LockKeyConflict", "LockKeyConflictFailFast"),
                 "Original Java transaction did not automatically roll back")
            java_lock.wait(lambda: reg.command(sql + [COUNTS], env).strip() == b"1\t1\t1\t1",
                           process, "Rejected Java XID did not finish")
            after = reg.command(sql + [account.select_state], env)
            put(out / "rust-held-after-java.tsv", after)
            need(before == after, "Rejected Java attempt changed durable Rust state")
        else:
            phase = java_lock.held(actor, out)
            need(phase["resource"] == expected["resource"] and phase["xid"] != expected["xid"],
                 "Admitted Java identity differs")
            actual = reg.command(sql + [account.select_state], env)
            put(out / "java-admitted-after-rust.tsv", actual)
            undo = [phase["xid"], str(phase["branch_id"]), "0", phase["undo_hex"]]
            need(actual.decode().splitlines()
                 == [account.lock_row(phase), *account.business_rows(890), "\t".join(undo)],
                 "Admitted Java branch does not own the row")
            put(work / "release.ready", "Original Java branch observed\n")
            java_lock.wait((work / "finished.ready").exists, process, "Original Java commit did not finish")
            java_lock.wait(lambda: reg.command(sql + [COUNTS], env).strip() == b"0\t0\t0\t0",
                           process, "Admitted Java branch did not release lock and Undo")
        phase["wire_conflict_code"] = verify_java_frames(java_lock.stop_actor(actor), phase, not conflict, account)
        save(out / f"{stage}-java.json", phase)
        return phase
    finally:
        relay.close()
        support.stop(process)


def finish(out, observation, rust, java, sql, env, account):
    held, released = rust
    conflict, admitted = java
    need(held == released == observation["identity"] and observation["commit_recovery_state"] == [1, 1, 1]
         and len({held["xid"], conflict["xid"], admitted["xid"]}) == 3,
         "Rust commit terminal or the three transaction identities differ")
    need(observation["rejected_multi_byte"] == account.binary,
         "Rust did not observe the expected multi-byte lock-key check")
    need(observation["rejected_lossy_tinyint"] == (account.key_shape == "tinyint-boolean"),
         "Rust did not observe the expected lossy TINYINT lock-key check")
    frames = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text("utf-8").splitlines()]
    need(frames[-1]["succeeded"] and frames[-1]["faults"] == 0, "Rust RM passthrough incomplete")
    request, = [row for row in frames if row.get("kind") == 11 and row["direction"] == "c2s"]
    reply, = [row for row in frames if row.get("kind") == 12 and row["direction"] == "s2c"]
    fields = support.branch_identity(request, False)
    need((request["connection"], request["request_id"]) == (reply["connection"], reply["request_id"])
         and (fields["xid"], fields["resource"], fields["lock"], fields["branch_type"])
         == (held["xid"], held["resource"], account.lock_key, 0)
         and support.payload(reply)[2:] == b"\x01\x00" + struct.pack(">q", held["branch_id"]),
         "Original Rust branch registration differs")
    report, = [row for row in frames if row.get("kind") == 13 and row["direction"] == "c2s"]
    ack, = [row for row in frames if row.get("kind") == 14 and row["direction"] == "s2c"]
    data = support.payload(report)
    xid, at = support.string(data, 2)
    need((report["connection"], report["request_id"]) == (ack["connection"], ack["request_id"])
         and xid == held["xid"] and data[at:at+9] == struct.pack(">qB", held["branch_id"], 2)
         and support.string(data, at + 9)[0] == held["resource"]
         and support.payload(ack)[2:] == b"\x01\x00", "Rust PhaseOneDone differs")
    callback, = [row for row in frames if row.get("kind") == 3 and row["direction"] == "s2c"]
    returned, = [row for row in frames if row.get("kind") == 4 and row["direction"] == "c2s"]
    target = support.branch_identity(callback, True)
    raw = support.payload(returned)
    xid, at = support.string(raw, 4)
    need((callback["connection"], callback["request_id"]) == (returned["connection"], returned["request_id"])
         and (target["xid"], target["branch_id"], target["resource"])
         == (held["xid"], held["branch_id"], held["resource"])
         and raw[2:4] == b"\x01\x00" and xid == held["xid"]
         and raw[at:] == struct.pack(">qB", held["branch_id"], 5), "Rust BranchCommit differs")
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text("utf-8").splitlines()]
    need(tm[-1]["succeeded"] and tm[-1]["faults"] == 0, "Rust TM passthrough incomplete")
    commit, = [row for row in tm if row.get("kind") == 7 and row["direction"] == "c2s"]
    committed, = [row for row in tm if row.get("kind") == 8 and row["direction"] == "s2c"]
    need((commit["connection"], commit["request_id"]) == (committed["connection"], committed["request_id"])
         and support.string(support.payload(commit), 2)[0] == held["xid"]
         and support.payload(committed)[2:] in (b"\x01\x00\x08", b"\x01\x00\x09"),
         "Rust GlobalCommit differs")
    balance = reg.command(sql + [account.select_business + ";" + COUNTS], env)
    put(out / "mixed-final.tsv", balance)
    need(balance.decode().splitlines() == [*account.business_rows(890), "0\t0\t0\t0"],
         "Final Rust and Java effects differ")
    return dict(rust_registrations=1, java_registrations=2, java_lock_conflicts=1,
                java_admitted_after_rust_commit=1, lock_key=account.lock_key, end_balance=890,
                rust_rejected_multi_byte_before_registration=account.binary,
                rust_rejected_lossy_tinyint_before_registration=observation["rejected_lossy_tinyint"])
