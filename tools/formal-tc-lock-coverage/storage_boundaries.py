"""Verify original TC DB-lock length and collation observations."""

import json

from live import need, put, reg, support, validator

CASES = ("pk-36", "pk-37", "table-32", "table-33", "row-128", "row-129",
         "collation-owner", "collation-contender")
OVERFLOW_COLUMNS = {"pk-37": "pk", "table-33": "table_name", "row-129": "row_key"}


def verify_schema(raw):
    settings, *columns = raw.decode("utf-8").splitlines()
    global_mode, session_mode, collation = settings.split("\t")
    need("STRICT_TRANS_TABLES" in global_mode.split(",") and "STRICT_TRANS_TABLES" in session_mode.split(",")
         and collation == "utf8mb4_0900_ai_ci", "lock server profile differs")
    need(columns == [f"{name}\t{size}\t{collation}" for name, size in
                     (("pk", 36), ("row_key", 128), ("table_name", 32))], "original lock column profile differs")


def snapshot(out, stage, case, observations, sql, env):
    query = ("SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table "
             "ORDER BY branch_id")
    raw = reg.command(sql+[query], env)
    put(out/f"{stage}-database.tsv", raw)
    need(case["case"] == stage, "checkpoint identity differs")
    expected = [[item["xid"], str(item["branch_id"]), item["resource"], item["table"], item["pk"],
                 "^^^".join((item["resource"], item["table"], item["pk"])), "0"]
                for item in observations if item["branch_id"] is not None]
    actual = [row.split("\t") for row in raw.decode("utf-8").splitlines()]
    need(actual == expected, f"independent lock rows differ at {stage}")


def verify_terminal_rows(out, cases, sql, env):
    raw = reg.command(sql+[
        "SELECT u.xid,u.branch_id,u.log_status,b.phase,b.terminal,b.first_phase_committed "
        "FROM undo_log u JOIN seata_rs_at_branch_v1 b "
        "ON CAST(u.xid AS BINARY)=b.xid AND u.branch_id=b.branch_id ORDER BY u.branch_id; "
        "SELECT (SELECT COUNT(*) FROM undo_log),(SELECT COUNT(*) FROM seata_rs_at_branch_v1)"], env)
    put(out/"final-storage-terminal-rows.tsv", raw)
    expected = [[row["xid"], str(row["branch_id"]), "1", "2", "1", "0"]
                for row in cases if row["branch_id"] is not None]
    actual = [line.split("\t") for line in raw.decode("utf-8").splitlines()]
    need(actual == expected+[[str(len(expected))]*2], "terminal rollback guards differ")


def events(out, role):
    rows = [json.loads(line) for line in (out/f"{role}-frames.jsonl").read_text("utf-8").splitlines()]
    need(rows[-1]["succeeded"] and rows[-1]["faults"] == 0, "observer incomplete")
    frames = [row for row in rows if row["event"] == "frame"]
    for row in frames:
        validator.wire.envelope(row)
        need(row["delivery_count"] == 1 and row["action"] == "forward", "observer altered frame")
    return frames


def pairs(frames, direction, kind, request_type):
    requests = [row for row in frames if row["direction"] == direction and row["kind"] == kind
                and row["request_type"] == request_type]
    replies = [row for row in frames if row["direction"] != direction and row["kind"] == kind+1]
    need(len(replies) == len(requests), "request/reply count differs")
    for req in requests:
        matched = [row for row in replies if (row["connection"], row["request_id"])
                   == (req["connection"], req["request_id"]) and req["index"] < row["index"]]
        need(len(matched) == 1, "reply not uniquely correlated to original")
        yield req, matched[0]


def verify_registrations(rm, cases):
    registrations = list(pairs(rm, "c2s", 11, 0))
    need(len(registrations) == len(cases), "missing original registrations")
    for case, (request, reply) in zip(cases, registrations):
        identity = support.branch_identity(request, False)
        need((identity["xid"], identity["resource"], identity["lock"], identity["branch_type"], identity["data"])
             == (case["xid"], case["resource"], case["lock_key"], 0, ""), "original registration differs")
        reader, result, exception, message = validator.response(dict(body=support.payload(reply)))
        returned_id = reader.number(">q")
        reader.done()
        if case["branch_id"] is None:
            need(result == 0 and returned_id == 0 and exception == case["exception"] and message,
                 "negative original registration differs")
            if case["case"] in OVERFLOW_COLUMNS:
                column = OVERFLOW_COLUMNS[case["case"]]
                need(exception == 18 and f"Data too long for column '{column}' at row 1" in message,
                     f"original overflow did not identify {column}")
        else:
            need(result == 1 and exception == 0 and returned_id == case["branch_id"],
                 "successful original registration differs")
    return registrations


