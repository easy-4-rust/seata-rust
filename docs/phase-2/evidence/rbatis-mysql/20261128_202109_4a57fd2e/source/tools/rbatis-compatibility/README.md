# rbatis 发布版兼容基线

这是具有独立 workspace/lock 的发布版探针。它没有 MySQL、TC 或正式 AT 执行器，不计入正式事务组件。该探针不计入正式工作区成员，执行本探针不修改生产 Cargo.lock。

运行 `python -B tools/formal-source-junit/rbatis_compatibility.py`，使用已安装的 Rust 1.85.0/1.99.0、E 盘 Cargo 缓存与构建目录、D 盘临时目录。依赖在先前下载后以 `--offline --locked` 构建；来源核对会只读访问 crates.io，安全审计使用现有本地 RustSec 快照，不安装或升级工具。

九项测试验证真实 rbatis CRUD、py_sql/html_sql/sql 宏分派、原序绑定、u64 回执与精确 DECIMAL/JSON、Any 桥和分页边界。捕获执行器返回的是测试回执和模型，并未执行数据库 SQL。运行器另恢复原宏驱动，验证同一测试中三个调用因原池查询而失败，证明补丁处理的实际缺口。

维护来源、逐文件原始/当前摘要和完整差异分别在五个 `third_party` 目录的 `SEATA_PROVENANCE.json`、`SEATA.patch`。后续 time 维护副本回移 RFC 2822 官方安全修复，原版本扫描发现仍保留。原发布包许可证原样保留；宏驱动发布包未包含 LICENSE，维护副本补充与其声明一致的 Apache 2.0 标准许可证文本。所有补丁均未去除许可证或版权声明。

正式执行器、精确读写和 Java TC 联试按 [接入契约](../../docs/phase-2/rbatis-mysql-contract.md) 继续实施。生产发布需要显式补丁集成；现有安全发现保持开放。
