"""验证同一原分支的跨行和跨表锁申请在冲突后的实际锁表状态。"""

import json
import struct

import mixed_java_lock as java_lock
from live import need, put, reg, support
from rust_owner_lock import verify_java_frames

STAGES = ("bundle-rejected", "free-subset-admitted", "free-subset-released", "whole-bundle-admitted")
BUSINESS = ["0\t2000", "1\t900", "1\t3000"]
COUNTS = ("SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
          "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM undo_log WHERE log_status=0)")
STATE = ("SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table "
         "ORDER BY table_name,pk;SELECT id,balance FROM accounts ORDER BY id;"
         "SELECT id,balance FROM reserves ORDER BY id;"
         "SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log ORDER BY xid,branch_id")


def lock_row(phase, table, pk):
    resource = phase["resource"]
    return "\t".join([phase["xid"], str(phase["branch_id"]), resource, table, pk,
                      f"{resource}^^^{table}^^^{pk}", "0"])


def guard_row(phase):
    return f"{phase['xid']}\t{phase['branch_id']}\t1\t7B7D"


def observe(owner, phase, out, stage, expected, previous, sql, env):
    work, _, _, actor = owner
    java_lock_row = lock_row(phase, "accounts", "1")
    java_undo = f"{phase['xid']}\t{phase['branch_id']}\t0\t{phase['undo_hex']}"
    raw = reg.command(sql + [STATE], env)
    put(out / f"{stage}-database.tsv", raw)
    if stage == STAGES[0]:
        rows = [java_lock_row, *BUSINESS, java_undo]
        xid = expected["xid"].encode().hex()
        active = reg.command(sql + [
            "SELECT status,(SELECT COUNT(*) FROM branch_table "
            f"WHERE BINARY xid=UNHEX('{xid}')) FROM global_table WHERE BINARY xid=UNHEX('{xid}')"
        ], env)
        put(out / "bundle-rejected-still-active.tsv", active)
        need(active.strip() == b"1\t0", "Rejected original XID was already ended or has a branch")
    elif stage == STAGES[1]:
        rows = [lock_row(expected, "accounts", "0"), java_lock_row, lock_row(expected, "reserves", "1"),
                *BUSINESS, java_undo]
    elif stage == STAGES[2]:
        rows = [java_lock_row, *BUSINESS, *sorted([java_undo, guard_row(previous[1])])]
    else:
        rows = [lock_row(expected, "accounts", "0"), lock_row(expected, "accounts", "1"),
                lock_row(expected, "reserves", "1"), *BUSINESS, guard_row(previous[1])]
    need(raw.decode().splitlines() == rows, f"Independent lock, business or Undo state differs at {stage}")
    if stage == STAGES[2]:
        put(work / "release.ready", "Original free subset released\n")
        java_lock.wait((work / "finished.ready").exists, actor, "Java owner did not commit")
        need((work / "finished.ready").read_text() == phase["xid"], "Java committed a different XID")
        java_lock.wait(lambda: reg.command(sql + [COUNTS], env).strip() == b"0\t0\t0\t0",
                       actor, "Original Java lock or Normal Undo remains")
        after = reg.command(sql + [STATE], env)
        put(out / "java-released.tsv", after)
        need(after.decode().splitlines() == [*BUSINESS, guard_row(previous[1])], "Java cleanup differs")


def correlated(request, responses):
    pair, = [row for row in responses if (row["connection"], row["request_id"])
             == (request["connection"], request["request_id"])]
    return support.payload(pair)


def finish(owner, out, phase, stages, observation, sql, env):
    verify_java_frames(java_lock.stop_actor(owner), phase, True, java_lock.AccountCase("integer"))
    rejected, free, released, entire = stages
    need(free == released and observation["final_branch"] == entire
         and observation["rejected_xid"] == rejected["xid"] and observation["recovery_state"] == [2, 1, 0]
         and len({phase["xid"], rejected["xid"], free["xid"], entire["xid"]}) == 4,
         "Expected four original transaction identities and two Rust successes")
    frames = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text("utf-8").splitlines()]
    need(frames[-1]["succeeded"] and frames[-1]["faults"] == 0, "Rust RM passthrough incomplete")

    def select(kind, direction):
        return [row for row in frames if row.get("kind") == kind and row["direction"] == direction]

    requests, responses = select(11, "c2s"), select(12, "s2c")
    need(len(requests) == len(responses) == 3, "Unexpected Rust registration count")
    for request, expected in zip(requests, (rejected, free, entire), strict=True):
        actual = support.branch_identity(request, False)
        need((actual["xid"], actual["resource"], actual["lock"], actual["branch_type"])
             == (expected["xid"], phase["resource"], expected["lock_key"], 0), "Rust bundle wire identity differs")
        raw = correlated(request, responses)
        wanted = ((0, rejected["error"], 0) if expected is rejected else (1, 0, expected["branch_id"]))
        need((raw[2], raw[-9], struct.unpack(">q", raw[-8:])[0]) == wanted, "Original bundle response differs")
    queries, decisions = select(21, "c2s"), select(22, "s2c")
    need(len(queries) == len(decisions) == 2, "Unexpected Rust lock query count")
    for query, expected in zip(queries, (free, entire), strict=True):
        actual = support.branch_identity(query, False)
        need((actual["xid"], actual["resource"], actual["lock"]) == (
            expected["xid"], expected["resource"], expected["lock_key"]
        ) and correlated(query, decisions)[2:] == b"\x01\x00\x00\x01", "Available bundle query differs")
    callbacks, replies = select(5, "s2c"), select(6, "c2s")
    accepted = {(case["xid"], case["branch_id"]) for case in (free, entire)}
    completed = set()
    need(len(callbacks) == len(replies), "Rust callback/reply count differs")
    for callback in callbacks:
        target = support.branch_identity(callback, True)
        key = (target["xid"], target["branch_id"])
        raw = correlated(callback, replies)
        xid, at = support.string(raw, 4)
        need(key in accepted and target["resource"] == phase["resource"] and xid == target["xid"]
             and raw[2:4] == b"\x01\x00" and raw[at:] == struct.pack(">qB", target["branch_id"], 8),
             "Unexpected rollback callback or response")
        completed.add(key)
    need(accepted == completed, "A Rust branch did not receive an original rollback callback")
    final = reg.command(sql + [STATE], env)
    put(out / "bundle-final.tsv", final)
    need(final.decode().splitlines() == [*BUSINESS, *sorted([guard_row(free), guard_row(entire)])],
         "Final balance, lock or Undo state differs")
    return dict(rejected_lock_key=rejected["lock_key"], admitted_free_lock_key=free["lock_key"],
                admitted_full_lock_key=entire["lock_key"], rust_successful_branches=2,
                rust_rejected_branches=1, rust_rollback_callbacks=len(callbacks),
                orphan_locks_after_rejection=0, java_owner_balance=900)
