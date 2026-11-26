"""Run the managed rollback fence regression with the pinned owned MySQL."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import uuid

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "formal-tc-lock-coverage"))
from live_guard import IMAGE, PASSWORD, ROOT, need, port, put, reg, save, sha, support
import managed_rollback_wire

TEST = "managed_rollback_fence_cases::managed_rollback_fences_a_stale_writer_after_compensating_normal_undo"
RED = b"late Normal should be rejected by the retained guard: Ok(())"


def run(version, red, suite, fault=None):
    work = Path(tempfile.mkdtemp(prefix="seata_managed_rollback_", dir="D:/codex-builds"))
    family = "mysql-managed-rollback-fault" if fault else "mysql-managed-rollback-fence"
    out = ROOT / "docs/phase-2/evidence" / family / work.name
    out.mkdir(parents=True)
    print(out, flush=True)
    for folder in ("data", "observations"):
        (work / folder).mkdir()
    env = os.environ | dict(TEMP="D:/Temp", TMP="D:/Temp", PYTHONUTF8="1", PYTHONDONTWRITEBYTECODE="1",
        CARGO_HOME=str(ROOT.parent / ".seata-cargo-cache"), CARGO_TARGET_DIR="D:/cargo-target/seata-rust-load-driver",
        CARGO_INCREMENTAL="0")
    cargo = "cargo"
    if version == "1.85":
        rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
        cargo = rust / "cargo.exe"
        env.update(RUSTC=str(rust / "rustc.exe"), RUSTDOC=str(rust / "rustdoc.exe"),
                   CARGO_TARGET_DIR="D:/cargo-target/seata-rust-load-driver-msrv")
    schema = reg.command(["git", "-C", reg.ORACLE, "cat-file", "blob",
                          reg.PIN+":script/client/at/db/mysql.sql"], env)
    put(work/"original-undo.sql", schema)
    put(out/"original-undo.sql", schema)
    build = [cargo, "test", "--offline", "--locked", "-p", "seata-db-mysql", "--test",
             "live_commit_preconditions", "--no-run", "--message-format=json"]
    payload = reg.command(build, env, timeout=180)
    put(out/"build.jsonl", payload)
    rows = [json.loads(line) for line in payload.splitlines() if line.startswith(b"{")]
    exe, = [Path(row["executable"]) for row in rows if row.get("executable")
            and row["target"]["name"] == "live_commit_preconditions"]
    artifact = sha(exe)
    owner = uuid.uuid4().hex
    name = "seata-managed-rollback-" + owner[:12]
    database = "undo_" + owner[:12]
    listen = port()
    env.update(SEATA_MYSQL_PORT=str(listen), SEATA_MYSQL_DATABASE=database, SEATA_MYSQL_PASSWORD=PASSWORD,
               SEATA_UNDO_SCHEMA_PATH=str(work/"original-undo.sql"), SEATA_UNDO_STORE_OUT=str(work/"observations"))
    container = None
    client = None
    observer = None
    observed_fault = None
    try:
        container = reg.command(["docker","create","--pull=never","--name",name,"--label",
            "seata.fixture.owner="+owner,"-p",f"127.0.0.1:{listen}:3306","--mount",
            "type=bind,source="+(work/"data").as_posix()+",target=/var/lib/mysql",
            "-e","MYSQL_ROOT_PASSWORD="+PASSWORD,"-e","MYSQL_ROOT_HOST=%","-e","MYSQL_DATABASE="+database,
            IMAGE], env).decode().strip()
        reg.command(["docker", "start", container], env)
        save(out/"ownership.json", dict(id=container, name=name, label=owner, port=listen,
            executable=str(exe), executable_sha256=artifact))
        sql = ["docker","exec","-e","MYSQL_PWD="+PASSWORD,container,"mysql","--protocol=TCP",
               "-h127.0.0.1","-uroot","-N","-B","--raw",database,"-e"]
        limit = time.monotonic()+90
        while True:
            try:
                put(out/"mysql-runtime.tsv", reg.command(sql+[
                    "SELECT VERSION(),@@GLOBAL.innodb_flush_log_at_trx_commit"], env))
                break
            except RuntimeError:
                need(time.monotonic() < limit, "owned MySQL initialization timed out")
                time.sleep(.25)
        selected = [] if suite else [TEST, "--exact"]
        if fault:
            location = HERE.parent/"formal-commit-direction/proxy.py"
            spec = importlib.util.spec_from_file_location("managed_rollback_observer",location)
            proxy = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(proxy)
            observer = proxy.UndoRollbackCommitObserver(listen, fault, managed_rollback_wire.TERMINAL)
            env.update(SEATA_MYSQL_PORT=str(observer.port), SEATA_MANAGED_ROLLBACK_FAULT=fault,
                       SEATA_MANAGED_ROLLBACK_ADMIN_PORT=str(listen))
        client = support.spawn([exe, *selected, "--ignored", "--test-threads=1"], ROOT, env, out, "client")
        if fault:
            checkpoint = work/"observations/managed-rollback-fault-before-retry.json"
            limit = time.monotonic()+30
            while True:
                try:
                    observed_fault = json.loads(checkpoint.read_bytes())
                    checkpoint_ns = time.perf_counter_ns()
                    break
                except (FileNotFoundError,ValueError):
                    need(client.poll() is None and time.monotonic()<limit, "missing original retry checkpoint")
                    time.sleep(.03)
            raw = reg.command(sql+[
                "SELECT COUNT(*) FROM undo_work; "
                "SELECT log_status FROM undo_log WHERE xid='commit-check:consumed-guard' AND branch_id=42; "
                "SELECT phase,terminal,first_phase_committed FROM seata_rs_at_branch_v1 "
                "WHERE xid='commit-check:consumed-guard' AND branch_id=42"],env)
            put(out/"before-retry-database.tsv",raw)
            committed = fault in ("drop_ok", "delay_ok")
            need(raw.decode().splitlines()==[str(int(not committed)),str(int(committed)),
                 f"2\t{int(committed)}\t1"],"independent retry state differs")
            need(observed_fault["journal"]=="Pending" and observed_fault["first_reply"]=="PhaseTwoRollbackFailedRetryable",
                 "uncertain rollback was prematurely published")
            save(out/"observer-checkpoint.json",dict(qpc_ns=checkpoint_ns, observed=observed_fault))
            put(checkpoint.with_suffix(".ack"),"observed\n")
        exit_code = client.wait(timeout=120)
        if observer:
            wire = observer.stop()
            wire["normal_delete_identity"] = "byte-exact"
            observer = None
            save(out/"mysql-wire.json",wire)
            save(out/"wire-verification.json",managed_rollback_wire.verify(wire,observed_fault,checkpoint_ns))
        stdout = (out/"client-stdout.log").read_bytes()
        need(sha(exe)==artifact, "executed binary changed")
        if red:
            need(exit_code==101 and RED in stdout, "did not reproduce the target stale writer failure")
        else:
            need(exit_code==0 and b"test result: ok." in stdout, "managed callback tests failed")
            receipt = json.loads((work/"observations/managed-rollback-fence.json").read_bytes())
            final_query = sql+[
                "SELECT (SELECT COUNT(*) FROM undo_work); "
                "SELECT xid,branch_id,log_status,context,HEX(rollback_info) FROM undo_log; "
                "SELECT CONVERT(xid USING utf8mb4),branch_id,registration_origin,phase,terminal,"
                "first_phase_committed FROM seata_rs_at_branch_v1"]
            raw = reg.command(final_query, env)
            if not suite:
                need(raw.decode().splitlines() == ["0", "commit-check:consumed-guard\t42\t1\t"
                    "serializer=jackson&compressorType=NONE\t7B7D",
                    "commit-check:consumed-guard\t42\t1\t2\t1\t1"], "independent persisted fence differs")
                reg.command(["docker", "restart", container], env)
                limit = time.monotonic()+90
                while True:
                    try:
                        restarted = reg.command(final_query, env)
                        break
                    except RuntimeError:
                        need(time.monotonic() < limit, "owned MySQL restart timed out")
                        time.sleep(.25)
                need(restarted == raw, "restart changed the durable rollback result")
                put(out/"restarted-database.tsv", restarted)
            put(out/"final-database.tsv", raw)
            save(out/"observed-receipt.json", receipt)
        for path in (work/"observations").iterdir():
            if path.is_file() and path.suffix!=".ack":
                put(out/"observations"/path.name, path.read_bytes())
        save(out/"result.json", dict(rust=version, mysql_image=IMAGE, red=red, suite=suite,
            fault=fault, client_exit=exit_code, restarted_database_verified=not (red or suite),
            actual_rust_business_commit=False, full_TC_gate=False))
    finally:
        try:
            try:
                support.stop(client)
            finally:
                if observer:
                    save(out/"mysql-wire.json",observer.stop())
        finally:
            owned = ["docker","ps","-aq","--no-trunc","--filter","label=seata.fixture.owner="+owner]
            ids = reg.command(owned,env).decode().split()
            need(len(ids)<=1, "multiple owned database candidates")
            for candidate in ids:
                inspection = json.loads(reg.command(["docker","inspect",candidate],env))[0]
                need(inspection["Id"]==candidate and (container is None or candidate==container)
                     and inspection["Name"]=="/"+name and inspection["Config"]["Labels"]["seata.fixture.owner"]==owner,
                     "container ownership differs")
                try:
                    put(out/"mysql.log",reg.command(["docker","logs",candidate],env))
                finally:
                    reg.command(["docker","rm","-f",candidate],env)
            need(not reg.command(owned,env).strip(), "owned container remains")
            save(out/"cleanup.json", dict(owned_container_removed=bool(ids), client_stopped=client is None or client.poll() is not None))
    members = [dict(path=path.relative_to(out).as_posix(),sha256=sha(path)) for path in sorted(out.rglob("*")) if path.is_file()]
    save(out/"catalog.json",dict(members=members))
    print(out, sha(out/"catalog.json"), flush=True)


if __name__ == "__main__":
    cli = argparse.ArgumentParser()
    cli.add_argument("rust", choices=["1.99","1.85"])
    role = cli.add_mutually_exclusive_group()
    role.add_argument("--red", action="store_true")
    role.add_argument("--suite", action="store_true")
    role.add_argument("--fault", choices=["drop_ok","block_commit","delay_ok"])
    args = cli.parse_args()
    run(args.rust,args.red,args.suite,args.fault)
