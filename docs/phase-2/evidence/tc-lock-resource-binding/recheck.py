import json
import sys
from copy import deepcopy
from pathlib import Path

root = Path.cwd()
sys.path.insert(0, str(root / "tools/formal-tc-lock-coverage"))
import identity_boundaries as observer

def read(path):
    return json.loads(path.read_bytes())

def execute(profile, version, restricted, mutate=None):
    report = root / "docs/phase-2/evidence" / profile / (
        f"java-tc-verified-{version}" if profile == "tc-lock-resource-binding"
        else f"java-tc-final-{version}"
    )
    physical = read(report / "identity-physical.json")
    cases = [read(report / f"{stage}.json") for stage in observer.CASES]
    if mutate is not None:
        cases = deepcopy(cases)
        cases[2][mutate[0]] = mutate[1]
    result = observer.verify(report, cases, physical, physical["primary"], physical["alias"],
                             mysql_port="43317", require_binding=restricted)
    return result, read(report / "result.json")

for version in ("1.99", "1.85"):
    result, saved = execute("tc-lock-resource-binding", version, True)
    assert result == saved and result["branch_registrations"] == 3 and result["locally_rejected"] == 3
    legacy, previous = execute("tc-lock-composite-registration", version, False)
    assert legacy == previous and legacy["branch_registrations"] == 4 and legacy["locally_rejected"] == 2
    try:
        execute("tc-lock-composite-registration", version, True)
    except ValueError as exc:
        assert str(exc) == "case input does not match the expected physical identity"
    else:
        raise AssertionError("previous alias success passed strict verification")
    for edit, expected in [(("local_rejected", False), "local guard result differs"),
                           (("branch_id", 42), "locally rejected key returned a TC result")]:
        try:
            execute("tc-lock-resource-binding", version, True, edit)
        except ValueError as exc:
            assert str(exc) == expected
        else:
            raise AssertionError("altered alias observation passed strict verification")
    print(version, "rechecked same-schema alias baseline, strict trace, and rejected 3 controls")
