"""Inventory the pinned SQL parser source without promoting a selected Rust profile to complete migration."""
import hashlib
from pathlib import Path

JAVA = Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')


def inventory(root):
    core = 'sqlparser/seata-sqlparser-core/src/main/java/org/apache/seata/sqlparser/'
    mysql = 'sqlparser/seata-sqlparser-druid/src/main/java/org/apache/seata/sqlparser/druid/mysql/'
    objects = []
    for source, target in [
        (core + 'SQLType.java', 'sql_type.rs'), (core + 'SQLRecognizer.java', 'sql_recognizer.rs'),
        (core + 'SQLRecognizerFactory.java', 'sql_recognizer_factory.rs'),
        (core + 'SQLParsingException.java', 'exception/sql_error.rs'),
        (mysql + 'BaseMySQLRecognizer.java', 'mysql/base_mysql_recognizer.rs'),
        (mysql + 'MySQLInsertRecognizer.java', 'mysql/mysql_insert_recognizer.rs'),
        (mysql + 'MySQLUpdateRecognizer.java', 'mysql/mysql_update_recognizer.rs'),
        (mysql + 'MySQLDeleteRecognizer.java', 'mysql/mysql_delete_recognizer.rs')]:
        path = JAVA / source
        objects.append(dict(source=source, source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
            target='crates/seata_at/src/' + target, state='PARTIAL',
            reason='Selected MySQL AT recognition profile only; complete Java API, other SQL and source-test differential remain unverified'))
    tests, assets, inputs = [], [], {JAVA / row['source'] for row in objects}
    for module in sorted((JAVA / 'sqlparser').iterdir()):
        test_root = module / 'src/test'
        if not test_root.is_dir():
            continue
        for path in sorted(test_root.rglob('*')):
            if not path.is_file():
                continue
            relative = path.relative_to(JAVA).as_posix()
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            inputs.add(path)
            if path.suffix == '.java':
                tests.append(dict(source=relative, sha256=digest, state='MISSING',
                    concrete_source_cases_expanded=False, differential='NOT_RUN'))
            else:
                target = root / 'tests/fixtures/at_sql/java_source_assets' / path.relative_to(JAVA / 'sqlparser')
                target.parent.mkdir(parents=True, exist_ok=True)
                if target.exists():
                    if target.read_bytes() != path.read_bytes():
                        raise ValueError('Existing Java test asset differs: ' + str(target))
                else:
                    target.write_bytes(path.read_bytes())
                inputs.add(target)
                assets.append(dict(source=relative, target=target.relative_to(root).as_posix(), sha256=digest, copied_byte_identical=True))
    return dict(status='module_migration_incomplete', java_pin='cba73e180fd8896a99df6415b859d275122feaa7',
        scope='All source files/assets under sqlparser/*/src/test; file inventory only, not expanded concrete-case parity',
        objects=objects, source_test_files=tests, source_test_assets=assets,
        SOURCE_PARITY_complete=False, Java_execution=False, golden_or_live_diff=False,
        Rust_obligation_tests='crates/seata_at/tests/mysql_sql_recognition.rs'), inputs
