# 固定原Java：正式协议组件的37帧对照

最终严格运行：[run_20261113_171558_83fc397c/result.json](run_20261113_171558_83fc397c/result.json)，[184成员清单](run_20261113_171558_83fc397c/catalog.json) SHA-256 `ebd29a04818c319bb1d8c805e84b25f9ed1ddd2717ceb4133a992e420e6a4b4a`。

实际执行原Java V1 encoder/decoder，生成37帧并检查所有body JSON字段、头map、请求编号、外层类型、serializer/compressor及完整消费；不是JUnit或TC联测。29原载荷与8个头/编号/单向边界样例，无服务。159实际原class、7关键来源、475依赖和helper二进制/9输入闭合；固定源码无HEAD差异，JAR SHA保持原认证值。

Lambda/CGLIB只允许明确形态/来源并绑定原owner；普通生产类的非file来源不能绕过。只读回放工具同时验证原171203捕获与本严格运行370成员，并从JSON逐字节再现TSV。171203已实际通过37帧，但当时来源分类过宽，现保留历史并以严格运行为最终认证；171139仅JVM启动失败的诊断。

导出夹具的原manifest保留171203身份，字节未改变；新增certification指向171558严格运行，未覆写旧封存。[正式Rust验证](../protocol-frame/README.md)单独执行74项测试，不能把它计作此Java运行的额外Java测试数。
