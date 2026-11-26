import json
import sys
from copy import deepcopy
from pathlib import Path

root = Path.cwd()
sys.path.insert(0, str(root / "tools/formal-tc-lock-coverage"))
import storage_boundaries as observer

evidence = root / "docs/phase-2/evidence/tc-lock-db-length-guard"
baseline = json.loads((root/"docs/phase-2/wp05-tc-lock-storage-boundaries.json").read_bytes())
red = evidence / "java-tc-red-1.99"
assert "unexpected registration result for pk-37" in (red/"client-stdout.log").read_text()
frames = [json.loads(line) for line in (red/"RM-frames.jsonl").read_text().splitlines()]
requests = [frame for frame in frames if frame.get("kind") == 11 and frame.get("direction") == "c2s"]
assert len(requests) == 2
overflow = requests[-1]
assert observer.support.branch_identity(overflow, False)["lock"] == "storage_text:"+"🦀"*37
reply, = [frame for frame in frames if frame.get("kind") == 12 and
          frame.get("request_id") == overflow["request_id"] and frame.get("connection") == overflow["connection"]]
reader, result, exception, reason = observer.validator.response(dict(body=observer.support.payload(reply)))
assert result == 0 and exception == 18 and "Data too long for column 'pk'" in reason
assert reader.number(">q") == 0
reader.done()
print("RED trace independently confirmed the pk=37 request reached fixed Java TC and returned FailedStore (18)")

def verify(folder, enabled):
    cases = [json.loads((folder / f"{stage}.json").read_bytes()) for stage in observer.CASES]
    observer.verify_schema((folder/"mysql-lock-schema.tsv").read_bytes())
    return cases, observer.verify(folder, cases, enforce_lock_limits=enabled)

for version in ("1.99", "1.85"):
    folder = evidence / f"java-tc-verified-{version}"
    cases, current = verify(folder, True)
    assert current == json.loads((folder/"result.json").read_bytes())
    previous = baseline["rust"][version]
    legacy_folder = root / previous["evidence"]
    _, legacy = verify(legacy_folder, False)
    assert legacy == previous["network"]
    control = evidence / f"java-tc-legacy-control-{version}"
    _, fresh_legacy = verify(control, False)
    assert fresh_legacy == json.loads((control/"result.json").read_bytes())
    assert fresh_legacy["branch_registrations"] == 8 and fresh_legacy["rejected"] == 4
    try:
        verify(legacy_folder, True)
    except ValueError as error:
        assert str(error) == "local lock limit differs"
    else:
        raise AssertionError("legacy TC rejection counted as local")
    for edit, reason in [
        (("local_limit", None), "local lock limit differs"),
        (("exception", 18), "locally over-limit case reached TC"),
    ]:
        changed = deepcopy(cases)
        changed[1][edit[0]] = edit[1]
        try:
            observer.verify(folder, changed, enforce_lock_limits=True)
        except ValueError as error:
            assert str(error) == reason
        else:
            raise AssertionError("changed local failure passed verification")
    print(version, "rechecked historical, guarded and fresh default TC frames and rejected 3 controls")
