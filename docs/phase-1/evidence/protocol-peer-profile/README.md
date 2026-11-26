# 正式协议组件：批准首期peer-profile门禁

固定Java语义、正式Rust组件及显式Go0001–0006适配器完成全部已认证协议语料的逐字段/逐字节比较。按已批准的初始peer profile，**WP-01协议组件Gate已验证**；下一工作包为WP-02连接与关联。原Go默认入口差异和完整Go兼容仍属原WP00-R/总目标，不因本门禁通过而取消。

正式运行：[run_20261113_174850_d10c8fed/result.json](run_20261113_174850_d10c8fed/result.json)，[1914成员清单](run_20261113_174850_d10c8fed/catalog.json) SHA-256 `8a5ba6f717bb2f9be1a01b7830620304b3378e3c8aa1ae6918a0f42c3c577fdb`。1847输入、1936项Go编译依赖及20实际产物（16正式测试binary、2验证器测试binary、2peer程序）前后核对；只读复审独立核对全部SHA与实际比较结果。

| 本轮实际执行 | 结果与限定 |
|---|---|
| 628正向上下文 | 576payload＋52完整frame；两个真实程序分别调用正式Rust和显式Go公开API，全部字段树及新编码字节与独立Java预期一致 |
| payload集合 | 原29、250异常、168状态、7补充失败、78处理器侧车、38扩展聚合＋6控制聚合；覆盖批准27类型 |
| frame集合 | 六原帧、九控制帧、37条既有新Java原编码/解码帧；本轮复用其封存，没有再执行Java |
| 2050畸形输入 | 原载荷/控制聚合/原及控制帧的截断前缀和尾随、未知/未支持类型、235未知全局状态、231未知异常码、magic/version/头与全长/编码器错误均未被完整消息入口接受；无panic/encode_error被算作正常拒绝 |
| 独立语义 | Java实际getter字段、固定显式enum源、原异常/状态构造规则、failure输入及独立侧车；不调用任一待测decoder生成预期。状态诊断JSON的展示文本有历史编码损坏，按已冻结Java构造规则使用原UTF8文本“状态诊断:IO” |
| 字段边界 | report255缺省与0、注册wire/default版本、nullable/空值/有序头、外层/子请求ID、i64、bool、结果/状态/异常域、所有子消息顺序均分别比较；字符串hex仅为验证表示，不修改原字段内容 |
| 正式Rust回归 | 74组件测试，Rust1.99/最低1.85各通过；fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过 |
| 验证器与负控 | Rust真实注入重编码失败单测1项在两工具链通过；7Python比较器测试；12份实际输出破坏全部拒绝，包括双方同错、bool/int、null/空、精确i64、异常域、report、版本、头、ID、字节、panic/encode_error及缺case |
| Go身份 | 原714文件及候选714基线/24候选条目/13辅助条目审计前后相同；依赖源码和新binary/build-info闭合。程序未给原registry新增59/60，不把适配器结果改称原默认兼容 |

Go Getty是流接口，短输入可能返回“等待更多”，连续输入可只消费第一帧；本比较包装器要求非nil对象且consumed等于整条输入。它不声称Go本身将流分片当错误，也不把Rust单帧API的拒绝说成已实现TCP流处理。比较采用显式测试预算；不声称Rust可配置资源策略与Go固定策略的所有边界完全相同。原Rust预算边界由组件测试单独验证。

复审发现Rust验证器最初将decode与encode错误合并。真实注入单测运行RED1后，解码错误才输出rejected，解码成功后的编码错误独立输出encode_error并导致门禁失败。该缺陷属于验证工具，不是新增生产协议缺陷。最初Python比较器RED是缺模块导入失败；12个实际输出破坏是证据负控，不是上游产品RED。

本轮没有新Java/JUnit、Go254历史测试重跑或现场TC/数据库；旧probe117、Go254与Java各阶段记录只作继承背景，不计本轮新执行。没有安装工具、创建分支或更改上游/候选生产代码。完整V1/发布、WP00-R、原Go默认差异、所有连接/TM/RM/资源原子性/恢复及后续Phase仍未完成；共享锁继续按用户要求延期，外部TC风险未接受。

复现：`python tools/formal-protocol-peer/run.py`。需要现有D:/E:固定源码与本地缓存，工具只创建独立运行目录；不联网安装，不写F盘，不重新运行旧归档器。[门禁状态](../../wp01-protocol-gate.json)记录证据与后续阶段。

2026-11-13连接层引入Tokio/Cargo.lock后再次回归：[run_20261113_191025_d1a9ffd7](run_20261113_191025_d1a9ffd7/result.json)，1848输入/1915成员，catalog SHA256=`e89f562278c942a3de7af0575357a93e66045dd563b992d25ef62109cf2b5c87`。以上74＋1双版本、628/2050、7Python/12实际输出负控均新跑通过。只验证协议包，连接层33项由[独立单会话门禁](../transport-session/README.md)执行与归档，不能把协议比较数充作TCP业务验收。

2026-11-13加入开发期test-util功能后新跑：[run_20261113_192819_1650e24d](run_20261113_192819_1650e24d/result.json)，1848输入/1915成员，catalog SHA256=`b8ddd7aac9ee5c776a9268f74bb1ffeec09c7939907aaf905b50438bcf84e9e2`。同一协议门禁全部通过，70编译协议/Cargo输入与[47项心跳/idle门禁](../transport-heartbeat/README.md)绑定；没有新Java执行或现场TC，不扩大原Go默认兼容/完整V1声明。
