"""Pinned Java inventory for the partial data-source/metadata increment, without claiming case parity."""
import hashlib
from pathlib import Path

JAVA = Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')


def inventory(root):
    sql = 'sqlparser/seata-sqlparser-core/src/main/java/org/apache/seata/sqlparser/struct/'
    ds = 'rm-datasource/src/main/java/org/apache/seata/rm/datasource/'
    pairs = [(sql + 'ColumnMeta.java', 'crates/seata_datasource/src/struct/column_meta.rs'),
        (sql + 'TableMeta.java', 'crates/seata_datasource/src/struct/table_meta.rs'),
        (ds + 'exception/TableMetaException.java', 'crates/seata_datasource/src/metadata_error.rs'),
        (ds + 'DataSourceProxy.java', 'crates/seata_db_mysql/src/data_source_proxy.rs'),
        (ds + 'ConnectionProxy.java', 'crates/seata_db_mysql/src/connection_proxy.rs'),
        (ds + 'sql/struct/cache/MysqlTableMetaCache.java', 'crates/seata_db_mysql/src/cache/mysql_table_meta_reader.rs')]
    objects = [dict(source=source, target=target, source_sha256=hashlib.sha256((JAVA / source).read_bytes()).hexdigest(),
        state='PARTIAL', reason='Metadata/local read foundation only; full Java API/source-case differential not implemented') for source, target in pairs]
    inputs = {JAVA / source for source, _ in pairs}
    tests, assets = [], []
    for relative in ['rm-datasource/src/test', 'sqlparser/seata-sqlparser-core/src/test']:
        for path in sorted((JAVA / relative).rglob('*')):
            if not path.is_file():
                continue
            inputs.add(path)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if path.suffix == '.java':
                tests.append(dict(source=path.relative_to(JAVA).as_posix(), sha256=digest, state='MISSING',
                    concrete_cases_expanded=False, differential='NOT_RUN'))
            else:
                target = root / 'tests/fixtures/mysql/java_source_assets' / path.relative_to(JAVA)
                target.parent.mkdir(parents=True, exist_ok=True)
                if target.exists() and target.read_bytes() != path.read_bytes():
                    raise ValueError('Existing copied source asset differs')
                if not target.exists():
                    target.write_bytes(path.read_bytes())
                inputs.add(target)
                assets.append(dict(source=path.relative_to(JAVA).as_posix(), target=target.relative_to(root).as_posix(),
                    sha256=digest, byte_identical=True))
    return dict(status='Java_module_migration_incomplete', java_pin='cba73e180fd8896a99df6415b859d275122feaa7',
        objects=objects, source_test_files=tests, source_test_assets=assets, source_case_parity_complete=False,
        Java_execution=False, full_API_implemented=False,
        Rust_extensions=['ColumnType JDBC value-domain subset', 'MySqlOptions finite pool/TLS configuration', 'DbError redacted SQLSTATE boundary']), inputs
