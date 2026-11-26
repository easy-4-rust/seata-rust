# WP05 本地业务执行与前后镜像（部分组件）

[最终执行](run_20261115_205718_cc203af6/result.json)验证[实施契约](../../mysql-execution-images-contract.md)：受限INSERT/UPDATE/DELETE真实执行、完整前后镜像与内存SQLUndoLog准备。当前没有公开COMMIT，尚未实现Undo序列化/同事务持久化、原BranchRegister或提交未知恢复。WP05原十四项保持0/14，WP06及后续计划继续。[机器状态](../../wp05-mysql-execution.json)。

最终冻结7971输入、10996成员，catalog SHA256=`07bed67b72f1501d5dcc7877aa498b1dc0fd186f67696571b00fc14293a45d5e`。Rust1.99与最低1.85各重新执行291项普通工作区测试及37项真实MySQL场景，保存80个工作区产物、8个独立正向现场产物。五项普通削弱、六项真实MySQL削弱均编译成功并在指定目标失败；六个负控产物单独归档，不与正向通过混计。既有RM通用文件恢复四组12子进程重新运行，不能代替SQL/Undo进程恢复。

| Rust | 场景集 | 通过 | 子记录 | catalog SHA256 |
|---|---|---:|---|---|
| 1.99 | metadata | 9 | [run_20261115_205956_d9749e96](../mysql-metadata/run_20261115_205956_d9749e96/result.json) | `94704599e0252a6c3c357fd9483bcaaebe43062e47984838cb6c10df5991d5c3` |
| 1.99 | row_images | 10 | [run_20261115_210039_69155b9b](../mysql-row-images/run_20261115_210039_69155b9b/result.json) | `70a04580c8a677ae2f964b1f7bdbc5157059130c1ffd2b7e5f231ccb23c9c559` |
| 1.99 | image_race | 1 | [run_20261115_210119_52d00df8](../mysql-image-race/run_20261115_210119_52d00df8/result.json) | `a28fc8b20794efd01cac242544841ffb59b8b2b63eb786a9c9b8d6e137698122` |
| 1.99 | execution | 17 | [run_20261115_210152_41bbeafd](../mysql-execution/run_20261115_210152_41bbeafd/result.json) | `4bf709dbedac759366661912975a96d1e66835b609884991c1ff239a495793dc` |
| 1.85 | metadata | 9 | [run_20261115_210247_c24a17dc](../mysql-metadata/run_20261115_210247_c24a17dc/result.json) | `6ad0145da1902b8092864371e0b35d0be6e6f2d9bd9dc3e12f48784b4edcb8ad` |
| 1.85 | row_images | 10 | [run_20261115_210338_b30ed19f](../mysql-row-images/run_20261115_210338_b30ed19f/result.json) | `15e54105b3ca9e87445f153ab39fc6fb8cbd90182f630e5dfd81ae41b9c86370` |
| 1.85 | image_race | 1 | [run_20261115_210423_951c938c](../mysql-image-race/run_20261115_210423_951c938c/result.json) | `1e0033c26ea18a2bf4c32984488361cab72f19d12d4907243cafe06ffb2e9080` |
| 1.85 | execution | 17 | [run_20261115_210503_3a16e276](../mysql-execution/run_20261115_210503_3a16e276/result.json) | `68c9e6f5e636e3eca24bd10e4384194cc07fede643268d0ac1314a914c2da286` |

每版37项为原metadata9、row10、精确查询间竞争1及新execution17，均为当前源码的新执行。显式复合PK多行INSERT、输入列顺序/参数切片、i64/u64极值、65位DECIMAL/30位scale、二进制/NULL均核对；UPDATE在原WHERE列被改后仍按原PK取after，DELETE后为空，零匹配不造日志、无变化不强求changed-row计数。INSERT general_log独立核对原业务执行一次，独立连接确认业务与Undo行尚不可见。

