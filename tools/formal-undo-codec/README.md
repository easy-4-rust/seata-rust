# 选定Undo编解码组件验证

执行 `python tools/formal-undo-codec/run.py`，运行锁定的Rust1.99/1.85工作区测试、六组独立MySQL测试、五项普通故障注入、真实Java二进制值差分负控及双向文档比较。所有输出在D/E盘，使用缓存镜像及独立随机库，不创建分支或业务提交入口。

验收范围和实际结果见[组件证据](../../docs/phase-2/evidence/undo-codec-gate/README.md)及[格式契约](../../docs/phase-2/undo-codec-document-contract.md)。`live.py`可单独运行当前真实四条准备日志的两种JDBC配置对照。

原Java18份选定源码、32class和冻结fallback/helper配置由既有原格式构建包提供；新增文档探针另编译固定SerialArray源码。依赖版本保持锁定，完整原Maven环境和Java JUnit测试集未重建。源方法声明库存、原资源字节、选定值探针分别记录，不把资产数当作测试案例数。

无原子Undo落库、TC分支登记或提交未知恢复证明。后续步骤继续遵守原WP05/06和全部后续计划；本组件通过不关闭这些门禁。
