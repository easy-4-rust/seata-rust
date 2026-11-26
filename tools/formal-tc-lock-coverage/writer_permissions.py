"""为自有锁联试数据库配置隔离的 Java、Rust 和非 AT 账号。"""
from dataclasses import dataclass
import re
import secrets
import subprocess

from live import ROOT, need, put, reg

BINDING = "seata_rs_at_resource_binding"
CONTROL = "seata_rs_at_branch_v1"
ATTEMPT = "seata_rs_at_commit_attempt"
ROUTINE = "seata_rs_at_constraint_flags_v1"


@dataclass
class Roles:
    java: tuple
    rust: tuple
    observer: tuple
    container: str
    schema: str
    definer_metadata: bool

    def command(self, credentials, statement):
        username, password = credentials
        return ["docker", "exec", "-e", f"MYSQL_PWD={password}", self.container,
                "mysql", "--protocol=TCP", "-h127.0.0.1", f"-u{username}",
                "-N", "-B", "--raw", self.schema, "-e", statement]


def provision(admin_sql, env, out, container, schema, *, rust_commits=False, definer_metadata=False):
    need(re.fullmatch(r"undo_[0-9a-f]{12}", schema), "Fixture role grants require an owned database")
    roles = Roles(*[(name, secrets.token_hex(24)) for name in
                    ("seata_java_writer", "seata_rust_writer", "seata_observer")], container, schema, definer_metadata)
    names = ("001_seata_rs_at_branch_v1.sql", "002_seata_rs_at_registration_origin.sql",
             "003_seata_rs_at_commit_decision.sql", "004_seata_rs_at_first_phase_witness.sql")
    if rust_commits:
        names += ("006_seata_rs_at_commit_attempt.sql", "007_seata_rs_at_commit_report_started.sql")
    migrations = "\n".join(
        (ROOT / "crates/seata_db_mysql/migrations" / migration).read_text(encoding="utf-8") for migration in names
    )
    reg.command(admin_sql + [migrations], env)
    if definer_metadata:
        install_metadata_procedure(admin_sql, env, out, schema)
    for role, credentials in (("java", roles.java), ("rust", roles.rust), ("observer", roles.observer)):
        username, password = credentials
        principal = f"'{username}'@'%'"
        reg.command(admin_sql + [f"CREATE USER {principal} IDENTIFIED BY '{password}'"], env)
        tables = {"accounts": "SELECT"}
        if role != "observer":
            tables["accounts"] += ",UPDATE"
            tables["undo_log"] = "SELECT,INSERT,UPDATE,DELETE"
            tables[BINDING] = "SELECT"
        if role == "rust":
            for table in ("accounts", "undo_log", CONTROL):
                tables[table] = tables.get(table, "SELECT,INSERT,UPDATE,DELETE")
                if not definer_metadata:
                    tables[table] += ",TRIGGER"
            if rust_commits:
                tables[ATTEMPT] = "SELECT,INSERT,UPDATE"
            if not definer_metadata:
                reg.command(admin_sql + [f"GRANT PROCESS ON *.* TO {principal}"], env)
        for table, privileges in tables.items():
            reg.command(admin_sql + [f"GRANT {privileges} ON `{schema}`.`{table}` TO {principal}"], env)
        put(out / f"{role}-grants.tsv", reg.command(roles.command(credentials, "SHOW GRANTS"), env))
    verify_denials(roles, env, out)
    reg.command(admin_sql + ["SET GLOBAL log_output='TABLE'; SET GLOBAL general_log=ON"], env)
    return roles


