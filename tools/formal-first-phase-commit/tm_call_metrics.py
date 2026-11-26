"""Compare TM call counters with the original Java TC loss profile, then weaken real evidence."""
import copy

FIELDS = ("active", "observed", "tc_rejected", "unknown", "not_sent",
          "local_rejected", "cancelled", "protocol_errors")
OPERATIONS = ("begin", "commit", "rollback", "status")


def verify(metrics, rollback):
    assert set(metrics) == {"original", "observer"}
    for source in metrics.values():
        assert set(source) == set(OPERATIONS)
        for values in source.values():
            assert set(values) == set(FIELDS)
            assert all(type(value) is int and value >= 0 for value in values.values())
    expected = {
        owner: {operation: dict.fromkeys(FIELDS, 0) for operation in OPERATIONS}
        for owner in ("original", "observer")
    }
    expected["original"]["begin"]["observed"] = 1
    expected["original"]["rollback" if rollback else "commit"]["unknown"] = 1
    expected["observer"]["status"]["observed"] = 1
    assert metrics == expected, "TM counters disagree with the single original end/lost-reply trace"


def controls(metrics, rollback):
    verify(metrics, rollback)
    operation = "rollback" if rollback else "commit"
    results = []
    for name in ("unknown_as_observed", "duplicated_end", "wrong_operation", "identity_label", "boolean_counter"):
        changed = copy.deepcopy(metrics)
        if name == "unknown_as_observed":
            changed["original"][operation]["unknown"] = 0
            changed["original"][operation]["observed"] = 1
        elif name == "duplicated_end":
            changed["original"][operation]["unknown"] = 2
        elif name == "wrong_operation":
            changed["original"]["commit" if rollback else "rollback"]["unknown"] = 1
        elif name == "identity_label":
            changed["original"][operation]["xid"] = "unexpected-identity"
        else:
            changed["original"][operation]["unknown"] = True
        try:
            verify(changed, rollback)
        except AssertionError:
            results.append(dict(control=name, rejected=True))
        else:
            raise AssertionError(f"counter verifier accepted weakened evidence: {name}")
    return results
