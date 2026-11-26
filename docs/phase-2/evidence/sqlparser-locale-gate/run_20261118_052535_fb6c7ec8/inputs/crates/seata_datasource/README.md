# seata-datasource

WP05公共元数据与精确行值基础：保留物理列顺序和独立主键顺序，验证列名、非空PK与DECIMAL维度；不包含数据库行读取或Undo。

[契约](../../docs/phase-2/mysql-foundation-contract.md)和[实际验证](../../docs/phase-2/evidence/mysql-foundation/README.md)记录范围及剩余工作。对应Java对象为部分实现，不宣称完整迁移。

Field/Row/TableRecords的[内存模型证据](../../docs/phase-2/evidence/row-image/README.md)覆盖精确整数/DECIMAL、NULL/二进制、完整schema及原子预算。数据库读取由配套seata_db_mysql负责，Java序列化互通与Undo仍待实施。

ColumnDomain/CharacterSet补足物理位宽、字符数/编码、定长二进制和unsigned十进制；旧无域构造保留。[最新真实读取证据](../../docs/phase-2/evidence/mysql-row/README.md)仍不表示完整Java迁移或Undo完成。


新增MySqlType原生身份及ColumnMeta可选字段，旧构造保持None；只验证类别/位宽相容，不声称完整DDL校验。[实际类型与原Java格式对照](../../docs/phase-2/evidence/mysql-type-gate/README.md)。