def install_metadata_procedure(admin_sql, env, out, schema):
    principal = "'seata_metadata_definer'@'%'"
    source = ROOT / "crates/seata_db_mysql/migrations/012_seata_rs_at_constraint_flags.sql"
    procedure = source.read_text(encoding="utf-8").replace(
        "\nCREATE PROCEDURE ", f"\nCREATE DEFINER={principal} PROCEDURE ", 1)
    put(out / "metadata-procedure.sql", procedure)
    reg.command(admin_sql + [f"CREATE USER {principal} ACCOUNT LOCK; "
                 f"GRANT REFERENCES ON *.* TO {principal}; "
                 f"GRANT TRIGGER ON `{schema}`.* TO {principal}"], env)
    reg.command(admin_sql + [procedure], env)
    reg.command(admin_sql + [f"GRANT EXECUTE ON PROCEDURE `{schema}`.`{ROUTINE}` TO {principal}"], env)
    put(out / "metadata-definer-grants.tsv", reg.command(admin_sql + [f"SHOW GRANTS FOR {principal}"], env))


def verify_denials(roles, env, out):
    probes = {
        "UPDATE": (f"UPDATE {BINDING} SET tinyint1_is_bit=0 WHERE slot=1", b"1142"),
        "ALTER": (f"ALTER TABLE {BINDING} COMMENT='isolated permission probe'", b"1142"),
        "TEMPORARY": (f"CREATE TEMPORARY TABLE {BINDING}(slot INT)", b"1044"),
        "BUSINESS_UPDATE": ("UPDATE accounts SET balance=0 WHERE id=1", b"1142"),
        "FOREIGN_DICTIONARY": ("SELECT COUNT(*) FROM information_schema.INNODB_FOREIGN", b"1227"),
        "ALTER_PROCEDURE": (f"ALTER PROCEDURE {ROUTINE} COMMENT 'owned fixture denial'", b"1370"),
    }
    for role, credentials, cases in (
        ("java", roles.java, ("UPDATE", "ALTER", "TEMPORARY")),
        ("rust", roles.rust, ("UPDATE", "ALTER", "TEMPORARY")),
        ("observer", roles.observer, ("BUSINESS_UPDATE",)),
    ):
        if role == "rust" and roles.definer_metadata:
            cases += ("FOREIGN_DICTIONARY", "ALTER_PROCEDURE")
        for probe in cases:
            statement, code = probes[probe]
            result = subprocess.run(roles.command(credentials, statement), env=env,
                                    capture_output=True, timeout=5)
            put(out / f"{role}-denies-{probe.lower()}.log", result.stderr)
            need(result.returncode != 0 and b"ERROR " + code in result.stderr,
                 f"MySQL unexpectedly allowed {role} {probe}")


def observe_commit_admission(stage, expected, sql, env, out, roles, resource, account):
    if stage == "commit-metadata-denied":
        need(expected == {"attempt_trigger_visibility": "rejected"}, "Incorrect absence of 006 privileges")
        state = reg.command(sql + ["SELECT (SELECT COUNT(*) FROM global_table),"
                                  "(SELECT COUNT(*) FROM seata_rs_at_commit_attempt),"
                                  "(SELECT COUNT(*) FROM undo_log); " + account.select_business], env)
        put(out / "before-commit-metadata-grant.tsv", state)
        need(state.decode().splitlines() == ["0\t0\t0", *account.business_rows(1000)],
             "The permission probe must leave the original workload untouched")
        principal = f"'{roles.rust[0]}'@'%'"
        if roles.definer_metadata:
            grant = f"GRANT EXECUTE ON PROCEDURE `{roles.schema}`.`{ROUTINE}` TO {principal}"
        else:
            grant = f"GRANT TRIGGER ON `{roles.schema}`.`{ATTEMPT}` TO {principal}"
        reg.command(sql + [grant], env)
        grants = reg.command(roles.command(roles.rust, "SHOW GRANTS"), env)
        put(out / "rust-commit-grants.tsv", grants)
        return
    raw = reg.command(sql + [
        "SELECT xid,branch_id,resource_id,pk FROM lock_table;"
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1;"
        "SELECT COUNT(*) FROM seata_rs_at_commit_attempt;SELECT COUNT(*) FROM undo_log;"
        + account.select_business
    ], env)
    put(out / "rust-before-business-commit.tsv", raw)
    row = f"{expected['xid']}\t{expected['branch_id']}\t{resource}\t{account.pk}"
    need(raw.decode().splitlines() == [row, "0", "0", "0", *account.business_rows(1000)],
         "Original lock must precede visibility of business data, Undo and the witness")


