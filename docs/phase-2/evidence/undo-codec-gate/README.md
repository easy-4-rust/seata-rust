# WP05 选定Jackson文档与实际准备日志编解码

[最终修订验证](../undo-codec-refinement/run_20261115_235850_b36442a0/result.json)完成[文档格式契约](../../undo-codec-document-contract.md)的选定组件验证；[当前机器状态](../../wp05-undo-codec.json)。原WP05原子提交Gate继续开放。

最终7978输入/8452成员，catalog SHA256=`6f562a7ae6bc1a992433ff990776eb873bb39fe3370ecda02fbe2bfdc78a0a59`。Rust1.99/1.85各新执行316工作区测试（92个测试产物）、1个真实准备日志场景（各2个JDBC配置×4条日志）及12个合计RM恢复子进程；最终[文档peer](../undo-document-peer/run_20261115_235646_c842e6f1/result.json)新执行40/40双向运行时MATCH。fmt/Clippy/rustdoc及全工作区all-features/all-targets检查通过。

[最终只读复审](../undo-codec-refinement/readonly-review-20261116.json)无剩余P1/P2；[发布核对](../undo-codec-refinement/publication-20261116.json)确认生产源码、Cargo、夹具、依赖和实际测试产物未变，7978冻结输入中仅工具README随结果发布更新，原封存不改；当前无自有MySQL容器运行残留。

最终epoch修复审查发现的容器深度不对称和Jackson数字读取限制。两项都有真实目标RED→GREEN；原Java数字探针14例为7接受/7因长度约束拒绝。节点预检原实现正确，Row还有@class字符串节点，因此新增40节点接受/39拒绝回归，不把早期错误测试预期39记作产品RED。最终相对首次整轮仅改`strict_json.rs`一个生产文件和3个测试文件，267个其余crate文件字节不变；185依赖archive和6871源码也保持原字节。

下面首次整轮的每版38项其他DB测试与5普通/1真实差分负控明确继承，不声称已在最终源码重新执行39项。只有调用codec的现场路径在最终epoch重新执行。容器深度现在按对象/数组从1起计，scalar不加层；数字尾数/指数合计最多1000个数字，符号和点不计，解析/树校验均约束。

## 首次整轮及其历史继承范围

[首次整轮](run_20261115_232915_d3f704e8/result.json)是审查修复前的完整回归，以下数字保留其原epoch含义。

7975输入/9071归档成员，catalog SHA256=`be839e7815f826755158a352ac30c0918f37922cb4af2d75a2362e3b4b0fbddb`。[只读字节复核](audit_run_20261115_232915_d3f704e8/result.json)catalog=`e488185710e68b6fead7be9ed20fa38117a8074846c59bbac0d671cd4e4b1963`，核对全部输入、12个正向子记录、5个隔离负控和149份未变的协议/传输/TM/RM源码。负向Java在故意抛断言后未被包装器复制的class-load日志，另从原执行目录补存，全部文件类来源仍逐SHA绑定。

|验证边界|实际结果|
|---|---|
|Rust1.99 / MSRV1.85|各313工作区测试；92个工作区测试产物|
|真实MySQL六组|每版39：元数据9、行镜像10、查询间竞争1、DML17、原生类型1、实际准备日志1|
|实际准备日志|每版在同一自有数据库中，Rust和原Java JDBC独立执行相同SQL/值；两配置各4日志，原Java类型/值/容器均MATCH|
|文档双向差分|33个文档Java→Rust→Java MATCH；原Java再编码→Rust→Java再次33 MATCH|
|普通实现削弱|默认对象预算、小数指数/scale、解码二进制长度、重复键、UINT32包装类5项均编译后命中目标断言|
|实际值差分削弱|颠倒二进制：Rust本地DML1例通过，原Java对照因值不同失败，只有1个声明生产文件被改动|
|旧RM通用恢复|12个实际恢复子进程；不代表数据库Undo恢复|
|依赖闭包|185个registry锁定版本/checksum/边未变；150个实际编译包、6871份源码逐归档成员核对|