真实字面量`'1.6'`暴露CAST取键会截断而真实整数列赋值会舍入，旧实现可误捕获相邻旧行；已改为同连接私有InnoDB临时键表，以真实目标类型赋值，再删除临时表、执行原INSERT一次。绑定文本的早期探查通过，明确不是该缺陷的RED。CHAR去尾空格、BINARY补零及DECIMAL舍入有现场证明。128条日志超限原先仍可读取元数据的状态漏洞也已修复。

额外现场验证1044缺临时表权限、1048临时键NULL赋值失败、累计16MiB及后镜像超限：均使事务poison，先前业务未被隐式提交，只能整体回滚。仅CREATE/DROP TEMPORARY；成功显式DROP，失败/取消/超时依赖close_on_drop物理会话退出。临时表本身不随ROLLBACK撤销，不能把采用临时表当成原子性证明。INSERT现需CREATE TEMPORARY TABLES权限，不自动授权；真实业务表匹配下一内部名称的冲突也已验证不会遮蔽或删除。

两个取消及一个期限场景扣住原COM_STMT_PREPARE：业务后镜像查询或临时键赋值后的SELECT。先由独立写入1205证明原锁，临时键路径另见Temp=1；取消/Deadline后放行原始包，在代理仍存活时确认Temp=0、写锁释放及原值10恢复，最后才关闭代理。代理任一方向EOF即释放双向连接，避免测试代理自身延长旧会话。去掉实际事务所有权会在目标失败，不能用poison状态或代理关闭替代数据库事实。

五项普通控制覆盖INSERT参数位置、Undo schema/方向/PK身份/重复。六项实际控制覆盖旧CAST误捕获、条数poison、累计payload、错用原WHERE取after、丢失Future事务所有权及临时名称遮蔽。每个执行源/现场产物有独立副本与SHA。

185个registry锁包checksum未改；实际cargo tree的148个registry包与双版本compiler-artifact集合相等，6745个实际源码逐原crate成员校验。metadata159候选不等于实际编译闭包，未声称所有可选依赖都支持最低Rust。Cargo仅新增本地AT到datasource的依赖边。[核心绑定](core-binding-audit-20261115.json)区分未改协议/传输/TM/RM与本轮AT/DB变更；旧Java/Go Gate未新跑。

14个Java对象PARTIAL、两处原方法引用、189测试文件仅库存、七资产原字节复制；本轮未完成Java具体案例差分或Undo序列化互通。MySQL固定8.4.13/lower_case_table_names2/loopback非TLS；八正向加六负控共14个自有容器均清理，D盘数据目录保留。容器数不是入口初始化的全部mysqld次数，生产TLS/最小权限仍未验收。

API缺失属于编译RED；真实主键转换与条数状态分别有行为RED。命名测试初次直接把general_log BLOB读为String，16通过1失败，属于测试错误。之后一轮17项通过但运行期间测试文件被误改，源校验拒绝封存；原文件已恢复，该轮不计正式通过。本最终父门禁在冻结后完整重跑并校验输入/产物不漂移。

额外[严格跨包审计](strict-cross-packet-audit-20261115.json)把八个正向子包的共同编译输入逐项绑定父快照；唯一额外现场输入为原Java mysql.sql，父包未直接列入，另经旧封存catalog/子包原schema逐SHA绑定，全部14现场相同。六负控只允许声明的一个变更文件，其余共同输入逐SHA相等。失败逐项绑定预期诊断；命名控制实际在显式“临时表不得同业务表名”断言失败，并非1054；所有权控制为已创建临时表的清理期限断言，不笼统把任意panic当成目标assert失败。

复现 `python tools/formal-mysql-execution/run.py` 后，以 `python tools/formal-mysql-execution/audit.py --run <本次父目录>` 执行严格审计；两步都通过才发布。开发及运行输出仅D/E盘。完整V1/发布、原生RustTC、WP00-R及所有未完成计划继续；外部TC风险未接受，共享锁按用户要求暂缓。


[最终独立只读复审](readonly-review-20261115.json)通过；严格跨包审计再次重放，无新阻断，范围保持部分组件。
