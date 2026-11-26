"""在原 Java TC 的注册应答送达前终止已确认 007 门闩的独立恢复进程。"""
import json
import socket
import time

import live_guard as guard
from mysql_phase_two_fault import TOOLS, load
from mysql_report_claim_fault import AT_FAULT, CLAIM, verify_claim_rows, verify_claim_wire

READY = "java-report-claim-child-ready.json"
EXITED = "java-report-claim-child-exited"
CHILD_TEST = "java_report_claim_child::recover_report_claim_then_wait_for_rm_registration"


def start_mysql_observation(upstream, out):
    proxy = load("claim_child_mysql_proxy", TOOLS / "formal-commit-direction/proxy.py")
    observer = proxy.UndoRollbackCommitObserver(upstream, "observe")
    guard.save(out / "mysql-fault-binding.json", dict(upstream_port=upstream, rm_port=observer.port,
               mode="observe", terminal_sql=CLAIM))
    return observer


def wait_for_confirmed_claim(observer, child):
    deadline = time.monotonic() + 8
    while True:
        with observer.lock:
            claim_sessions = {event["connection"] for event in observer.events
                              if event["kind"] == "execute" and event["sql"] == CLAIM}
            finished = any(event["connection"] in claim_sessions and event["kind"] == "client_command"
                           and event["opcode"] == 1 for event in observer.events)
        if finished:
            return
        assert child.poll() is None and time.monotonic() < deadline, "Child did not close the confirmed claim session"
        time.sleep(.02)


def terminate_at_registration(artifact, parent, observations, mysql, db, env, out, tc, observer):
    deadline = time.monotonic() + 15
    while True:
        try:
            request = json.loads((observations / READY).read_bytes())
            break
        except (FileNotFoundError, ValueError):
            assert parent.poll() is None and time.monotonic() < deadline, "Parent did not commit the original branch"
            time.sleep(.02)
    assert request["parent_pid"] == parent.pid
    codec = load("claim_child_original_frames", TOOLS / "formal-rm-java/rm_proxy.py")
    rule = codec.CallbackRule("basic", "RM")
    sink = codec.Sink(out / "claim-child-RM-frames.jsonl")
    try:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            listener.settimeout(7)
            child_env = env | dict(SEATA_JAVA_TC_CLAIM_CHILD_RM=f"127.0.0.1:{listener.getsockname()[1]}",
                                   SEATA_JAVA_TC_CLAIM_CHILD_XID=request["xid"],
                                   SEATA_JAVA_TC_CLAIM_CHILD_BRANCH=str(request["branch_id"]))
            child = guard.support.spawn([artifact, CHILD_TEST, "--ignored", "--exact"], guard.ROOT, child_env,
                                        out, "claim-child")
            try:
                downstream, _ = listener.accept()
                with downstream, socket.create_connection(("127.0.0.1", tc), timeout=4) as upstream:
                    downstream.settimeout(5)
                    register = codec.read_frame(downstream)
                    event, _, _ = rule.inspect("c2s", 1, register)
                    assert event["kind"] == 103 and event["request_type"] == 0
                    sink.emit(event)
                    upstream.sendall(register)
                    reply = codec.read_frame(upstream)
                    event, _, _ = rule.inspect("s2c", 1, reply)
                    assert event["kind"] == 104 and event["request_type"] == 1
                    assert guard.support.payload(event)[2] == 1
                    sink.emit(dict(event, action="hold_original_registration_reply", delivery_count=0))
                    wait_for_confirmed_claim(observer, child)
                    state = guard.reg.command(mysql + [db, "-e", AT_FAULT], env, timeout=5)
                    guard.put(out / "mysql-report-claim-before-child-exit.tsv", state)
                    assert child.poll() is None, "Child exited before forced termination"
                    child.kill()
                    assert child.wait(timeout=5) != 0
                    # 再读子进程连接，排除退出前仍有报告留在接收缓冲区。
                    try:
                        assert codec.read_frame(downstream) is None
                    except ConnectionResetError:
                        pass
                    sink.emit(dict(event="child_reaped", child_pid=child.pid, exit_code=child.returncode))
            finally:
                guard.support.stop(child)
        guard.save(out / "report-child-ownership.json", dict(request, child_pid=child.pid, stopped=True))
        guard.put(observations / EXITED, "Child exited\n")
    finally:
        sink.close()


def verify_child_exit(mysql, db, env, out, wire, result):
    assert result["report_child_exited_without_sending"] is True
    quit_time = verify_claim_wire(wire, result, delivered=True)
    frames = [json.loads(line) for line in (out / "claim-child-RM-frames.jsonl").read_text().splitlines()]
    request, response, child = frames
    assert (request["direction"], request["kind"], request["delivery_count"]) == ("c2s", 103, 1)
    assert (response["direction"], response["kind"], response["delivery_count"]) == ("s2c", 104, 0)
    assert response["connection"] == request["connection"] and response["request_id"] == request["request_id"]
    assert response["action"] == "hold_original_registration_reply" and guard.support.payload(response)[2] == 1
    data = guard.support.payload(request)
    offset = 2
    for _ in range(4):
        _, offset = guard.support.string(data, offset)
    resource, end = guard.support.string(data, offset, wide=True)
    assert end == len(data) and resource == result["resource_id"]
    ownership = json.loads((out / "report-child-ownership.json").read_bytes())
    assert ownership["stopped"] and ownership["parent_pid"] != child["child_pid"] == ownership["child_pid"]
    assert (ownership["xid"], ownership["branch_id"]) == (result["xid"], result["branch_id"])
    assert child["event"] == "child_reaped" and child["exit_code"] != 0
    tm = [json.loads(line) for line in (out / "TM-frames.jsonl").read_text().splitlines()]
    rollback, = [event for event in tm if event.get("kind") == 9 and event["direction"] == "c2s"]
    assert max(quit_time, response["monotonic_ns"]) < child["monotonic_ns"] < rollback["monotonic_ns"]
    verify_claim_rows(mysql, db, env, out, result, "mysql-report-claim-before-child-exit.tsv")
    return dict(mysql_report_claim_commits=1, mysql_report_claim_ok_delivered=1,
                original_child_rm_registration_replies=1, delivered_child_rm_registration_replies=0,
                child_terminated_before_report=True, report_child_pid=child["child_pid"],
                durable_report_started=1, phase_one_report_recovery="AlreadyStarted")
