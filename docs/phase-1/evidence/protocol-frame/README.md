# 正式 seata-protocol：完整V1帧、HeadMap与心跳

已完成原WP-01九项实施任务：九组显式域转换、27类载荷、逐字段空值、完整16字节帧、有序可空头、空心跳、全长/预算与畸形输入拒绝。编码/解码按Java对象独立文件实现，入口不缓存原始输入代替模型。

正式运行：[run_20261113_172217_7c824d56/result.json](run_20261113_172217_7c824d56/result.json)，[1192成员清单](run_20261113_172217_7c824d56/catalog.json) SHA-256 `2993ad4fa4dea37abf3b81a4da4ab698eade391830f59090c39e7f02da800815`。1148输入及两版Rust的实际测试产物冻结、前后核验；旧封存保持原样。172127已完成同样测试，但Java执行计数表述不清；172217明确本验证器只读引用独立新Java运行，不将它说成本脚本内再次执行。

| 实际验证 | 结果及边界 |
|---|---|
| 正式Rust | 74独立测试，1.99/最低1.85分别通过；7原语＋5码表＋30原载荷＋8普通边界＋3Java记录族＋8聚合＋12帧＋1处理器语料族 |
| 既有认证语料 | 原29payload/六frame、425Java记录、44聚合/nine控制frame、78处理器上下文独立侧车语义全部接入正式API；不同上下文可能有相同字节 |
| 新实际Java | [严格171558运行](../formal-frame-java/run_20261113_171558_83fc397c/result.json)37帧：原29载荷＋null/empty/empty-key/ordered Unicode头、32767字节头、ID最小/最大及单向请求；原encoder生成、原decoder逐字段核对，无JUnit引擎 |
| 新Java到正式Rust | 37帧与独立标量/头预期和既有已验证载荷组合对象比较；从对象编码与原Java字节一致；37帧全部截断前缀拒绝 |
| Java执行来源 | 159原生产class逐一对应原JAR内lib，7关键来源、475依赖、helper二进制与源输入前后校验；184成员catalog=`ebd29a04818c319bb1d8c805e84b25f9ed1ddd2717ceb4133a992e420e6a4b4a` |
| 头与帧边界 | 有序nullable头、空key、signed ID；null/重复key、非法负长/UTF8、头跨body、错误magic/version、full/head长度、尾随、未知码及未实现编码器拒绝；精确预算、32767头边界及接受的单字节变异逐字节往返 |
| 质量 | fmt、全部target Clippy -Dwarnings、rustdoc -Dwarnings；原独立probe117及fmt/Clippy均新跑通过；只读复审无剩余阻断 |
| 运行工具门禁 | 4项Python类来源正负控通过；只读核验两个Java捕获370成员及JSON/TSV派生关系。初始API缺失RED为编译失败，来源helper缺失RED为导入失败，均非运行期断言RED；头预算顺序另有1个真实运行RED及修复 |

复审修正了两个P2：头key先检查长度/预算并预算内写入，再做哈希；Java非file生产类只能是严格匹配的Lambda/CGLIB来源，其owner还须对应原JAR class，其他情况拒绝。较早171203真实37帧保留，严格171558重新执行；夹具payload/TSV字节未变，单独certification链接严格运行。171139在JVM启动前因日志路径选项失败，保留诊断，不计Java帧执行成功。

头key为空串允许，null key/重复key/-2等非规范负长是严格profile主动拒绝，不能说原Java也拒绝。心跳body必须为空且不加120前缀；普通信封必须有完整Some消息。仅SEATA=1/NONE=0执行，其他已知与未知能力分别报错。FrameLimits全部显式配置；不是固定继承探针8MiB设置。Debug不打印头/载荷字符串。

所有原WP-01实施checkbox已核对完成；**完整WP-01 Gate仍开放**，下一步收敛正式peer-profile的完整三方比较与显式Go适配证据。原Go默认失败/59/60缺口不自动关闭，完整Go兼容仍在原总目标内。完整客户端、TCP分片/连续帧、连接代际、TM/RM/资源原子性与恢复均属后续门禁；完整WP00-R、外部TC风险与共享锁延期状态不变。没有启动服务、安装工具或改写上游生产代码。
