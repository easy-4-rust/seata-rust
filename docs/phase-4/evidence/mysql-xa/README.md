# Java XA 身份与 MySQL 原生两阶段证据

规格事实源：[XA 实施契约](../../../phase-2/mysql-xa-contract.md)。机器入口：[native ledger](../../mysql-xa-native.json)。本增量实现 Phase 4 的标识/原生数据库基础，完整 XA Resource/RM Gate 和总计划未完成。

## 已执行

- 固定 Java `cba73e180fd8896a99df6415b859d275122feaa7` 的 XAXid、XABranchXid、XAXidBuilder 三源原样编译，实际生成 8 组向量。Rust 1.99/1.85 各 5 项身份测试逐字节核对格式号 9752、UTF-8 和原 `-` 分支前缀；覆盖 null/空/零、负 long、long 两端及超过 JS 安全整数的分支。恢复入口额外拒绝有损 UTF-8/非规范文本，数据库入口另有字节上限及正分支约束。没有宣称完整迁移三个 Java 对象或全部原源测试。
- 两版本各 8 项真实 MySQL 8.4.13 场景通过。Prepare 数据对普通连接不可见，真实 UPDATE 得 1205 证明原行锁保留；显式 Commit/两种 Rollback 结果按独立 SQL 核对；重复 Commit 的 1397 保留 Unknown。格式号/分支字节必须精确，十六进制出站保留引号、换行和中文的原身份。
- 活动作用域 Drop、真实重复键、错误参数均关闭原连接且封闭作用域；取消前从 performance_schema.data_lock_waits 观察同一真实 blocker 的行锁等待，取消后不能继续 DML/Prepare，原服务器事务消失后再核对原数据。非 InnoDB、触发器、双向外键和内部表拒绝路径已运行；未将未提交行不可见冒充回滚。
- 每版本实际截留 PREPARE、COMMIT、ROLLBACK 三个原服务端 OK。原 COM_QUERY 请求、完整原响应和操作/分支逐项关联，三结果均 Unknown，代理退出后的实际精确控制命令计数均为 1。独立会话分别看到 Prepared、已提交行、已回滚行，确认未知不会被恢复缺行改写。
- 原写入测试进程退出后，独立 CLI 仍见两条原 Prepared；真正 stop/start 同一 MySQL 容器后两段原字节保持，业务仍不可见。新的 Rust 进程以不同 PID 恢复原 101/102 分支并分别 Commit/Rollback；同 server_uuid，最终仅原提交行可见，Prepared 全清。每个案例结束后都有独立 CLI 的业务行与 XA RECOVER 快照，自有容器已删除。该重启是正常停止/启动，不冒充断电或完整 XA 故障门禁。
- 全工作区每版本 785 passed / 356 ignored，169 个成功摘要含 rustdoc；忽略项未当作已执行。两组件全功能严格 Clippy 已通过。最终仅增强条件 native 测试的 PFS 观察，生产字节未改变；随后重新执行双版本所有 native 场景、身份目标、最终输入封存、默认/仅 XA 严格检查和格式检查。默认 AT 的正常依赖不包含 seata-xa；新增本地包和两个需要的依赖边，既有三方版本/校验和不变。
- 七类实际生产源码反例 × 双版本共 14 次拒绝：丢 Java 分支前缀、删 Prepare、提交改为回滚、忽略格式号、忽略分支字节、Unknown 错分为 NotSent、跳过约束检查。每次还原原生产字节。独立校验每版本拒绝 12 项弱化证据，核对最终 crates/Cargo 源归档与当前字节一致。

## 固定入口

