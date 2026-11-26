"""独立验证原 TC 并发处理重叠锁键批次时的提交与清理。"""
import json
import struct

from live import need, put, reg, support
from lock_bundle import correlated, guard_row, lock_row

STAGES = ("both-lock-inserts-pending", "one-lock-bundle-won")
SEED = (
    "CREATE TABLE accounts(id INT PRIMARY KEY,balance INT NOT NULL) ENGINE=InnoDB;"
    "INSERT INTO accounts VALUES(1,1000),(2,2000),(3,3000);"
    "CREATE TABLE undo_work(id INT PRIMARY KEY) ENGINE=InnoDB"
)
BALANCES = "SELECT id,balance FROM accounts ORDER BY id"
BUSINESS = ["1\t1000", "2\t2000", "3\t3000"]


def observe(out, stage, expected, first, sql, env):
    if stage == STAGES[0]:
        gate = expected["gate"].encode().hex()
        raw = reg.command(sql + [
            "SELECT m.OWNER_THREAD_ID,CAST(v.VARIABLE_VALUE AS CHAR CHARACTER SET utf8mb4) "
            "FROM performance_schema.metadata_locks AS m JOIN performance_schema.user_variables_by_thread AS v "
            "ON v.THREAD_ID=m.OWNER_THREAD_ID WHERE m.OBJECT_TYPE='USER LEVEL LOCK' "
            f"AND m.OBJECT_NAME=UNHEX('{gate}') AND m.LOCK_STATUS='PENDING' "
            "AND v.VARIABLE_NAME='seata_fixture_private_key' ORDER BY v.VARIABLE_VALUE"
        ], env)
        put(out / "pending-tc-workers.tsv", raw)
        workers = [row.split("\t") for row in raw.decode().splitlines()]
        need(len(workers) == 2 and workers[0][0] != workers[1][0]
             and [worker[1] for worker in workers] == ["1", "2"],
             "Both TC workers must have inserted their private row before reaching the shared key")
        raw = reg.command(sql + ["SELECT COUNT(*) FROM lock_table;" + BALANCES], env)
        put(out / f"{stage}-database.tsv", raw)
        need(raw.decode().splitlines() == ["0", *BUSINESS], "Uncommitted locks or business writes became visible")
        return

    winner, loser = expected["winner"], expected["loser"]
    private_key = expected["private_key"]
    need({winner, loser} == {first["xid_a"], first["xid_b"]} and winner != loser
         and private_key == ("1" if winner == first["xid_a"] else "2")
         and expected["resource"] == first["resource"], "Wrong winner or private key")
    winner_branch = dict(expected, xid=winner)
    xid = loser.encode().hex()
    raw = reg.command(sql + [
        "SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table ORDER BY pk;"
        "SELECT xid,branch_id,resource_id FROM branch_table;"
        f"SELECT status FROM global_table WHERE BINARY xid=UNHEX('{xid}');" + BALANCES
    ], env)
    put(out / f"{stage}-database.tsv", raw)
    need(raw.decode().splitlines() == [
        lock_row(winner_branch, "accounts", private_key),
        lock_row(winner_branch, "accounts", "3"),
        f"{winner}\t{expected['branch_id']}\t{expected['resource']}",
        "1", *BUSINESS,
    ], "Only the winner's complete bundle and branch should be visible before rollback")


def finish(out, stages, observation, sql, env):
    first, winner = stages
    need(winner == observation and winner["lock_conflict_code"] == 2, "Original conflict code or identity differs")
    frames = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text("utf-8").splitlines()]
    need(frames[-1]["succeeded"] and frames[-1]["faults"] == 0, "RM observer did not finish in passthrough mode")

    def select(kind, direction):
        return [row for row in frames if row.get("kind") == kind and row["direction"] == direction]

    requests, responses = select(11, "c2s"), select(12, "s2c")
    need(len(requests) == len(responses) == 2
         and max(map(frames.index, requests)) < min(map(frames.index, responses)),
         "Both original registration requests must precede the first response")
    keys = {first["xid_a"]: "accounts:1,3", first["xid_b"]: "accounts:2,3"}
    for request in requests:
        actual = support.branch_identity(request, False)
        need((actual["resource"], actual["branch_type"], actual["lock"])
             == (first["resource"], 0, keys.pop(actual["xid"])), "Lock bundle request differs")
        raw = correlated(request, responses)
        expected = ((1, 0, winner["branch_id"]) if actual["xid"] == winner["winner"] else (0, 2, 0))
        need((raw[2], raw[-9], struct.unpack(">q", raw[-8:])[0]) == expected, "Registration response differs")

    query, = select(21, "c2s")
    decision, = select(22, "s2c")
    actual = support.branch_identity(query, False)
    losing_key = "accounts:2" if winner["private_key"] == "1" else "accounts:1"
    need((actual["xid"], actual["resource"], actual["lock"]) == (
        winner["loser"], winner["resource"], losing_key
    ) and correlated(query, [decision])[2:] == b"\x01\x00\x00\x01", "Losing private key must remain available")
    callbacks, replies = select(5, "s2c"), select(6, "c2s")
    need(bool(callbacks) and len(callbacks) == len(replies), "Winning branch rollback callback missing")
    for callback in callbacks:
        target = support.branch_identity(callback, True)
        raw = correlated(callback, replies)
        xid, offset = support.string(raw, 4)
        need((target["xid"], target["branch_id"], target["resource"], xid) ==
             (winner["winner"], winner["branch_id"], winner["resource"], winner["winner"])
             and raw[2:4] == b"\x01\x00" and raw[offset:] == struct.pack(">qB", winner["branch_id"], 8),
             "Unexpected original rollback callback or response")

    raw = reg.command(sql + [BALANCES + ";SELECT xid,branch_id,log_status,HEX(rollback_info) FROM undo_log"], env)
    put(out / "race-final.tsv", raw)
    need(raw.decode().splitlines() == [*BUSINESS, guard_row(dict(winner, xid=winner["winner"]))],
         "Original winner should leave exactly one rollback fence and preserve balances")
    return dict(concurrent_workers=2, successful_bundles=1, rejected_bundles=1,
                orphan_locks_after_rejection=0, rust_rollback_callbacks=len(callbacks))
