# WP02流解码/编号基础验证

先运行 `python tools/formal-protocol-peer/run.py`，再将其成功输出路径传入：

```text
python tools/transport-foundation/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/<成功运行目录>
```

工具完整核对子peer封存，并确认当前协议Rust源码/Cargo输入与实际已执行的回归相同，再冻结本批全部workspace源码/夹具/规范。分别运行Rust1.99/最低1.85的11项transport测试、workspace格式/Clippy/doc、原独立probe117及其格式/Clippy，归档实际测试产物。只引用子运行的628/2050比较，不重复宣称本脚本执行了Go/Java。

本脚本认证首批流/编号检查点；扩展transport测试后应形成新的验证检查点，不改写旧封存。不会启动TC/DB、安装工具、切换分支或替代完整WP02/NET矩阵。已存在环境与所有构建/输出均使用D:/E:。
