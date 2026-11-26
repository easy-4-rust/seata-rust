import json, os, subprocess, sys, time, uuid
from pathlib import Path
from functools import partial

root = Path.cwd()
sys.path.insert(0, str(root / "tools/formal-tc-lock-coverage"))
import live_guard as guard
import collation_equivalence

out_root = root / "docs/phase-2/evidence/tc-lock-collation-guard"
work_root = Path("D:/codex-builds/seata-collation-validation-20261120")
mysql = ["wsl.exe", "-d", "Ubuntu-20.04", "--exec", "docker", "-H",
         "unix:///run/seata-validation-20261120/docker.sock", "exec", "-i",
         "-e", "MYSQL_PWD="+guard.PASSWORD, "seata-wsl-collation-20261120",
         "mysql", "-h127.0.0.1", "-P43317", "-uroot",
         "--default-character-set=utf8mb4", "-N", "-B", "--raw"]
env = os.environ | dict(TEMP="D:/codex-builds", TMP="D:/codex-builds", PYTHONDONTWRITEBYTECODE="1",
                         PYTHONUTF8="1", CARGO_HOME=str(root.parent / ".seata-cargo-cache"),
                         CARGO_INCREMENTAL="0")
for key in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "SEATA_CONFIG_NAME", "SEATA_ENV",
            "SEATA_TC_LOCK_STORAGE_BOUNDARIES", "RUSTC", "RUSTDOC"):
    env.pop(key, None)
jar = guard.reg.ORACLE / "server/target/seata-server.jar"
assert guard.reg.command(["git", "-C", guard.reg.ORACLE, "rev-parse", "HEAD"], env).decode().strip() == guard.reg.PIN
assert guard.sha(jar) == guard.reg.JAR_SHA and guard.sha(guard.DRIVER) == guard.DRIVER_SHA

def checkpoint(file, client):
    deadline = time.monotonic() + 50
    while not file.exists():
        assert client.poll() is None and time.monotonic() < deadline, str(file)
        time.sleep(.05)
    return json.loads(file.read_bytes())

