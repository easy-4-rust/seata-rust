# WP-00 原始候选语料复核

核对[原 29/6 语料](../../docs/phase-0/wp00-vector-freeze-review.md)，不将现行 any-mismatch 任务缩为原语料；完整任务、Phase0/WP00 和正式组件仍有剩余门禁。

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
$env:PYTHONUTF8 = '1'
$env:TEMP = 'D:/Temp'
$env:TMP = 'D:/Temp'
python tools/wp00-vector-audit/run.py
```

需要既有固定 Java JAR、Go 原源码和本地 JDK/Go/Rust 工具。只在 D:/E: 创建新运行，原样例不改写，不安装工具，不启网络服务。

脚本核对文档 25 行和全部 29 夹具；新执行原 Go 独立对象构造/编码的 29 样例与 6 帧。Java 从已认证原 JAR 的 BOOT-INF/lib 提取全部依赖，优先加载，直接执行现有 29 载荷及 6 帧断言体；没有运行 Maven 或 JUnit 引擎。原 Boot 配置 SPI 使用公开 ObjectHolder/StandardEnvironment 和 file 配置启动，生产类/序列化器均不改写。实际加载的原 Seata class 对原 JAR 对应 lib 字节逐项认证；lambda/CGLIB 动态类单列，不能称为原 classfile 匹配。

Rust 在 D 盘副本构建现有测试 binary，执行 117 项探针测试。Java 和同一 Rust binary 都对有效 timeout2000→2001 的变异触发精确目标失败；Java 其余 34 断言作为控制。变异只修改新 D 盘副本，结束后核对恢复字节。

源、原夹具、动态 Python 支持、类路径来源 XML 在启动前冻结；实际 JAR/classpath、编译产物和执行 binary 前后核对。该结果只证明原语料，补充差异和生产 Rust 不从这些计数推导。详见[正式证据](../../docs/phase-0/evidence/local-wp00-vector-audit/README.md)。