def finish(sql, env, out, roles, resource, *, rust_commits=False, committed_identity=None):
    reg.command(sql + ["SET GLOBAL general_log=OFF"], env)
    # 独立 MySQL 日志把真实业务和绑定读取关联到两端各自的认证账号。
    activity = reg.command(sql + [
        "SELECT SUBSTRING_INDEX(user_host,'[',1) AS account_name,"
        "SUM(REGEXP_LIKE(argument,'^UPDATE [`]?accounts[`]? ')),"
        "SUM(argument LIKE 'SELECT resource_id%seata_rs_at_resource_binding%FOR SHARE') "
        "FROM mysql.general_log WHERE command_type IN ('Query','Execute') "
        "AND SUBSTRING_INDEX(user_host,'[',1) IN ('seata_java_writer','seata_rust_writer') "
        "GROUP BY account_name ORDER BY account_name"
    ], env)
    put(out / "writer-authenticated-statements.tsv", activity)
    rows = [line.split("\t") for line in activity.decode().splitlines()]
    java_updates = "2" if rust_commits else "1"
    need(len(rows) == 2 and rows[0][:2] == ["seata_java_writer", java_updates]
         and rows[1][:2] == ["seata_rust_writer", "2"]
         and int(rows[0][2]) >= (2 if rust_commits else 1) and int(rows[1][2]) >= (1 if rust_commits else 2),
         "Original DML and binding reads were not executed by the intended writer accounts")
    binding = reg.command(sql + [f"SELECT HEX(resource_id),tinyint1_is_bit FROM {BINDING}"], env)
    put(out / "final-admission-binding.tsv", binding)
    need(binding.decode().strip() == f"{resource.encode().hex().upper()}\t1",
         "Provisioned resource or mapping changed")
    for role, credentials in (("java", roles.java), ("rust", roles.rust), ("observer", roles.observer)):
        query = "SELECT CURRENT_USER(),(SELECT balance FROM accounts WHERE id=1)"
        state = reg.command(roles.command(credentials, query), env)
        put(out / f"{role}-account-and-balance.tsv", state)
        expected_balance = 890 if rust_commits else 900
        need(state.decode().strip() == f"{credentials[0]}@%\t{expected_balance}", "Writer grant or final balance differs")
    if rust_commits:
        final = reg.command(sql + [
            "SELECT HEX(a.xid),a.branch_id,HEX(a.resource_id),HEX(a.lock_key),a.attempt_state,a.report_started,"
            "b.first_phase_committed,b.commit_terminal FROM seata_rs_at_commit_attempt a "
            "JOIN seata_rs_at_branch_v1 b USING(xid,branch_id)"
        ], env)
        put(out / "final-restricted-commit-attempt.tsv", final)
        identity = committed_identity
        need(final.decode().strip().split("\t") == [
            identity["xid"].encode().hex().upper(), str(identity["branch_id"]),
            resource.encode().hex().upper(), b"accounts:true".hex().upper(), "1", "1", "1", "1"
        ], "Original branch identity, report claim or commit terminal differs")
    return dict(separate_java_and_rust_users=True, binding_mutation_denials=6,
                observer_business_write_denied=True, rust_trigger_scope=(
                    None if roles.definer_metadata else
                    "business_undo_recovery_attempt_tables" if rust_commits else "business_undo_recovery_tables"),
                commit_attempt_metadata_guard_verified=rust_commits,
                rust_definer_metadata=roles.definer_metadata,
                rust_global_process=not roles.definer_metadata)
