"""Match RM native call metrics to original Java TC request/response delivery."""
import copy
import json
from mysql_phase_two_fault import TOOLS, load

OPERATIONS={11:"branch_register",13:"branch_report",21:"lock_query"}
FIELDS=("active","observed","tc_rejected","unknown","not_sent","local_rejected","cancelled","protocol_errors")


def expected(rows,scenario):
    decoder=load("rm_call_metrics_native",TOOLS/"formal-rm-java/validate.py")
    frames=[]
    for row in rows:
        if row.get("event")!="frame":
            continue
        native=dict(row)
        if native.get("action")=="drop_original_report_reply":
            assert scenario=="commit_report_loss"
            assert native.get("direction")=="s2c" and native.get("kind")==14
            assert type(native.get("delivery_count")) is int and native["delivery_count"]==0
            # This profile-specific action drops the original reply without delivery.
            # Reuse the canonical envelope checks with the equivalent zero-copy action.
            native["action"]="drop_and_close"
        decoded=decoder.envelope(native)
        decoded["action"]=row["action"]
        frames.append(decoded)
    metrics={name:dict.fromkeys(FIELDS,0) for name in OPERATIONS.values()}
    counts={}
    for kind,name in OPERATIONS.items():
        requests=[r for r in frames if r["direction"]=="c2s" and r["kind"]==kind]
        responses=[r for r in frames if r["direction"]=="s2c" and r["kind"]==kind+1]
        counts[name]=len(requests)
        for request in requests:
            matched=[r for r in responses if (r["connection"],r["request_id"])==(request["connection"],request["request_id"])]
            assert len(matched)<=1
            if not matched or matched[0]["delivery_count"]==0:
                outcome="unknown"
            else:
                response=matched[0]
                assert response["delivery_count"]==1 and response["body"][2] in (0,1)
                outcome="observed" if response["body"][2]==1 else "tc_rejected"
            metrics[name][outcome]+=1
        assert len(responses)==len(requests), "these profiles require original TC replies, including dropped ones"
    assert counts["branch_register"]==1
    assert counts["branch_report"]==(0 if scenario=="rm_call_rejected" else 1)
    if scenario=="rm_call_rejected":
        assert counts["lock_query"]==1
        assert metrics["branch_register"]["tc_rejected"]==1
        assert metrics["lock_query"]["observed"]==1
    else:
        assert metrics["branch_register"]["observed"]==1
        assert metrics["branch_report"]["unknown" if scenario=="commit_report_loss" else "observed"]==1
    return metrics


def verify(metrics,rows,scenario):
    assert set(metrics)==set(OPERATIONS.values())
    for values in metrics.values():
        assert set(values)==set(FIELDS)
        assert all(type(value) is int and value>=0 for value in values.values())
    assert metrics==expected(rows,scenario), "RM call metrics disagree with native TC response/delivery"


def validate(observations,out,scenario,save):
    metrics=json.loads((observations/"rm-call-metrics.json").read_bytes())
    save(out/"rm-call-metrics.json",metrics)
    rows=[json.loads(line) for line in (out/"RM-frames.jsonl").read_text().splitlines()]
    verify(metrics,rows,scenario)
    operation="branch_register" if scenario=="rm_call_rejected" else "branch_report"
    outcome="tc_rejected" if scenario=="rm_call_rejected" else "unknown" if scenario=="commit_report_loss" else "observed"
    controls=[]
    for name in ("wrong_outcome","double_receipt","identity_label","boolean_counter","wrong_delivery"):
        changed=copy.deepcopy(metrics); native=copy.deepcopy(rows)
        if name=="wrong_outcome":
            changed[operation][outcome]=0
            changed[operation]["unknown" if outcome=="observed" else "observed"]=1
        elif name=="double_receipt":changed["branch_register"]["observed"]+=1
        elif name=="identity_label":changed[operation]["xid"]="unexpected-identity"
        elif name=="boolean_counter":changed[operation][outcome]=True
        else:
            reply=next(r for r in native if r.get("event")=="frame" and r["direction"]=="s2c" and r["kind"]==({"branch_register":12,"branch_report":14}[operation]))
            reply["delivery_count"]=1-int(reply["delivery_count"])
            reply["action"]="forward" if reply["delivery_count"] else "drop_and_close"
        try:verify(changed,native,scenario)
        except (AssertionError,ValueError):controls.append(dict(control=name,rejected=True))
        else:raise AssertionError(f"RM call metric verifier accepted {name}")
    save(out/"rm-call-metrics-controls.json",controls)
