# 正式Rust协议与固定Go显式适配器比较

运行 `python tools/formal-protocol-peer/run.py`，消费已封存的Java证据和独立预期，编译两个实际程序比较628正向/2050负向。Rust入口调用正式seata-protocol，Go入口调用固定0001–0006副本公开API，不修改已认证副本。独立语义由corpus.py构造；两个程序只接收ID/类型/原字节，不接收预期结果。

比较涵盖27类型全部线缆字段及Java nullable投影、注册raw/default版本、报告缺省、结果/异常/状态、子树和有序头；全字段JSON严格比较区分bool/int和i64精度，字符串统一hex仅用于比较。正向要求重新编码逐字节相同；负向必须解码拒绝，panic或decode成功后encode_error均使门禁失败。

Go Getty返回等待/只消费第一帧时，完整帧比较包装器将该条记录视为“不是一条完整输入”，不改变实际Getty流语义。跨实现数据使用明确测试预算，不等同所有资源策略边界完全一致。

同时执行正式74组件测试与独立验证器单测，在Rust1.99/1.85各过；静态/文档门禁、7Python、12实际输出破坏、原/候选源及1936依赖前后校验。初始Python导入RED与验证器实际运行RED1分别记录。不运行TC/DB，不重跑历史Java或Go254测试。

workspace新增transport后，本工具的Rust测试/格式/Clippy/doc明确限定seata-protocol包；所有workspace成员Cargo manifest仍冻结，读取范围与质量命令一致。transport测试和workspace质量门禁由对应实施记录单独执行，不将新增包测试误算成协议74项。

固定Go副本：`D:/codex-builds/sif_20261112_232026_e565657b`。源码、依赖缓存、构建和输出均使用现有D:/E:环境；缺少环境时失败，不安装/初始化或改写候选。原Go默认差异及完整客户端退出条件继续开放。[正式证据](../../docs/phase-1/evidence/protocol-peer-profile/README.md)。
