"""Validate RM gauges and counters against the original two-callback shutdown trace."""
import copy
import json

FIELDS = (
    "journal_pending", "journal_completed", "journal_retry_safe", "journal_rejected",
    "active", "applied", "rejected", "retry_safe", "unknown", "manual_intervention",
    "duplicate_completed", "duplicate_rejected", "errors", "cancelled",
    "apply_attempts", "recover_attempts", "journal_faulted", "journal_closed",
)


def expected(old_callbacks, before_active):
    base = dict.fromkeys(FIELDS, 0)
    base.update(journal_faulted=False, journal_closed=False)
    before = dict(base, journal_pending=1, active=before_active, apply_attempts=1)
    after = dict(base, journal_pending=1, apply_attempts=1, cancelled=old_callbacks)
    final = dict(base, journal_completed=1, apply_attempts=1, recover_attempts=1,
                 cancelled=old_callbacks, applied=1)
    return dict(before_shutdown=before, after_shutdown=after, after_recovery=final)


def verify(metrics, old_callbacks):
    assert set(metrics) == {"before_shutdown", "after_shutdown", "after_recovery"}
    for point in metrics.values():
        assert set(point) == set(FIELDS)
        for key, value in point.items():
            assert type(value) is (bool if key in ("journal_faulted", "journal_closed") else int)
            assert value >= 0
    assert type(old_callbacks) is int and old_callbacks >= 1
    before_active = metrics["before_shutdown"]["active"]
    assert 1 <= before_active <= old_callbacks
    assert metrics == expected(old_callbacks, before_active), "RM metrics disagree with pending/cancelled/recovered original branch"



def native_counts(rows, rollback):
    from mysql_phase_two_fault import TOOLS, load
    decoder = load("rm_metrics_native", TOOLS / "formal-rm-java/validate.py")
    frames = [decoder.envelope(row) for row in rows if row.get("event") == "frame"]
    registrations = [row for row in frames if row["kind"] == 103 and row["direction"] == "c2s"]
    assert len(registrations) == 2
    old, new = [row["connection"] for row in registrations]
    assert old != new
    kind = 5 if rollback else 3
    callbacks = [row for row in frames if row["kind"] == kind and row["direction"] == "s2c"]
    old_requests = [row for row in callbacks if row["connection"] == old]
    new_requests = [row for row in callbacks if row["connection"] == new]
    assert old_requests and len(new_requests) == 1
    assert callbacks == old_requests + new_requests
    assert len({(row["connection"], row["request_id"]) for row in callbacks}) == len(callbacks)
    assert all(row["body"] == new_requests[0]["body"] for row in old_requests)
    assert not any(row["kind"] == (3 if rollback else 5) and row["direction"] == "s2c" for row in frames)
    responses = [row for row in frames if row["kind"] == kind + 1 and row["direction"] == "c2s"]
    assert len(responses) == 1
    assert (responses[0]["connection"], responses[0]["request_id"]) == (new, new_requests[0]["request_id"])
    return len(old_requests)


def validate(observations, out, save, *, rollback=False):
    metrics = json.loads((observations / "rm-branch-metrics-restart.json").read_bytes())
    metrics["after_recovery"] = json.loads((observations / "rm-branch-metrics-final.json").read_bytes())
    # 原始快照即使未通过验证也保留，避免失败时丢失诊断依据。
    save(out / "rm-branch-metrics.json", metrics)
    rows = [json.loads(line) for line in (out / "RM-frames.jsonl").read_text().splitlines()]
    old_callbacks = native_counts(rows, rollback)
    verify(metrics, old_callbacks)
    save(out / "rm-metrics-native-binding.json", dict(
        original_callback_requests=old_callbacks, replacement_callback_requests=1,
        before_shutdown_active=metrics["before_shutdown"]["active"],
        cancelled_after_shutdown=metrics["after_shutdown"]["cancelled"],
    ))
    controls = []
    for name in ("duplicate_as_new_apply", "cancelled_as_completed", "lost_pending",
                 "counter_reset_on_owner_change", "identity_label", "boolean_counter", "wrong_native_count"):
        changed = copy.deepcopy(metrics)
        if name == "duplicate_as_new_apply":
            changed["after_recovery"].update(apply_attempts=2, applied=2)
        elif name == "cancelled_as_completed":
            changed["after_shutdown"].update(journal_pending=0, journal_completed=1)
        elif name == "lost_pending":
            changed["after_shutdown"]["journal_pending"] = 0
        elif name == "counter_reset_on_owner_change":
            changed["after_recovery"].update(cancelled=0, apply_attempts=0)
        elif name == "identity_label":
            changed["after_recovery"]["xid"] = "unexpected-identity"
        elif name == "boolean_counter":
            changed["after_shutdown"]["cancelled"] = True
        try:
            verify(changed, old_callbacks + int(name == "wrong_native_count"))
        except AssertionError:
            controls.append(dict(control=name, rejected=True))
        else:
            raise AssertionError(f"RM metrics verifier accepted weakening: {name}")
    for name in ("changed_old_callback_body", "old_callback_on_new_connection"):
        changed = copy.deepcopy(rows)
        old = [row for row in changed if row.get("event") == "frame" and row["kind"] == 103 and row["direction"] == "c2s"][0]["connection"]
        request = next(row for row in changed if row.get("event") == "frame" and row["direction"] == "s2c" and row["kind"] == (5 if rollback else 3) and row["connection"] == old)
        if name == "changed_old_callback_body":
            frame = bytearray.fromhex(request["frame_hex"])
            frame[-1] ^= 1
            request["frame_hex"] = frame.hex()
        else:
            new = [row for row in changed if row.get("event") == "frame" and row["kind"] == 103 and row["direction"] == "c2s"][1]["connection"]
            request["connection"] = new
        try:
            native_counts(changed, rollback)
        except AssertionError:
            controls.append(dict(control=name, rejected=True))
        else:
            raise AssertionError(f"native metrics binding accepted weakening: {name}")
    save(out / "rm-metrics-controls.json", controls)
