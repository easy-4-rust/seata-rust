"""Independent fixed control counts for one original MySQL COMMIT OK loss and TC retry."""
import copy
import json
from mysql_phase_two_fault import TOOLS,load
OPERATIONS=("enroll","observe","bind_commit","bind_rollback","commit","rollback")
FIELDS=("active","completed","unknown_commit","before_commit_error","cancelled","applied","unresolved")

def expected(rows,scenario):
    assert scenario in ("commit_db_drop_ok","rollback_db_drop_ok")
    decoder=load("mysql_control_native",TOOLS/"formal-rm-java/validate.py")
    rows=[decoder.envelope(r) for r in rows if r.get("event")=="frame"]
    rollback=scenario.startswith("rollback")
    kind=5 if rollback else 3
    callbacks=[r for r in rows if r.get("event")=="frame" and r["direction"]=="s2c" and r["kind"]==kind]
    replies=[r for r in rows if r.get("event")=="frame" and r["direction"]=="c2s" and r["kind"]==kind+1]
    assert len(callbacks)==len(replies)==2
    assert all(r["delivery_count"]==1 and r["action"]=="forward" for r in callbacks+replies)
    previous=0
    for request,reply in zip(callbacks,replies,strict=True):
        assert type(request["index"]) is int and type(reply["index"]) is int
        assert previous < request["index"] < reply["index"]
        previous=request["index"]
        assert (request["connection"],request["request_id"])==(reply["connection"],reply["request_id"])
    values={op:dict.fromkeys(FIELDS,0) for op in OPERATIONS}
    values["enroll"].update(completed=1,unresolved=1)
    values["observe"].update(completed=2,unresolved=2)
    values["bind_rollback" if rollback else "bind_commit"].update(completed=2,unresolved=1,applied=1)
    values["rollback" if rollback else "commit"].update(completed=1,unknown_commit=1,applied=1)
    return values

def verify(metrics,rows,scenario):
    assert set(metrics)==set(OPERATIONS)
    for value in metrics.values():
        assert set(value)==set(FIELDS)
        assert all(type(n) is int and n>=0 for n in value.values())
    assert metrics==expected(rows,scenario),"control metrics differ from original callbacks and COMMIT OK loss"

def validate(observations,out,scenario,save):
    metrics=json.loads((observations/"mysql-control-metrics.json").read_bytes())
    save(out/"mysql-control-metrics.json",metrics)
    rows=[json.loads(s) for s in (out/"RM-frames.jsonl").read_text().splitlines()]
    verify(metrics,rows,scenario)
    op="rollback" if scenario.startswith("rollback") else "commit"
    controls=[]
    for name in ("unknown_as_completed","spurious_before_commit","double_applied","identity_label","boolean_counter","missing_callback","response_before_request","reversed_retry"):
        changed=copy.deepcopy(metrics);native=copy.deepcopy(rows)
        if name=="unknown_as_completed":
            changed[op]["unknown_commit"]=0;changed[op]["completed"]+=1
        elif name=="spurious_before_commit":changed[op]["before_commit_error"]+=1
        elif name=="double_applied":changed[op]["applied"]+=1
        elif name=="identity_label":changed[op]["xid"]="unexpected"
        elif name=="boolean_counter":changed[op]["unknown_commit"]=True
        elif name in ("response_before_request","reversed_retry"):
            kind=5 if op=="rollback" else 3
            callbacks=[r for r in native if r.get("kind")==kind and r.get("direction")=="s2c"]
            replies=[r for r in native if r.get("kind")==kind+1 and r.get("direction")=="c2s"]
            if name=="response_before_request":replies[0]["index"]=callbacks[0]["index"]-1
            else:callbacks[1]["index"]=callbacks[0]["index"]
        else:
            kind=5 if op=="rollback" else 3
            removed=next(r for r in native if r.get("kind")==kind and r.get("direction")=="s2c")
            native.remove(removed)
        try:verify(changed,native,scenario)
        except AssertionError:controls.append(dict(control=name,rejected=True))
        else:raise AssertionError("accepted metric weakening: "+name)
    save(out/"mysql-control-metrics-controls.json",controls)
