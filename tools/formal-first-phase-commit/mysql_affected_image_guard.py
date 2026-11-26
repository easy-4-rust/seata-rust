"""Original TC and actual MySQL zero-row DML with a bounded affected-count wire fault."""
import copy
import hashlib
import json
import time
from pathlib import Path
import live_guard as guard
from mysql_phase_two_fault import TOOLS,load

UPDATE="UPDATE undo_work SET v='missing' WHERE id=99"
DELETE="DELETE FROM undo_work WHERE id=99"
PRIOR="INSERT INTO undo_work(id,v) VALUES(1,'prior-uncommitted')"
TARGETS=(UPDATE,DELETE)

def affected_count_fault(sql,wire):
    if sql not in TARGETS:
        return wire
    assert type(wire) is bytes and len(wire)>=11
    assert int.from_bytes(wire[:3],"little")==len(wire)-4 and wire[3]==1
    assert wire[4:7]==b"\x00\x00\x00","target must be an original zero-affected OK without insert ID"
    return wire[:5]+b"\x01"+wire[6:]

def start_observer(upstream,out):
    proxy=load("affected_image_mysql_proxy",TOOLS/"formal-commit-direction/proxy.py")
    observer=proxy.UndoRollbackCommitObserver(upstream,"observe",dml_reply_transform=affected_count_fault)
    guard.save(out/"mysql-fault-binding.json",dict(upstream_port=upstream,rm_port=observer.port,mode="affected_count_0_to_1",targets=TARGETS))
    return observer

def verify_wire(wire):
    raw=load("affected_image_raw_mysql",TOOLS/"formal-undo-rollback-loss/validate.py")
    assert wire["mode"]=="observe" and wire["claimed"] is False
    events=wire["events"]
    assert not any(e["kind"]=="observer_error" for e in events)
    assert [e["qpc_ns"] for e in events]==sorted(e["qpc_ns"] for e in events)
    assert not any(e["kind"] in ("commit_received","commit_forwarded","commit_reply") for e in events)
    commands=[e for e in events if e["kind"]=="client_command"]
    for command in commands:
        if command.get("wire_hex") is None:
            continue
        packet=bytes.fromhex(command["wire_hex"])
        payload=raw.frame(command,0)
        assert payload[0]==command["opcode"]
        assert hashlib.sha256(packet).hexdigest()==command["wire_sha256"]
        assert not (command["opcode"]==3 and payload[1:].lstrip().upper().startswith(b"COMMIT"))
    prepared={};pending={};executed=[]
    for event in events:
        key=event["connection"]
        if event["kind"]=="prepare":
            assert raw.frame(event,0)==b"\x16"+event["sql"].encode()
            pending[key]=event["sql"]
        elif event["kind"]=="prepare_ok":
            payload=raw.frame(event,1)
            assert payload[0]==0 and event["sql"]==pending.pop(key)
            prepared[key,int.from_bytes(payload[1:5],"little")]=(event["sql"],int.from_bytes(payload[7:9],"little"))
        elif event["kind"]=="execute" and event.get("sql") in TARGETS+(PRIOR,):
            payload=raw.frame(event,0)
            assert payload[0]==23 and len(payload)==10 and payload[5:10]==b"\x00\x01\x00\x00\x00"
            assert prepared[key,int.from_bytes(payload[1:5],"little")]==(event["sql"],0)
            original,=[c for c in commands if c["connection"]==key and c.get("wire_hex")==event["wire_hex"] and c["qpc_ns"]<event["qpc_ns"]]
            assert original["opcode"]==23
            executed.append(event)
    faults=[e for e in events if e["kind"]=="dml_reply_transformed"]
    assert len(faults)==4
    assert [e["sql"] for e in faults]==[UPDATE,DELETE,UPDATE,DELETE]
    assert len({e["connection"] for e in faults})==4
    physical=[]
    for fault,prior in zip(faults,(False,False,True,True),strict=True):
        session=[e for e in events if e["connection"]==fault["connection"]]
        original,=[e for e in session if e["kind"]=="dml_reply" and e["sql"]==fault["sql"]]
        delivered,=[e for e in session if e["kind"]=="dml_reply_forwarded" and e["sql"]==fault["sql"]]
        original_payload=raw.frame(original,1)
        changed_payload=raw.frame(fault,1)
        assert original_payload[:3]==b"\x00\x00\x00"
        assert changed_payload[:3]==b"\x00\x01\x00"
        before=bytes.fromhex(original["wire_hex"]);after=bytes.fromhex(fault["wire_hex"])
        assert before==bytes.fromhex(fault["original_wire_hex"])
        assert before[:5]==after[:5] and before[6:]==after[6:]
        assert delivered["wire_hex"]==fault["wire_hex"]
        assert original["qpc_ns"]<fault["qpc_ns"]<delivered["qpc_ns"]
        business=[e for e in executed if e["connection"]==fault["connection"]]
        assert [e["sql"] for e in business]==([PRIOR] if prior else [])+[fault["sql"]]
        assert business[-1]["qpc_ns"]<original["qpc_ns"]
        begin,=[e for e in session if e["kind"]=="begin_reply_forwarded"]
        assert raw.frame(begin,1)[0]==0 and begin["qpc_ns"]<business[0]["qpc_ns"]
        greeting,=[e for e in session if e["kind"]=="greeting"]
        payload=raw.frame(greeting,0)
        start=payload.index(0,1)+1
        identifier=int.from_bytes(payload[start:start+4],"little")
        assert type(identifier) is int and identifier>0 and identifier==greeting["mysql_connection_id"]
        physical.append(identifier)
    assert len(set(physical))==4 and len(executed)==6
    return physical