- java: [java_20261127_195047_fdfecd8b](java_20261127_195047_fdfecd8b/catalog.json)，SHA-256 `1a5ad6ebb0026c2ec18638c45708991dda49ce5ff4e39ecfe28efe1f045f77aa`。
- rust199: [20261127_200051_778d6eee](rust_/20261127_200051_778d6eee/catalog.json)，SHA-256 `2774341fc63aa249712b71ebedfbcb9032e4f55b2adbaf5752b363fae6a2ff12`。
- rust185: [20261127_200052_b78c1a98](rust_/20261127_200052_b78c1a98/catalog.json)，SHA-256 `fef031bce6926bbbdc83f331ccb066780fa54c9f539a1c25f49bd05d86786a66`。
- mysql: [mysql_20261127_200051_2a4729a5](mysql_20261127_200051_2a4729a5/catalog.json)，SHA-256 `b183be3516f8192f1a324559abefdc7637731f21850db550a46022fefc93296d`。
- mutations: [mutations_20261127_194103_d97bda89](mutations_20261127_194103_d97bda89/catalog.json)，SHA-256 `e5507331a342544552307143eab5b2b89fa47f9e21319d052afc423858090dbf`。
- regression199: [regression_1.99_20261127_194653_306db348](regression_1.99_20261127_194653_306db348/catalog.json)，SHA-256 `3e74a1e9a80e7547c74d6a4de6f9e7740d839a0aedb0f8303852b239c15bb81d`。
- regression185: [regression_1.85_20261127_194643_0c30a643](regression_1.85_20261127_194643_0c30a643/catalog.json)，SHA-256 `b16a83e6cfda766ec14868751cc459e7a9d2a5dba0deeb7db9dcf1e5bb7f7798`。
- default: [default_20261127_195244_2204d2f1](default_20261127_195244_2204d2f1/catalog.json)，SHA-256 `8d3062fe2e8efe196319e5459bb307b9a85c24a20f8d7b82f640440a8ccd44ad`。
- validation: [validation_20261127_200233_f805e422](validation_20261127_200233_f805e422/catalog.json)，SHA-256 `0046d8aadc37479a62b9b80f7da693f6863a45f3f3f639175c2fcc12b72e398b`。

各目录保留命令、stdout/stderr、确切输入与哈希。tools/formal-source-junit/xa_java_oracle.py、xa_rust.py、xa_mysql.py、xa_mutations.py、xa_regression.py、xa_default.py 重建各门禁；validate_xa_native.py 接收 ledger 指定目录和固定 catalog SHA，独立检查原 Java、源输入、原帧、数据与重启事实及反例。不能仅凭计划、编译成功或 result.json 宣称完整门禁通过。

## 初始失败及诊断

早期 Rust 前缀原型缺 `-`，两项身份断言失败；最终实际 Java 向量和封存源反例另有绑定。原生日志 mysql_20261127_192415_d9d9a3a9 记录 XA START 走 prepared-statement 协议得到真实 1295，改为安全十六进制 COM_QUERY 后通过。随后真实恢复列发现长度为有符号整数，修正 SQLx 解码并增加负值/上限检查。两个夹具的 START TRANSACTION/CREATE TRIGGER 也因相同 prepared 协议限制改为原文本命令。

较早 mysql_/rust_/regression_/default_ 目录是中间源码或诊断，不能取代上方固定最终入口；部分失败目录没有成功 catalog。十四个源码反例早于最终测试观测补强，但完整源码输入与对应实际失败均保留，生产源恢复哈希仍与最终一致。最后加入代理全程命令计数和实际 PFS 行锁观察，最终双版本现场重新通过。

## 剩余门槛

本轮未执行原 Java TC XA 全局事务，未实现正式 XA BranchResource 的 TC 登记/报告、持久所有权/方向仲裁及权威恢复；缺少 Prepared 或 XAER_NOTA 始终不能当作已有成功。完整 XA SQL/故障/并发/部署/安全门禁、TCC/Fence/空回滚/悬挂、AT WP05–07 及 Phase 5–7 继续。新 native 入口不授予 TC 权限，也不改变 AT 默认有变化 COMMIT 的生产门槛。

最后补齐测试代理的中文接口注释后，再次执行并封存双版本身份目标和完整 native 矩阵；生产逻辑未改变。上方固定入口指向该最终字节。

原 Java TSV 向量的 null/空分量产生行末空字段与制表符，按原字节保留；代码空白检查排除该原始 golden 与运行原日志，不裁剪后冒充原输出。
