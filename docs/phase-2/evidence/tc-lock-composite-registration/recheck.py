import importlib.util
import json
import re
import sys
from copy import deepcopy
from pathlib import Path

root = Path.cwd()
tools = root / "tools"
evidence = root / "docs/phase-2/evidence/tc-lock-composite-registration"
sys.path.insert(0, str(tools / "formal-tc-lock-coverage"))
import identity_boundaries as identity

profiles = {
    "first-phase-preparation": ("native", "FIRST_PHASE_PREPARATION"),
    "commit-preconditions": ("preparation", "COMMIT_PRECONDITIONS"),
    "commit-direction": ("preparation", "COMMIT_DIRECTION"),
}
for version in ("1.99", "1.85"):
    log = (evidence / f"{version}-live-preparation.log").read_text()
    line, = [line for line in log.splitlines() if line.startswith("test result:")]
    for family, (role, status) in profiles.items():
        path = tools / f"formal-{family}-gate/qualify.py"
        module_spec = importlib.util.spec_from_file_location(family, path)
        module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(module)
        packet = dict(status=f"SELECTED_{status}_OBSERVATIONS_VERIFIED", verified=True,
                      Rust=version, selected_DB_target="live_first_phase_preparation",
                      selected=None, wire_observation=False, enrollment_fault=None,
                      commit_fault=None, Java_peer=None, actual_test_summary=[line],
                      public_COMMIT_available=False, TC_registration_gate_passed=False,
                      SQL_Undo_commit_atomicity_verified=False, WP05_complete=False)
        module.role(packet, version, role)
        packet["actual_test_summary"] = [re.sub(r"\b11 passed", "10 passed", line)]
        try:
            module.role(packet, version, role)
        except ValueError as error:
            assert str(error) == "Actual selected count differs"
        else:
            raise AssertionError("stale roster count accepted")
        print(version, family, "fresh 11-case role accepted and stale count rejected")
    report = evidence / f"java-tc-final-{version}"
    cases = [json.loads((report / f"{stage}.json").read_bytes()) for stage in identity.CASES]
    provenance = json.loads((report / "identity-physical.json").read_bytes())
    check = lambda states: identity.verify(report, states, provenance, provenance["primary"],
                                            provenance["alias"], mysql_port="43317")
    assert check(cases) == json.loads((report / "result.json").read_bytes())
    for field, value, expected in [
        ("local_rejected", False, "local guard result differs"),
        ("branch_id", 7, "locally rejected key returned a TC result"),
        ("exception", 2, "locally rejected key returned a TC result"),
    ]:
        changed = deepcopy(cases)
        changed[3][field] = value
        try:
            check(changed)
        except ValueError as error:
            assert str(error) == expected
        else:
            raise AssertionError(f"invalid local result accepted: {field}")
    print(version, "pinned Java frame transcript reverified and 3 modified observations rejected")