def verify_frames(out,result,journal,artifact):
    decoder=load("affected_image_native",TOOLS/"formal-rm-java/validate.py")
    def frames(role):
        rows=[json.loads(s) for s in (out/f"{role}-frames.jsonl").read_text().splitlines()]
        assert rows[-1]["event"]=="summary" and rows[-1]["succeeded"] is True and rows[-1]["faults"]==0
        assert all(r["index"]==i for i,r in enumerate(rows))
        decoded=[decoder.envelope(r) for r in rows if r["event"]=="frame"]
        assert all(r["action"]=="forward" and r["delivery_count"]==1 for r in decoded)
        return [r for r in decoded if r["request_type"] not in (3,4)]
    def pair(req,res,kind):
        assert (req["direction"],req["request_type"],req["kind"])==("c2s",0,kind)
        assert (res["direction"],res["request_type"],res["kind"])==("s2c",1,kind+1)
        assert (req["connection"],req["request_id"])==(res["connection"],res["request_id"])
    rm=frames("RM")
    assert len(rm)==2
    pair(*rm,103)
    assert rm[1]["body"][2]==1 and journal==b"SRMJNL01"
    tm=frames("TM")
    pair(*tm[:2],101)
    assert len(tm)>=26
    cases=result["cases"]
    assert len(cases)==4 and len({c["xid"] for c in cases})==4
    at=2
    for case,kind,prior in zip(cases,("update","delete","update","delete"),(False,False,True,True),strict=True):
        assert case["kind"]==kind and case["prior"] is prior
        assert all(case[n] is True for n in ("image_error","poisoned_execute","poisoned_prepare","global_finished"))
        begin,begun,end,ended=tm[at:at+4];at+=4
        pair(begin,begun,1);pair(end,ended,9)
        assert begun["body"][2:4]==b"\x01\x00"
        assert guard.support.string(begun["body"],4)[0]==case["xid"]
        assert guard.support.string(end["body"],2)[0]==case["xid"]
        assert case["observed_end"] in (4,11)
        assert ended["body"][2:]==bytes((1,0,case["observed_end"]))
        statuses=[]
        while at<len(tm) and tm[at]["kind"]==15:
            query,status=tm[at:at+2];at+=2
            pair(query,status,15)
            assert guard.support.string(query["body"],2)[0]==case["xid"]
            assert status["body"][2:4]==b"\x01\x00" and len(status["body"])==5
            statuses.append(status["body"][-1])
        assert statuses==case["status_queries"] and statuses[-1]==15
        assert all(code in (4,11) for code in statuses[:-1])
    assert at==len(tm)
    compiled,=[row for line in (out/"cargo-build.jsonl").read_text().splitlines() if line.startswith("{") and (row:=json.loads(line)).get("executable") and Path(row["executable"])==artifact]
    assert "experimental_changed_commit" not in compiled["features"]
    assert result["business_rows"]==result["undo_rows"]==0

