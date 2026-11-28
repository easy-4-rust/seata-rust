# DATE/TIME 正式 SQL/Undo 与原 TC 现场（2026-11-28）

已接入独立物理列域、原生身份、镜像投影/精确参数、INSERT 临时主键表、原 Jackson 编码/恢复、锁键与显式持久配置码 5/6。旧码 1–4 的语义保持不变；它们拒绝 DATE/TIME 编码与恢复（包括 NULL）。使用精确模式的 Java 写入器须安装 [JDBC 适配器](../../../../compat/java/README.md)，保持同一资源约定。

权威现场：`java_20261128_112133_38926f66`，catalog SHA-256 `a30c2955b07293b2310bac4c4bb79e8a1bbcd8204a3ebf7a484a9451ad447bb3`。固定原 Java 提交 `cba73e180fd8896a99df6415b859d275122feaa7`，原 TC JAR 未修改；只安装既有身份检查测试钩子。MySQL 8.4.13 使用独占数据库与容器，最后已停止并移除。

Rust 1.99/1.85 × Boolean/数字 TINYINT 配置 × 提交/回滚，共 8 笔实际全局事务、48 条业务 DML。每笔在 TIME 全列镜像上执行 UPDATE/INSERT/DELETE，并在 DATE 主键上执行 UPDATE/INSERT/DELETE。TIME 覆盖负一微秒、±838:59:59、超一天时长、FSP 0–6 与 SQL NULL；DATE 主键覆盖 1582 年合法日期和 9999 上界。TIME 主键的真实数据库路径及 DATE 普通列完整矩阵仍需补验。

原 Java 精确数据源读取的两张表均使用原 TableRecords 和 Jackson parser 生成三类 SQL Undo，与 Rust 编码逐字段相等。回调由原 Java TC 发往正式 Rust RM；本轮没有声称运行了 Java RM 的整个事务实现。第一阶段实际核对：分支与提交尝试配置一致、Undo 原子持久化、SHA 与原字节相符、见证标记为 1、六把 TC 锁存在。错误旧配置恢复及加载提交尝试被拒绝。二阶段核对业务最终值、Completed 日志、持久终态、Undo/防悬挂记录和 TC 全局/分支/锁清理。

负例实际观察：预编译业务 TIME 越界字符串返回 1292 / 22007，事务随后被 poison；不能拿原 SQL 字面量路径的 1264 作为同一路径断言。零 DATE 作为非法值被拒绝，未降为 NULL。前两次 `java_20261128_111838_*`、`112007_*` 因此错误码断言失败，已标为诊断，不计验收。

独立核验由 `validate_exact_date_time_column.py` 执行：原图、现场业务/锁/配置、原始协议及回调相关性、日志、进程/数据库清理和来源均核对通过，160 次证据削弱被拒绝。实际运行输出为 `native-validation_20261128.txt`。运行时完整源码输入和依赖封存在各 packet 中；后续全工作区编译仅补齐一个不在本现场测试目标中的旧 Fastjson 测试辅助函数的枚举匹配，生产逻辑及本现场目标保持相同原字节。最终检查同时核对封存字节及当前生产/现场源码绑定。

Rust 1.99 与 1.85 各 822 项通过、404 项忽略、190 个成功测试汇总（含 rustdoc），严格 Clippy 均通过。全工作区回归与严格 Clippy 见本目录最终 `regression_1.99_*` / `regression_1.85_*` 的真实 result.json。`regression_1.99_20261128_112157_*` 是旧测试辅助函数穷尽匹配编译失败的诊断；没有计为通过。`tdd_20261128` 保留目标 API 缺失的红灯与初次绿灯日志。

尚未关闭的验收：TIME 主键现场、DATE 普通列完整值域、该精确配置下的独立进程强杀/重启及篡改原字节恢复、旧 DATETIME/JSON/AT 的新现场回归和写入器准入。完整 DATE/TIME、WP05、V1 与 Phase 0–7 均继续开放。Fastjson 路径和其他 JDBC 版本仍未认证，不能由这八笔事务推断生产容量。

最终封存入口 `validation_20261128_final`，catalog SHA-256：`91023027c93f2c44cc119f4fa6ee0130ec13d18c3ef70c43db75adccb577cdcd`。复核当前生产源码及现场目标、唯一旧测试辅助函数六行增量、两次完整工作区回归和原始权威现场，脚本为 `validate_exact_date_time_checkpoint.py`。