12个正向及1个负向自有MySQL容器均清理，数据目录留D盘。固定MySQL8.4.13/lower_case_table_names2、测试环回连接不启用TLS；生产TLS/权限仍未验收。每个实际Java调用绑定原源构建、全部runtime输入和class-load来源。完整原Maven环境未重建；helper版本范围延续[原格式工具说明](../../../../tools/formal-undo-format/README.md)。

## 实现与可观察语义

`JacksonUndoLogParser`严格读取JSON文档并有界输出；`UndoLogDocument`是只读原对象图投影，区别于已校验的事务模型。保留null/缺失默认值、null集合元素、ArrayList/LinkedList、EmptyTableRecords具体类、JDBC码和完整数值词法。原两字节`{}`按缺省Bean解释；重新编码输出完整Bean。原48个SQLType词汇只用于文档，不扩大业务SQL支持。

重复键（含Unicode转义同名键）、私有Number伪造、尾随内容、未知类/成员、越界整数/时间戳及超预算输入被拒绝。整数/BigInteger/BigDecimal不经f64；BigDecimal指数和32位scale受原Java实际拒绝探针约束。32MiB编码/262144节点/32深度/单字段1MiB等预算同时生效，可收紧；scalar检查仍可能在临时String分配之后发生，不承诺精确进程heap上界。

真实事务准备日志按明确的Connector8默认或numeric tinyint配置编码。UINT32始终为Long、U64为BigInteger，原生文本/二进制及DECIMAL类型不合并；默认signed tinyint(1)仅物理0/1可无损转Boolean，物理2须numeric配置或拒绝。没有原生类型身份的模型不能被猜测编码。

实际RED分别证明：缺省`{}`展开曾绕过结构预算；原Java拒绝的非法小数指数/scale曾被Rust接受。两项均修复并进入本轮GREEN。早期Java参考程序的`setObject(BigInteger)`会按原驱动BIGINT路径转有符号long；保留230406失败和固定驱动源码证明，改用显式精确数值绑定后才比较U64最大值，未删除输入或修改驱动。

## 源测试与资源范围

最终40文档=12个原Java原生表生成资产+4个Rust义务/原Java接受探针+17个固定源断言值/对象图与scale边界探针+7个原Java接受的数字边界。17项覆盖时间戳/LocalDateTime、SerialBlob/Clob、SerialArray/NULL元素及可空SQL表名；这些是值探针，原JUnit方法没有被实际执行，不能计作完整方法迁移。数字读取14个原Java正反探针的原报告和七份接受文档保存在最终peer，未覆盖改写旧17份固定源值资产。

[源码库存](../undo-source-inventory/run_20261115_231536_dbc7bfc1/result.json)是189文件/1135个已识别测试方法声明；参数、继承、重复及动态运行案例仍未完整展开，声明保持MISSING/未差分。完整源案例与覆盖率门禁未完成。

[资源来源纠正](../../java-resource-provenance-correction.json)：历史7份`java_source_assets`是Windows checkout字节副本，都与Git blob存在CRLF/LF差异；保留旧副本/封存证据，另存`java_git_assets`的固定Git blob原字节。Git属性保护新原字节资产，避免后续checkout再次转换。7份资源不是7个测试案例；[源值探针资产](../../undo-source-probe-assets.json)也与原测试资源分别记录。

原Java5.1.42在MySQL8.4的两个连接失败仍保留，不能用8.0.27通过替代。尚无同连接Undo持久化、实际TC分支登记、提交未知围栏/恢复或公共COMMIT；WP05原14项、WP06、WP00-R、完整V1、发布及Phase4–7继续。外部TC风险未接受，共享锁暂缓。复现方式见[验证工具](../../../../tools/formal-undo-codec/README.md)。
