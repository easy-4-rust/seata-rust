# 原Java Undo格式对照

此工具建立固定原源/驱动/配置的实际格式对照，并提供原Java文档/实际准备日志peer。当前选定Rust编解码的实际结果见[组件证据](../../docs/phase-2/evidence/undo-codec-gate/README.md)；Java全部源案例、Undo落库和原子提交仍未完成。

首次准备精确依赖时执行：

```powershell
$env:TEMP='D:/Temp'
$env:TMP='D:/Temp'
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/formal-undo-format/prepare.py
```

只读公开缓存或获取固定版本Maven Central原包至D盘，并核对官方实际发布的校验算法，再固定归档SHA256；不安装CLI。运行步骤：

```powershell
python tools/formal-undo-format/build.py
python tools/formal-undo-format/live.py --build <本次UNDO_JAVA_BUILD目录>
python tools/formal-undo-format/verify.py --build <同一build目录> --live <本次UNDO_JAVA_LIVE目录>
```

原Java pin固定。选定18份源码实际编译，依赖/fallback字节完整封存；fallback不视作整个Java模块已重建。独立复核把源码内容绑定Git pin（只允许记录的checkout行尾变化），并重核全部实际runtime输入、类加载来源、生成类owner、默认两字节内容、具体Empty子类及原生成资产。

Jackson2.18.3、Caffeine2.9.3及驱动5.1.42/8.0.27按原声明固定；其他helper使用明确冻结的server/lib版本，因此这不是完整原POM环境。每个驱动分别测试默认和tinyInt1isBit=false。8.0.27在MySQL8.4.13两配置各六表通过；5.1.42因已移除query_cache_size在连接阶段失败，完整保留该失败。

使用固定缓存镜像及自有随机库/容器，D盘保留数据目录。仅这个测试容器开启native_password，让旧驱动能够走到实际连接兼容性边界；生产认证/TLS不因此验收。就绪必须走TCP，避免命中入口初始化临时socket。无共享数据库、无TC调用、无业务提交能力。

`document_peer.py`运行40文档双向比较，包含14个原Java数字边界探针（7接受、7拒绝）；`prepared_peer.py`由自有MySQL现场驱动器调用，核对两配置各4条真实准备日志。`source_inventory.py`只列测试方法声明，不能当作JUnit运行或完整迁移；资源原字节与Windows checkout副本的纠正见[来源记录](../../docs/phase-2/java-resource-provenance-correction.json)。
