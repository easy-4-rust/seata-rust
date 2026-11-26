"""Verify business-key equality across the original Java TC MySQL lock boundary."""

from live import need, put, reg, support
from storage_boundaries import events, pairs, verify_lifecycle, verify_registrations

CASES = ("pad-owner", "pad-contender", "match-owner", "match-contender")


def verify(out, cases, enforce):
    need(tuple(row["case"] for row in cases) == CASES, "case identities differ")
    expected = (
        ("storage_pad", "key", enforce, None, not enforce),
        ("storage_pad", "key ", enforce, None if enforce else True, not enforce),
        ("storage_match", "Cafe", False, None, True),
        ("storage_match", "café", False, False, False),
    )
    for case, (table, pk, local, lockable, accepted) in zip(cases, expected):
        wire_table = case["lock_key"].split(":", 1)[0]
        need(wire_table == table or case["case"] == "match-contender" and wire_table == "StOrAgE_MaTcH",
             "case lock table spelling differs")
        need((case["table"], case["pk"], case["lock_key"], case["locally_rejected"], case["lockable"])
             == (table, pk, f"{wire_table}:{pk}", local, lockable), "case inputs or guard result differ")
        if accepted:
            need(case["branch_id"] > 0 and case["exception"] is None, "expected TC registration success")
        elif local:
            need(case["branch_id"] is None and case["exception"] is None, "local failure has TC outcome")
        else:
            need(case["branch_id"] is None and case["exception"] in (2, 20), "expected TC lock conflict")
    need(len({row["resource"] for row in cases}) == 1, "resources differ")

    rm, tm = events(out, "RM"), events(out, "TM")
    sent = [row for row in cases if not row["locally_rejected"]]
    registrations = verify_registrations(rm, sent)
    queried = [(row, req) for row, (req, _) in zip(sent, registrations) if row["lockable"] is not None]
    queries = list(pairs(rm, "c2s", 21, 0))
    need(len(queries) == len(queried), "unexpected query count")
    for (req, reply), (row, registration) in zip(queries, queried):
        fields = support.branch_identity(req, False)
        need((fields["xid"], fields["resource"], fields["lock"]) ==
             (row["xid"], row["resource"], row["lock_key"]), "query differs")
        need(support.payload(reply)[2:] == b"\x01\x00\x00"+bytes([row["lockable"]])
             and reply["index"] < registration["index"], "query result/order differs")

    need(all(len(list(pairs(tm, "c2s", code, 0))) == len(cases) for code in (1, 9)),
         "unexpected global begin or rollback attempt")
    result = verify_lifecycle(rm, tm, cases, len(queries))
    result.update(branch_registrations=len(sent), rejected=len(sent)-result["successful"],
                  locally_rejected=len(cases)-len(sent))
    return result


def verify_database(out, sql, env):
    remaining = reg.command(sql+[
        "SELECT (SELECT COUNT(*) FROM storage_pad),(SELECT COUNT(*) FROM storage_match)"
    ], env)
    put(out/"final-collation-business-counts.tsv", remaining)
    need(remaining.decode().splitlines() == ["0\t0"], "collation business rows remain")
    raw = reg.command(sql+[
        "SELECT c.TABLE_NAME,c.COLLATION_NAME,p.PAD_ATTRIBUTE "
        "FROM information_schema.COLUMNS c JOIN information_schema.COLLATIONS p "
        "ON c.COLLATION_NAME=p.COLLATION_NAME WHERE c.TABLE_SCHEMA=DATABASE() "
        "AND ((c.TABLE_NAME IN ('storage_pad','storage_match') AND c.COLUMN_NAME='id') "
        "OR (c.TABLE_NAME='lock_table' AND c.COLUMN_NAME='row_key')) ORDER BY c.TABLE_NAME; "
        "SELECT (_utf8mb4'key' COLLATE utf8mb4_general_ci = _utf8mb4'key '), "
        "(_utf8mb4'key' COLLATE utf8mb4_0900_ai_ci = _utf8mb4'key '), "
        "(_utf8mb4'Cafe' COLLATE utf8mb4_0900_ai_ci = _utf8mb4'café')"
    ], env)
    put(out/"business-and-tc-collations.tsv", raw)
    verify_profile(raw)


def verify_profile(raw):
    need(raw.decode("utf-8").splitlines() == [
        "lock_table\tutf8mb4_0900_ai_ci\tNO PAD",
        "storage_match\tutf8mb4_0900_ai_ci\tNO PAD",
        "storage_pad\tutf8mb4_general_ci\tPAD SPACE",
        "1\t0\t1",
    ], "business or TC collation differs")
