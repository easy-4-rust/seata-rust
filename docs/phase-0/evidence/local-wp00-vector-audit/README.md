# 原 29 载荷 / 6 帧：当前三方复核

正式运行：[run_20261113_133856_5af18bca/result.json](run_20261113_133856_5af18bca/result.json)，对应[核对口径](../../wp00-vector-freeze-review.md)和[工具](../../../../tools/wp00-vector-audit/README.md)。原 29/6 无字节差异；这不把现行 any-mismatch 任务缩为原语料，原 checkbox 继续未勾选，补充差异、完整 WP00/Phase0 和正式 Rust 均未关闭。

| 本轮新执行 | 实际结果 |
|---|---|
| 文档及夹具 | 文档 25 行表、29 个原二进制、类型前缀、manifest 与 JSON 逐项一致 |
| 原 Go codec 工具 | 独立对象构造/编码/往返的 29 载荷、6 帧全部通过 |
| 原 Java 断言体 | 现有 29 载荷语义断言＋6 帧断言，35 全通过；直接调用，未运行 Maven/JUnit 引擎 |
| 原 Rust 探针 | D 盘源码/夹具副本新构建的测试 binary，117 项全部通过 |
| 有效语义变异 | timeout2000→2001：Java 精确 1 失败/34 控制通过；同 Rust binary 的独立语义目标失败；原夹具未改 |
| 来源 | 643 个冻结输入；475 个实际 classpath JAR；158 个实际原 Seata class 匹配认证原 JAR；16 个动态类另列 |

Java 固定点 cba73e180fd8896a99df6415b859d275122feaa7，Go 固定点 82570d905919c1c66bb025a3a5979f25390de498。原 JAR SHA-256：aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5。全部原 BOOT-INF/lib 优先加载，原 ProtocolDecoderV1/ProtocolEncoderV1/serializer 的实际来源来自同一认证 JAR，未使用前导分片候选替代原 decoder。

原 Boot 配置 SPI 通过公开 ObjectHolder/StandardEnvironment 初始化，并显式选择 file 配置，file listener 关闭；没有修改原生产类或启动 TC/数据库。lambda/CGLIB 是运行时生成类，仅列观察，不能声称与原 classfile 逐字节相同。实际原库、编译后的三 Java 类、Go/Rust binary、D 盘 Rust 副本及原仓库输入前后核对。

[完整输出清单](run_20261113_133856_5af18bca/catalog.json)，SHA-256 `ad0789a861cba04070abecd7bd148761177ce1fe7fc7e388d77cbfdd5cdac153`。输入清单包含实际类路径来源 XML；35 个 Java 断言的逐项身份、输出字段/编码，117 Rust 日志、两次精确语义负控、原类来源、编译参数和 binary 都已保存。

早期运行仅作为工具诊断保留：缺 Boot SPI 环境导致 6 帧初始化失败；完整 classpath 超过 Windows 命令长度后改用参数文件；配置名与动态生成类分类分别修正。133431 的实际结果已通过复审，但缺少外部来源 XML 冻结，不作为最终闭包认证，没有补写其输入清单。以上不被算作上游协议缺陷或新的生产修复。

本项没有新认证补充三项失败差异、59/60 原 Go 默认路径、全部字段变体、生产客户端或现场事务。对应原任务仍开放，正式实施顺序与待确认提案保持原状态，共享锁继续延期。
