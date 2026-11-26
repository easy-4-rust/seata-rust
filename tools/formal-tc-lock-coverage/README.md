# 原TC排他锁覆盖观察

延续现有WP05/06和[核查契约](../../docs/phase-2/tc-lock-coverage-contract.md)，不创建新的规格体系。执行路径与产物只使用D/E盘，不安装工具、不切换分支、不修改原Java server JAR。

```powershell
$env:TEMP='D:/Temp'
$env:TMP='D:/Temp'
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/formal-tc-lock-coverage/source.py
python tools/formal-tc-lock-coverage/live.py
python tools/formal-tc-lock-coverage/verify_controls.py <本次tc-lock-live证据目录>
python tools/formal-tc-lock-coverage/verify_msrv.py <本次tc-lock-live证据目录>
```

`source.py`重新编译固定原AbstractLockManager，并使用录制后端观察15个原方法案例。完整633个classpath输入、285个server fallback文件及实际class来源绑定；它不发送TC网络请求，也不证明实际锁存储。

`live.py`使用正式Rust MySQL数据源从16次真实INSERT镜像生成8组owner/contender锁键，每次保存Java格式日志后回滚。独立MySQL连接确认业务行与Undo为0，然后用正式TM/RM向未修改原TC登记这些原键。两路原始帧观察者绑定注册身份、原连接/signed request ID、XID/BranchId、锁查询、登记结果、回滚及回调；25个同步确认点独立读取自有TC数据库的global/branch/lock真实行。每次Begin/分支登记/全局回滚最多一次，TC拥有二阶段重试。未知结果使本次失败，不借Finished补造历史成功。

原TC数据库模式使用单独封存的MySQL JDBC8.0.33；Java驱动、Rust依赖归档和解压源均在编译前校验并封存。自有MySQL8.4.13固定缓存镜像，D盘绑定持久数据；文本夹具使用utf8mb4_0900_as_cs。原TC清理配置`server.retry-dead-threshold=1000ms`缩短测试等待，metrics仍关闭；默认70秒配置未在本轮作为完整现场矩阵验收。该配置没有改变锁键解析方法。原公共认证SPI仅用于本机测试。

`verify_controls.py`只改实际证据的内存副本，16项负控制要求命中指定拒绝原因，不能把任意异常算作检测成功。`verify_msrv.py`先验证原证据及相同编译输入，使用Rust1.85编译并运行2项资源夹具测试；不声称再次执行网络场景。

**观察符合预期包含缺陷，不意味着锁安全通过。** 空/全空白、多行含空值与分号可漏锁；不同二进制值有共享拆分片段的额外冲突。本工具不转义/hash原值，不接受外部TC风险，不提供同事务业务SQL/Undo持久化、生产二阶段补偿或公开COMMIT。完整原JUnit、WP05/06及后续全部范围继续开放，共享锁暂缓。

[实际结果与边界](../../docs/phase-2/evidence/tc-lock-live/README.md)。

`python -B tools/formal-tc-lock-coverage/live_guard.py 1.99 --java-owner` 运行原 Java AT 持锁、正式 Rust RM 竞争的场景。Rust 最低版本可用 `1.85` 运行同一用例。运行器使用固定 Java TC JAR、原 Java 客户端模块、Connector/J 和自有 MySQL 8.4.13 容器，保留独立 SQL 快照、Java V2 和 Rust V1 报文及客户端依赖摘要。[场景与结果](../../docs/phase-2/tc-lock-coverage-contract.md#原-java-at-持锁时-rust-竞争同一行)

`python -B tools/formal-tc-lock-coverage/live_guard.py 1.99 --rust-owner` 执行反向竞争，构建时启用实验性一阶段提交，使用原 Java JDBC 连接核对原 TC 拒绝登记后的本地回滚。用 `1.85` 可复跑同一场景。[反向竞争验证](../../docs/phase-2/tc-lock-coverage-contract.md#rust-at-持锁时原-java-竞争同一行)

`--java-owner` 和 `--rust-owner` 均可追加 `--composite-key`，核对 BIGINT 与 VARCHAR 复合键的双向竞争，以及原 Java 锁键顺序和文本大小写别名。[复合主键验证](../../docs/phase-2/tc-lock-coverage-contract.md#混合类型复合主键的双向竞争)

两个持锁方向都可追加 `--decimal-key plain` 或 `--decimal-key scientific`，核对 DECIMAL 的完整刻度和 Java 科学计数法锁键。[DECIMAL 主键验证](../../docs/phase-2/tc-lock-coverage-contract.md#decimal-主键的双向竞争)

两个持锁方向也可追加 `--binary-key`，核对单字节 VARBINARY 锁键。Rust 持锁场景同时检查含逗号的多字节列表在登记前被拒绝。[二进制主键验证](../../docs/phase-2/tc-lock-coverage-contract.md#varbinary-主键的排他锁边界)

`--tinyint-key boolean` 或 `--tinyint-key numeric` 在两种持锁方向核对 Connector/J 与 Rust 的同配置锁键。只有 `--java-owner` 支持 `--tinyint-key mismatch`，用真实登记观察两端映射混用时的锁记录。[TINYINT(1) 验证](../../docs/phase-2/tc-lock-coverage-contract.md#tinyint1-的-jdbc-配置与锁键)

`--java-owner --tinyint-key mismatch --bind-tinyint-profile` 验证 011 映射约束。可用 `--guard-location transaction` 或 `--guard-location manager` 在两个独立连接池中分别启用校验。[映射绑定验证](../../docs/phase-2/tc-lock-coverage-contract.md#rust-写入器的-tinyint1-映射绑定)

`--java-owner --multi-row-locks` 验证包含已持有行和空闲行的跨表申请。观察原 TC 冲突回包后是否残留部分锁，再核对持锁期间的空闲子集登记及释放后的完整登记。[跨行跨表登记验证](../../docs/phase-2/tc-lock-coverage-contract.md#跨行跨表登记的冲突原子性)

`python -B tools/formal-tc-lock-coverage/live_guard.py 1.99 --race-lock-bundles` 执行两个重叠锁批次的并发登记。以 `1.85` 复跑同一场景。两个运行器依次启动，用各自的自有数据库同步原 TC 的批量写入，并保留原始帧与独立 SQL 快照。[并发登记验证](../../docs/phase-2/tc-lock-coverage-contract.md#重叠锁批次的并发登记)

`python -B tools/formal-tc-lock-coverage/live_guard.py 1.99 --java-owner --tinyint-key boolean --java-writer-admission` 运行 Java 写入器准入场景，使用 `1.85` 复跑。先验证不匹配配置未写入业务库或登记分支，再验证同一部署绑定下的原 Java 与 Rust 锁竞争。

在同一命令追加 `--separate-writer-roles` 可用三个独立 MySQL 账号复跑。运行器记录实际授权、MySQL 权限拒绝，以及按已认证账号汇总的业务语句和绑定读取。Rust 的触发器授权只覆盖当前业务、Undo 与分支恢复表。

用 `--rust-owner --tinyint-key boolean --java-writer-admission --separate-writer-roles` 运行分离账号的有变化提交场景。运行器先验证 006 触发器权限缺失时的明确拒绝，再授予此表的直接 `TRIGGER`，核对原 Rust 持锁提交、Java 冲突和释放后登记。

在分离账号的反向提交命令上追加 `--definer-metadata`，复跑直接过程授权模式。运行器核对无过程授权时的 006 拒绝、授予 EXECUTE 后的原 Rust AT 提交，以及运行账号直接读外键字典和修改过程时收到的权限拒绝。
