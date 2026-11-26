# 正式注册池对固定JavaTC现场验证（2026-11-13）

最终现场运行：[run_20261113_202129_a18c4375](run_20261113_202129_a18c4375/result.json)，133来源/146归档成员/1实际正式Rust示例binary；catalog SHA256=`3c6bcb2027de70623779f9eaa942dae9b831fba1732fed5aa817f5780622224f`。

未修改JavaTC JAR：pin=`cba73e180fd8896a99df6415b859d275122feaa7`，SHA256=`aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`。认证扩展复用此前已测试的公共fixture artifact，class/service哈希逐项匹配历史记录；没有新Java编译或JUnit。原始SPI策略同时检查身份/凭据及资源白名单。

| 当前正式组件的现场用例 | 实际结果 |
|---|---|
| TM允许 | 101/102 identified=true，Ready |
| TM错误公共凭据 | identified=false，保持不可选 |
| RM两个缓存资源 | 首个资源登记同时建立身份，两个103/104均接受后Ready |
| RM代际轮换 | close原代际后全量重登两个资源，原句柄关闭、新代际不同，再Ready |
| 动态第三资源拒绝 | 旧ready立即失效，前两个接受、第三个拒绝，逐项报告及池保持not-ready |
| RM错误公共凭据 | 首资源及身份同一attempt被拒绝，不进入Ready |

JDK class+load日志实际证明5类来源：RegTmProcessor/RegRmProcessor/ChannelManager均从固定JAR内seata-core加载，AbstractCheckAuthHandler从该JAR classes加载，fixture checker从冻结的runtime extension目录加载。运行目录的实际class/service两文件也封存，不能只依赖启动参数或原缓存目录。父门禁122个Rust/Cargo编译输入与现场一致。

单个自有TC进程的38091监听PID经过核对；最终进程停止、端口释放且cleanup_errors为空。仅登记操作，FILE root.data保持0，没有创建事务/执行业务SQL或RM补偿。公共fixture凭据按预期可能出现于原Java INFO日志（JAVA-AUTH-EXTRADATA-001未关闭），不代表生产凭据/TLS/防重放策略。

先前[run_20261113_201818_a80e856a](run_20261113_201818_a80e856a/result.json)六用例功能也成功，但未记录实际类加载来源和runtime扩展文件快照；保留历史，并以上述增强来源闭包的新运行作为最终证据。两次执行覆盖同一六用例，不虚增为十二种新场景。

复现：`python tools/formal-registration/live.py`。启动并清理隔离D运行目录内的本地固定TC，不安装、不联网、不修改JAR/源码或原fixture。现场通过只支撑注册/就绪边界，不扩大为NET010–012、RM回调/Undo补偿、完整WP02/V1或生产认证。

后续正式transport回包实现后的[211350重跑](run_20261113_211350_6a0bee84/result.json)六用例通过，139来源/152成员/catalog=`8ac6fd9f4a2c2e1af44be92ea9f3ed9a1bc35479e73b0fd89815cacb8fe2487f`。5实际类来源及runtime SPI/清理继续验证，128共同编译输入与[WP02组合验收](../transport-recovery/README.md)一致；旧202129保留其122输入历史范围。