def controls(wire):
    outcomes=[]
    for name in ("missing_fault","unchanged_delivery","changed_status","forged_original","commit_added","wrong_statement","duplicate_physical","response_before_execute","raw_commit_hidden","raw_execute_missing"):
        changed=copy.deepcopy(wire)
        faults=[e for e in changed["events"] if e["kind"]=="dml_reply_transformed"]
        fault=faults[0]
        if name=="missing_fault":changed["events"].remove(fault)
        elif name=="unchanged_delivery":
            next(e for e in changed["events"] if e["kind"]=="dml_reply_forwarded" and e["connection"]==fault["connection"] and e["sql"]==fault["sql"])["wire_hex"]=fault["original_wire_hex"]
        elif name=="changed_status":
            b=bytearray.fromhex(fault["wire_hex"]);b[7]^=1;fault["wire_hex"]=b.hex()
        elif name=="forged_original":fault["original_wire_hex"]=fault["wire_hex"]
        elif name=="commit_added":changed["events"].append(dict(kind="commit_received",connection=1,qpc_ns=changed["events"][-1]["qpc_ns"]+1))
        elif name=="wrong_statement":next(e for e in changed["events"] if e["kind"]=="execute" and e.get("sql") in TARGETS)["sql"]=PRIOR
        elif name=="duplicate_physical":
            greetings=[e for e in changed["events"] if e["kind"]=="greeting" and e["connection"] in {f["connection"] for f in faults}]
            greetings[1]["mysql_connection_id"]=greetings[0]["mysql_connection_id"]
        elif name=="raw_commit_hidden":
            packet=b"\x07\x00\x00\x00\x03COMMIT"
            changed["events"].append(dict(kind="client_command",connection=fault["connection"],opcode=3,wire_hex=packet.hex(),wire_sha256=hashlib.sha256(packet).hexdigest(),qpc_ns=changed["events"][-1]["qpc_ns"]+1))
        elif name=="raw_execute_missing":
            execute=next(e for e in changed["events"] if e["kind"]=="execute" and e.get("sql") in TARGETS)
            command=next(e for e in changed["events"] if e["kind"]=="client_command" and e["connection"]==execute["connection"] and e.get("wire_hex")==execute["wire_hex"])
            changed["events"].remove(command)
        else:fault["qpc_ns"]=0
        try:verify_wire(changed)
        except (AssertionError,ValueError,KeyError):outcomes.append(dict(control=name,rejected=True))
        else:raise AssertionError("accepted weakened affected-image evidence: "+name)
    return outcomes

def native_controls(out,result,journal,artifact):
    outcomes=[]
    for name in ("wrong_original_xid","preparation_not_poisoned","residual_business","journal_not_empty","not_finished","repeated_xid"):
        changed=copy.deepcopy(result);changed_journal=journal
        if name=="wrong_original_xid":changed["cases"][0]["xid"]="forged-original"
        elif name=="preparation_not_poisoned":changed["cases"][0]["poisoned_prepare"]=False
        elif name=="residual_business":changed["business_rows"]=1
        elif name=="journal_not_empty":changed_journal+=b"unexpected-record"
        elif name=="not_finished":changed["cases"][0]["status_queries"][-1]=4
        else:changed["cases"][1]["xid"]=changed["cases"][0]["xid"]
        try:verify_frames(out,changed,changed_journal,artifact)
        except (AssertionError,ValueError):outcomes.append(dict(control=name,rejected=True))
        else:raise AssertionError("accepted weakened native TC evidence: "+name)
    return outcomes

def verify(mysql,db,env,out,wire,result,journal,artifact):
    physical=verify_wire(wire)
    verify_frames(out,result,journal,artifact)
    guard.save(out/"affected-image-controls.json",controls(wire))
    guard.save(out/"affected-image-native-controls.json",native_controls(out,result,journal,artifact))
    deadline=time.monotonic()+8
    sql=mysql+[db,"-e"]
    query="SELECT COUNT(*) FROM performance_schema.threads WHERE PROCESSLIST_ID IN ("+",".join(map(str,physical))+")"
    while True:
        count=guard.reg.command(sql+[query],env,timeout=5).strip()
        if count==b"0":break
        assert time.monotonic()<deadline,"original physical transaction remains"
        time.sleep(.05)
    guard.put(out/"mysql-physical-session-drained.tsv",count+b"\n")
    final=guard.reg.command(sql+["SELECT "+",".join("(SELECT COUNT(*) FROM "+table+")" for table in ("global_table","branch_table","lock_table","undo_work","undo_log","seata_rs_at_branch_v1","seata_rs_at_commit_attempt"))],env,timeout=5)
    guard.put(out/"mysql-final-counts.tsv",final)
    assert final.strip().split(b"\t")==[b"0"]*7
    return dict(cases=4,wire_faults=4,original_physical_sessions=physical,physical_sessions_drained=True,branch_registrations=0,branch_reports=0,local_commits=0,global_rollbacks=4,default_build=True,final_counts=[0]*7,controls_rejected=16)
