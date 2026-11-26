"""Verify original-TC callbacks on a replacement RM session after owned shutdown."""
import live_guard as guard
from mysql_phase_two_fault import TOOLS, load


def verify_replacement(rows, result, out, mysql_wire):
    wire = load("graceful_restart_wire", TOOLS / "formal-rm-java/validate.py")
    frames = [wire.envelope(row) for row in rows if row.get("event") == "frame"]

    def select(kind, direction):
        return [row for row in frames if row["kind"] == kind and row["direction"] == direction]

    registrations = select(103, "c2s")
    identified = select(104, "s2c")
    assert len(registrations) == len(identified) == 2
    old, new = registrations
    assert old["connection"] != new["connection"]
    assert old["body"] == new["body"]
    for request, response in zip(registrations, identified, strict=True):
        assert (request["connection"], request["request_id"]) == (response["connection"], response["request_id"])
        assert response["body"][2] == 1
    report, = select(13, "c2s")
    reported, = select(14, "s2c")
    assert report["connection"] == old["connection"] and reported["body"][2:] == b"\x01\x00"
    assert report["request_id"] == reported["request_id"] and report["connection"] == reported["connection"]
    assert reported["monotonic_ns"] < new["monotonic_ns"]
    callback, = select(3, "s2c")
    completed, = select(4, "c2s")
    assert callback["connection"] == completed["connection"] == new["connection"]
    assert callback["request_id"] == completed["request_id"]
    identity = guard.support.branch_identity(callback, True)
    assert (identity["xid"], identity["branch_id"], identity["resource"]) == (
        result["xid"], result["branch_id"], result["resource_id"])
    assert completed["body"][2:4] == b"\x01\x00" and completed["body"][-1] == 5
    assert new["index"] < identified[1]["index"] < callback["index"] < completed["index"]
    assert mysql_wire["mode"] == "observe" and not mysql_wire["claimed"]
    events = mysql_wire["events"]
    assert not any(row["kind"] == "observer_error" for row in events)
    business, = [row for row in events if row["kind"] == "execute" and row.get("sql") ==
                 "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_report')"]
    commits = [row for row in events if row["connection"] == business["connection"]
               and row["kind"] == "commit_reply_forwarded"
               and business["qpc_ns"] < row["qpc_ns"] < report["monotonic_ns"]]
    commit, = commits
    raw = load("graceful_mysql_wire", TOOLS / "formal-undo-rollback-loss/validate.py")
    assert raw.frame(commit, 1)[:3] == b"\x00\x00\x00"
    guard.save(out / "graceful-replacement-wire.json", dict(
        original_connection=old["connection"], replacement_connection=new["connection"],
        resource_registration_requests=2, phase_one_reports=1, callbacks=1, callback_status=5,
        original_business_statements=1, original_business_commits=1))