def run(version):
    out = out_root / ("java-tc-" + os.environ["SEATA_COLLATION_RUN"] + "-" + version)
    work = work_root / ("java-tc-" + version + "-" + uuid.uuid4().hex[:8])
    observations = work / "observations"
    observations.mkdir(parents=True)
    out.mkdir(exist_ok=False)
    scoped = env | {"CARGO_TARGET_DIR": "D:/cargo-target/seata-rust-load-driver"}
    cargo = "cargo"
    if version == "1.85":
        rust = Path("C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin")
        cargo = rust / "cargo.exe"
        scoped.update(RUSTC=str(rust / "rustc.exe"), RUSTDOC=str(rust / "rustdoc.exe"),
                      CARGO_TARGET_DIR="D:/cargo-target/seata-rust-load-driver-msrv")
    db = "undo_tc_collation_java_" + version.split(".")[1] + "_" + uuid.uuid4().hex[:8]
    guard.reg.command(mysql + ["-e", "CREATE DATABASE " + db], scoped)
    for role, path in (("server","script/server/db/mysql.sql"),("client","script/client/at/db/mysql.sql")):
        blob = guard.reg.command(["git", "-C", guard.reg.ORACLE, "cat-file", "blob",
                                  f"{guard.reg.PIN}:{path}"], scoped)
        guard.put(out / f"{role}-schema.sql", blob)
        guard.put(work / f"{role}-schema.sql", blob)
        if role == "server":
            subprocess.run(mysql + [db], input=blob, env=scoped, check=True, timeout=20)
    tc, rm, tm = (guard.port() for _ in range(3))
    assert len({tc, rm, tm}) == 3
    primary = f"jdbc:mysql://127.0.0.1:43318/{db}"
    guard.put(work / "application.yml", f"""spring:
  main:
    web-application-type: none
logging:
  file:
    path: {work.as_posix()}/logs
seata:
  config:
    type: file
  registry:
    type: file
  server:
    service-port: {tc}
    enable-check-auth: true
    retry-dead-threshold: 1000
  store:
    mode: db
    session:
      mode: db
    lock:
      mode: db
    db:
      datasource: druid
      db-type: mysql
      driver-class-name: com.mysql.cj.jdbc.Driver
      url: {primary}?useSSL=false&allowPublicKeyRetrieval=true
      user: root
      password: {guard.PASSWORD}
      min-conn: 1
      max-conn: 4
""")
    guard.put(out / "application.yml", (work / "application.yml").read_bytes())
    overlay = guard.compile_auth(jar, work, out, scoped, False)
    executable_output = guard.reg.command([cargo, "test", "--offline", "--locked", "-p", "seata-db-mysql",
        "--test", "live_commit_direction_java", "--no-run", "--message-format=json"], scoped, timeout=180)
    artifact, = [row["executable"] for line in executable_output.splitlines() if line.startswith(b"{")
                 and (row:=json.loads(line)).get("executable")
                 and row["target"]["name"] == "live_commit_direction_java"]
    digest = guard.sha(Path(artifact))
    guard.save(out/"inputs.json", dict(java_pin=guard.reg.PIN, java_jar=guard.reg.JAR_SHA,
         connector_j=guard.DRIVER_SHA, fixture="Isolated WSL MySQL with independent Windows loopback relays",
         physical_port=43317, artifact=artifact, artifact_sha256=digest))
    scoped.update(SEATA_MYSQL_PORT="43318", SEATA_TC_LOCK_COLLATION_EQUIVALENCE="1", SEATA_MYSQL_DATABASE=db,
        SEATA_MYSQL_PASSWORD=guard.PASSWORD, SEATA_UNDO_SCHEMA_PATH=str(work/"client-schema.sql"),
        SEATA_UNDO_STORE_OUT=str(observations), SEATA_RM_ENDPOINT=f"127.0.0.1:{rm}", SEATA_TM_ENDPOINT=f"127.0.0.1:{tm}")
    enforce_lock_limits = os.environ.get("SEATA_COLLATION_ENFORCE", "1") == "1"
    scoped.pop("SEATA_TC_ENFORCE_DB_LOCK_COLLATION", None)
    if enforce_lock_limits:
        scoped["SEATA_TC_ENFORCE_DB_LOCK_COLLATION"] = "1"
    sql = mysql + [db, "-e"]
    schema = guard.reg.command(sql+[
        "SELECT @@global.sql_mode,@@session.sql_mode,@@collation_server; "
        "SELECT COLUMN_NAME,CHARACTER_MAXIMUM_LENGTH,COLLATION_NAME FROM information_schema.COLUMNS "
        "WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='lock_table' "
        "AND COLUMN_NAME IN ('row_key','pk','table_name') ORDER BY COLUMN_NAME"
    ], scoped)
    guard.storage_boundaries.verify_schema(schema)
    guard.put(out/"mysql-lock-schema.tsv", schema)
    stop = work / "stop"
    processes = []
    try:
        java = [guard.reg.JAVA, "-Xlog:class+load=info:file=class-load.log", "-Djava.io.tmpdir=D:/codex-builds",
                "-Duser.home="+str(work), "-Dseata.callback.fixture.resource="+primary,
                "-Dspring.config.location=file:"+(work/"application.yml").as_posix(),
                "-Dloader.path="+overlay.as_posix()+","+guard.DRIVER.as_posix(), "-cp", jar,
                "org.springframework.boot.loader.launch.PropertiesLauncher", "-h", "127.0.0.1"]
        processes.append(guard.support.spawn(java, work, scoped, out, "tc"))
        guard.support.wait_owned(processes[-1], tc, scoped)
        for role, listener in (("RM",rm),("TM",tm)):
            processes.append(guard.support.spawn([sys.executable, guard.HERE/"proxy.py", "--listen-port", listener,
                "--upstream-port", tc, "--events", out/f"{role}-frames.jsonl", "--stop-file", stop,
                "--role", role], work, scoped, out, role))
            guard.support.wait_owned(processes[-1], listener, scoped)
        client=guard.support.spawn([artifact, guard.TEST, "--ignored", "--exact", "--test-threads=1"],
                                   root, scoped, out, "client")
        processes.append(client)
        cases=[]
        for stage in collation_equivalence.CASES:
            current=checkpoint(observations/f"{stage}.json", client)
            guard.save(out/f"{stage}.json", current)
            cases.append(current)
            guard.storage_boundaries.snapshot(out, stage, current, cases, sql, scoped)
            guard.put(observations/f"{stage}.ack", "confirmed\n")
        assert client.wait(timeout=45) == 0
        guard.put(stop, "complete\n")
        assert all(process.wait(timeout=12) == 0 for process in processes[1:3])
        observation = json.loads((observations/"tc-lock-guard-result.json").read_bytes())
        assert observation == {"cases":cases}
        final = guard.reg.command(sql+[
            "SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),"
            "(SELECT COUNT(*) FROM lock_table),(SELECT COUNT(*) FROM storage_pad),"
            "(SELECT COUNT(*) FROM storage_match)"
        ], scoped)
        guard.put(out/"final-counts.tsv", final)
        assert final.decode().strip().split("\t") == ["0"] * 5, final
        profile = guard.reg.command(sql+[
            "SELECT c.TABLE_NAME,c.COLLATION_NAME,p.PAD_ATTRIBUTE "
            "FROM information_schema.COLUMNS c JOIN information_schema.COLLATIONS p "
            "ON c.COLLATION_NAME=p.COLLATION_NAME WHERE c.TABLE_SCHEMA=DATABASE() "
            "AND ((c.TABLE_NAME IN ('storage_pad','storage_match') AND c.COLUMN_NAME='id') "
            "OR (c.TABLE_NAME='lock_table' AND c.COLUMN_NAME='row_key')) ORDER BY c.TABLE_NAME; "
            "SELECT (_utf8mb4'key' COLLATE utf8mb4_general_ci = _utf8mb4'key '), "
            "(_utf8mb4'key' COLLATE utf8mb4_0900_ai_ci = _utf8mb4'key '), "
            "(_utf8mb4'Cafe' COLLATE utf8mb4_0900_ai_ci = _utf8mb4'café')"
        ], scoped)
        guard.put(out/"business-and-tc-collations.tsv", profile)
        assert profile.decode().splitlines() == [
            "lock_table\tutf8mb4_0900_ai_ci\tNO PAD", "storage_match\tutf8mb4_0900_ai_ci\tNO PAD",
            "storage_pad\tutf8mb4_general_ci\tPAD SPACE", "1\t0\t1"
        ], profile
        wire = collation_equivalence.verify(out,cases,enforce_lock_limits)
        guard.storage_boundaries.verify_terminal_rows(out,cases,sql,scoped)
        classes = (work/"class-load.log").read_text("utf-8").splitlines()
        for klass in ("org.apache.seata.server.lock.AbstractLockManager",
                      "org.apache.seata.server.coordinator.DefaultCore",
                      "org.apache.seata.server.storage.db.lock.LockStoreDataBaseDAO"):
            line, = [line for line in classes if f" {klass} source: " in line]
            assert jar.as_posix() in line.replace("\\","/"), line
        assert guard.sha(Path(artifact)) == digest and guard.sha(jar) == guard.reg.JAR_SHA
        guard.save(out/"result.json", wire)
        print(version, json.dumps(wire), flush=True)
    finally:
        if not stop.exists():
            guard.put(stop,"cleanup\n")
        for process in reversed(processes):
            guard.support.stop(process)
        assert all(process.poll() is not None for process in processes)
        if (work/"class-load.log").exists():
            guard.put(out/"class-load.log",(work/"class-load.log").read_bytes())
        guard.save(out/"process-cleanup.json", {"processes_stopped":True,"pids":[p.pid for p in processes]})

for version in sys.argv[1:]:
    run(version)
