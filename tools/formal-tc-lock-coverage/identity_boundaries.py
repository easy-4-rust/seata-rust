"""Verify original TC resource aliases and local composite-key registration rejection."""

from live import need, support, validator
from storage_boundaries import events, pairs, verify_lifecycle, verify_registrations

CASES = ("alias-owner", "alias-same-id", "alias-other-id", "composite-owner",
         "composite-collision", "composite-distinct")


def verify(out, cases, physical, primary, alias, mysql_port="3306", require_binding=False):
    need(tuple(case["case"] for case in cases) == CASES, "fixture case identities differ")
    need(physical["primary"] == primary and physical["alias"] == alias and primary != alias
         and physical["mysql_port"] == mysql_port and physical["seed"] == "physical-seed"
         and physical["distinct_composite_rows"] == 3, "same physical instance was not established")
    expected = [
        (primary, "undo_work:1", None, None),
        (primary, "undo_work:1", None, False),
        (alias, "undo_work:1", None, None if require_binding else True),
        (primary, "identity_pair:a_b_c", ["a_b", "c"], None),
        (primary, "identity_pair:a_b_c", ["a", "b_c"], None),
        (primary, "identity_pair:a_bd", ["a", "bd"], True),
    ]
    for case, (resource, key, row, lockable) in zip(cases, expected):
        table, pk = key.split(":", 1)
        need((case["resource"], case["lock_key"], case["tuple"], case["lockable"],
              case["table"], case["pk"]) == (resource, key, row, lockable, table, pk),
             "case input does not match the expected physical identity")
        local_rejected = (case["case"] in ("composite-owner", "composite-collision")
                          or require_binding and case["case"] == "alias-other-id")
        need(case["local_rejected"] is local_rejected, "local guard result differs")
        if local_rejected:
            need(case["branch_id"] is None and case["exception"] is None,
                 "locally rejected key returned a TC result")
        elif lockable is False:
            need(case["branch_id"] is None and case["exception"] in (2, 20),
                 "expected explicit original TC conflict")
        else:
            need(case["branch_id"] > 0 and case["exception"] is None,
                 "expected original TC success")
    rm, tm = events(out, "RM"), events(out, "TM")
    registered_resources = []
    for req, rep in pairs(rm, "c2s", 103, 0):
        request = validator.wire.Reader(support.payload(req))
        request.take(2)
        identity = [request.string() for _ in range(4)]
        resource = request.string(True)
        request.done()
        response = validator.wire.Reader(support.payload(rep))
        response.take(2)
        need(identity == ["1.1.0", "phase0-auth", "default_tx_group", "phase0-local-only"]
             and response.number(">B") == 1 and response.string(), "RM resource was not identified")
        response.done()
        registered_resources.append(resource)
    need(set(registered_resources) == {primary, alias}, "original RM resource registrations differ")
    sent_cases = [case for case in cases if not case["local_rejected"]]
    registrations = verify_registrations(rm, sent_cases)
    expected_queries = [(case, registration[0]) for case, registration in zip(sent_cases, registrations)
                        if case["lockable"] is not None]
    queries = list(pairs(rm, "c2s", 21, 0))
    need(len(queries) == len(expected_queries), "original lock queries differ")
    for (query, answer), (case, registration) in zip(queries, expected_queries):
        identity = support.branch_identity(query, False)
        need((identity["xid"], identity["resource"], identity["lock"], identity["branch_type"],
              identity["data"]) == (case["xid"], case["resource"], case["lock_key"], 0, "")
             and support.payload(answer)[2:] == b"\x01\x00\x00"+bytes([case["lockable"]])
             and answer["index"] < registration["index"], "original lock query did not match registration")
    need(all(len(list(pairs(tm, "c2s", kind, 0))) == len(cases) for kind in (1, 9)),
         "unexpected global begin or rollback attempt")
    lifetime = verify_lifecycle(rm, tm, cases, len(queries))
    lifetime.update(branch_registrations=len(registrations), rejected=len(registrations)-lifetime["successful"],
                    locally_rejected=len(cases)-len(sent_cases))
    return lifetime
