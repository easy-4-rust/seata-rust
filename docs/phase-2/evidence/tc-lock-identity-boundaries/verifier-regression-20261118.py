import copy
import json
from pathlib import Path
import sys
import tempfile
sys.path.insert(0, "tools/formal-tc-lock-coverage")
import identity_boundaries as identity
import storage_boundaries as storage
import live_guard as runner
from live import support

def read(path):
    return json.loads(path.read_bytes())

def reject(name, action, message):
    try:
        action()
    except ValueError as error:
        assert str(error) == message, (name, str(error))
        return dict(case=name, status="EXPECTED_REJECTION", error=message)
    raise AssertionError(f"{name} did not detect corrupted evidence")

phase = Path("docs/phase-2")
evidence = phase/"evidence/tc-lock-identity-boundaries"
run = evidence/"seata_original_guard_jyhhvzrf"
obs = read(run/"observation.json")
cases = obs["cases"]
physical = obs["physical"]
primary, alias = physical["primary"], physical["alias"]
assert identity.verify(run, cases, physical, primary, alias) == read(run/"result.json")["network"]

collapsed = dict(physical, alias=primary)
controls = [reject("coalesced-published-resource",
    lambda: identity.verify(run,cases,collapsed,primary,primary),
    "same physical instance was not established")]

wrong_tuple = copy.deepcopy(cases)
wrong_tuple[4]["tuple"] = ["a_b","c"]
controls.append(reject("duplicated-physical-composite-tuple",
    lambda: identity.verify(run,wrong_tuple,physical,primary,alias),
    "case input does not match the expected physical identity"))
with tempfile.TemporaryDirectory(prefix="identity-frame-mutation-",dir="D:/Temp") as scratch:
    fake = Path(scratch)
    rm = [json.loads(row) for row in (run/"RM-frames.jsonl").read_text("utf-8").splitlines()]
    changed = [row for row in rm if row.get("kind")==22][1]
    assert support.payload(changed)[-1] == 1
    changed["frame_hex"] = changed["frame_hex"][:-2]+"00"
    (fake/"RM-frames.jsonl").write_text("\n".join(json.dumps(row) for row in rm)+"\n",encoding="utf-8")
    (fake/"TM-frames.jsonl").write_bytes((run/"TM-frames.jsonl").read_bytes())
    controls.append(reject("altered-alias-query-boolean",
        lambda: identity.verify(fake,cases,physical,primary,alias),
        "original lock query did not match registration"))
previous = {}
for version,name in [("1.99","seata_original_guard__ipt5vjs"),("1.85","seata_original_guard_w59z40ii")]:
    stored=phase/"evidence/tc-lock-storage-boundaries"/name
    result=storage.verify(stored,read(stored/"observation.json")["cases"])
    assert result==read(stored/"result.json")["network"]
    previous[version]=result
runner.save(evidence/"verifier-regression-20261118.json",dict(
    source_sha256=runner.sha(Path(__file__)),controls=controls,storage_profile_replay=previous))
runner.put(evidence/"verifier-regression-20261118.py",Path(__file__).read_bytes())
print(json.dumps(controls))