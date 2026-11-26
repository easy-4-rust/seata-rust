# seata-datasource

WP05公共元数据与精确行值基础：保留物理列顺序和独立主键顺序，验证列名、非空PK与DECIMAL维度；不包含数据库行读取或Undo。

[契约](../../docs/phase-2/mysql-foundation-contract.md)和[实际验证](../../docs/phase-2/evidence/mysql-foundation/README.md)记录范围及剩余工作。对应Java对象为部分实现，不宣称完整迁移。

精确Field/Row/TableRecords内存模型已加入；[行值证据](../../docs/phase-2/evidence/row-image/README.md)覆盖精确整数/DECIMAL、NULL/二进制、完整schema及原子预算。尚未接数据库行读取，完整物理列限制、Java序列化互通与Undo仍待实施。