def verify(out, cases, enforce_lock_limits=False):
    rm, tm = events(out, "RM"), events(out, "TM")
    need(tuple(case["case"] for case in cases) == CASES, "case identities differ")
    local_limits = {
        "pk-37": "Java TC MySQL lock_table.pk (36 characters)",
        "table-33": "Java TC MySQL lock_table.table_name (32 characters)",
        "row-129": "Java TC MySQL lock_table.row_key (128 characters)",
    } if enforce_lock_limits else {}
    for case in cases:
        need(case.get("local_limit") == local_limits.get(case["case"]), "local lock limit differs")
        if case["case"] in local_limits:
            need(case["branch_id"] is None and case["exception"] is None,
                 "locally over-limit case reached TC")
    sent_cases = [case for case in cases if case["case"] not in local_limits]
    registrations = verify_registrations(rm, sent_cases)
    pk_36, pk_37, table_32, table_33, row_128, row_129, owner, contender = cases
    length = lambda row: len("^^^".join((row["resource"], row["table"], row["pk"])))
    need((len(pk_36["pk"]), len(pk_37["pk"]), len(table_32["table"]), len(table_33["table"]),
          length(row_128), length(row_129)) == (36, 37, 32, 33, 128, 129), "physical boundary differs")
    need(all(length(x) < 128 for x in cases[:4]) and len(row_129["pk"]) <= 36,
         "physical limits are not independently isolated")
    need((owner["pk"], contender["pk"]) == ("Cafe", "café") and contender["exception"] in (2, 20),
         "original collation conflict differs")
    queries = list(pairs(rm, "c2s", 21, 0))
    need(len(queries) == 1, "expected one original collation lock query")
    query, answer = queries[0]
    queried = support.branch_identity(query, False)
    need((queried["xid"], queried["resource"], queried["lock"], queried["branch_type"], queried["data"])
         == (contender["xid"], contender["resource"], contender["lock_key"], 0, "")
         and support.payload(answer)[2:] == b"\x01\x00\x00\x00"
         and answer["index"] < registrations[-1][0]["index"], "original collation lock query differs")
    need(all(len(list(pairs(tm, "c2s", kind, 0))) == len(cases) for kind in (1, 9)),
         "unexpected global begin or rollback attempt")
    result = verify_lifecycle(rm, tm, cases, len(queries))
    if enforce_lock_limits:
        result.update(branch_registrations=len(registrations),
                      rejected=len(registrations)-result["successful"], locally_rejected=len(local_limits))
    return result


def verify_lifecycle(rm, tm, cases, query_count):
    xids = [row["xid"] for row in cases]
    begins = [support.string(support.payload(rep), 4)[0] for _, rep in pairs(tm, "c2s", 1, 0)
              if support.payload(rep)[2:4] == b"\x01\x00"]
    rollback_pairs = list(pairs(tm, "c2s", 9, 0))
    ends = [support.string(support.payload(req), 2)[0] for req, rep in rollback_pairs
            if support.payload(rep)[2:] == b"\x01\x00\x0b"]
    need(begins == ends == xids and len(set(xids)) == len(xids), "original TM lifecycle differs")
    statuses = {support.string(support.payload(req), 2)[0]: support.payload(rep)[4]
                for req, rep in pairs(tm, "c2s", 15, 0)}
    need(statuses == dict.fromkeys(xids, 15), "not all original transactions finished")
    rollback_started = {support.string(support.payload(req), 2)[0]: req["monotonic_ns"]
                        for req,_ in rollback_pairs}
    accepted = {row["xid"]:row for row in cases if row["branch_id"] is not None}
    callbacks = []
    for req, rep in pairs(rm, "s2c", 5, 0):
        identity = support.branch_identity(req, True)
        payload = support.payload(rep)
        xid, offset = support.string(payload, 4)
        case = accepted.get(xid)
        need(case is not None and (identity["resource"], identity["branch_id"], identity["branch_type"], identity["data"])
             == (case["resource"], case["branch_id"], 0, "")
             and rollback_started[xid] < req["monotonic_ns"], "unexpected original rollback callback identity")
        need(payload[2:4] == b"\x01\x00" and xid == identity["xid"]
             and int.from_bytes(payload[offset:offset+8], "big", signed=True) == identity["branch_id"]
             and payload[offset+8:] == b"\x08", "original rollback callback reply differs")
        callbacks.append((xid, identity["branch_id"]))
    need(set(callbacks) == {(row["xid"], row["branch_id"]) for row in accepted.values()},
         "original rollback did not cover the registered branches")
    return dict(branch_registrations=len(cases), successful=len(accepted), rejected=len(cases)-len(accepted),
                lock_queries=query_count, original_rollback_callbacks=len(callbacks),
                original_rollback_retries=len(callbacks)-len(accepted), global_rollbacks=len(ends))
