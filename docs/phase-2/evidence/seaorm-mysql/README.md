# SeaORM 实体 DML 与正式 MySQL AT

对应[适配契约](../../seaorm-mysql-contract.md)和原架构的独立 ORM 层要求。新增正式 `seata-orm-seaorm` crate，核心 MySQL 不依赖 SeaORM；适配器关闭 ORM 独立 SQLx 驱动与 SQL 打印，在调用方原本地事务中执行真实实体生成的 SQL。

权威现场：[20261128_151334_5cd54054](20261128_151334_5cd54054/result.json)，最终 catalog SHA-256：`2af7b1d65a7f15cdac45e766c00a6293508d160e81a2c3f49c053f7400d0b4c8`。固定未修改原 Java TC、真实 MySQL，Rust 1.99/1.85 各执行三类实体 DML 的提交/补偿，共十二笔实际全局事务。Entity insert/update_many/delete_many 输入并非手写 SQL 替身；支持 SQL、原绑定和参数顺序由实体代码实际生成。

独立观察核对完整业务行、真实资源/原登记/锁、同事务 Undo/SHA-256/见证/配置、一次阶段一报告、原回调和终态。覆盖有符号整数、NULL、带问号文本、`9007199254740993.1250` 精确 DECIMAL、任意二进制和包含大整数/中文的 JSON。原 Java parser 重读并重编码全部十二份 Undo。独立 Python 观察器用 `decimal.Decimal` 读取数值，验证全部前后镜像字段和四位 scale，避免自身先转 float 掩盖精度错误。

两版各通过七个组件测试（六项类型/安全输入加实体分类）、一个 native 忽略项，以及严格 all-target Clippy；全工作区各通过 840 个测试、414 个忽略。忽略项未计为通过。时间映射和亚微秒拒绝在组件测试中核验，本次原生字段矩阵未包含全部时间类型或完整 SeaORM 读写；原 `ConnectionTrait`、模型/生成 ID、关联、迁移和 2.x 仍开放。默认 COMMIT、WP05/WP07 和 Phase 5–7 的其余要求保持原门禁。

依赖锁文件使用 SeaORM 1.1.19（原清单 MSRV 1.81），实际宏版本为兼容解析到的 1.1.20，锁定包和完整构建结果均封存。依赖树确认正常 MySQL 核心无 SeaORM，SeaORM 本身未启用 SQLx 驱动。

更新 RustSec DB `c5279d0b0c9e8b70df666eb6d1edbabcfd1176bd`（1571 公告）的完整 lock 审计退出 **1**：仍报告 [rsa](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)、[time](https://rustsec.org/advisories/RUSTSEC-2026-0009.html)、[libsqlite3-sys](https://rustsec.org/advisories/RUSTSEC-2026-0459.html)。当前 Windows 全工作区 all-features 正常依赖树未启用 time/SQLite，rsa 仍在实际链中；锁项与告警没有删除或加入 ignore，此结果不是生产安全验收通过。

首轮 [20261128_151044_911c7a3d](20261128_151044_911c7a3d/result.json) 的 mysql CLI 观察者未设置字符集，把中文显示为 `??`；Rust 的 UTF-8 行和 Undo 保留原文。独立比较拒绝该不一致，现场停止自有进程并保留原阶段一记录，不计完成。修订轮显式 utf8mb4，十二笔全部通过。

```powershell
python -B tools/formal-source-junit/native_seaorm_mysql.py
python -B tools/formal-source-junit/validate_seaorm_mysql.py docs/phase-2/evidence/seaorm-mysql/20261128_151334_5cd54054 --full
```

原始日志、运行输入与执行二进制、全部 Undo/Java 重编码、独立 SQL、原协议帧、组件/工作区回归及原始审计结果完整封存。自有回环 TC 关闭认证，MySQL 使用夹具 root，不作为生产权限或认证证明。

后续 [原业务回执增量](../mysql-statement-result/README.md)已在正式事务及 SeaORM 输入适配器中贯通原始计数/ID，新增双版本底层大整数回执与十二笔原 TC 事务证据。权威现场 `20261128_163855_5075e62d` 使用当前 RSA 替换后的依赖图与审计，两项剩余告警继续保留；上述早期三告警结论只描述当时快照。全工作区现各 840 passed / 416 ignored，严格 Clippy 通过。自增表准入、生成主键镜像和完整 ORM 仍未交付。
