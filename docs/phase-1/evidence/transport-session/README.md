# WP-02正式异步TCP单会话与请求关联（2026-11-13）

最终运行：[run_20261113_191253_8ba89fdc](run_20261113_191253_8ba89fdc/result.json)。catalog SHA256=`bde63f35709b2ef4cf9472d3a65a998424cc13623039815d7683ff9a92833fe3`；2546来源输入、2570归档成员、6个实际执行的测试binary。来源包含正式源码、锁文件、16个registry依赖archive及实际解包源码、规格和peer引用；archive校验与Cargo.lock checksum一致。后续README/任务状态更新不代表旧归档全部输入当前不变，执行代码和Cargo输入单独核对。

| 验证 | 实际结果 |
|---|---|
| Rust1.99与最低1.85 | 每套33项：9unit＋10流/ID integration＋14真实loopback TCP测试函数；全部通过 |
| 真实TCP | 拒绝连接、分段回复/同ID心跳隔离、掉响应后超时/迟到/重复、取消、断连、错误响应类型、并发逆序signed-wrap/quota、显式shutdown、容量拒绝、两个活动socket同wire ID、owner Drop、Failed+Committed字段原样保留、三种合并形状、本地无效请求不写入 |
| 完成仲裁 | queued expiry为本地、writer跳过已过期帧、已决定响应优先、按每个write marker关闭、写前伪响应拒绝、未poll的worker guard、poison终止清理、调用timer未poll时接收侧仍检查绝对deadline |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过 |
| 协议回归 | 新peer191025：74正式协议＋1验证器测试在1.99/1.85通过；628正向/2050负向、7Python/12实际输出破坏负控通过；引用catalog=`e89f562278c942a3de7af0575357a93e66045dd563b992d25ef62109cf2b5c87`，编译协议/Cargo输入与本次一致 |

API初始RED为缺公开类型的编译失败，不是九项运行断言失败。只读审查发现deadline在调用timer调度滞后时失效；新增真实运行断言RED1后，接收侧在同一仲裁锁内先检查绝对期限，针对性GREEN并两工具链全量通过。期限前已移交的响应仍保留。第二次只读审查确认修复有效，未发现新的阻断；它没有再次运行服务/测试。

[只读封存终审](readonly-review.json)独立核对2570成员/2546来源、6产物、16archive checksum，并逐tar成员对照1850原包文件与实际源码快照；包含生成标记的1866依赖源文件完整封存。新peer70个协议/Cargo输入与本轮相同，628/2050及12输出破坏负控离线重算通过。文档随后更新，不冒称全部历史README当前无漂移；当前92个正式编译/Cargo输入仍与封存一致。

使用现有本地缓存加入锁定Tokio1.54.1及其依赖；无CLI/Skill安装或网络下载。首个`run_20261113_191226_1d816118`仅保存全平台cargo metadata缺本地WASI archive的诊断，无result/catalog，不计成功。补齐现有只读C缓存至D后，完整离线门禁通过；开发和归档写入均在D/E。

NET-001–007的传输组件场景已有以上证据；实现同一socket内永不回收ID、各socket私有等待表和checked代际，未声称运行四十亿次发号。3/4心跳可接收/回复，但定时调度/idle、连接池、重连退避、认证/RM重新注册、业务回包分发、NET-008–012与WP-02完整Gate仍未完成。真实TCP故障peer不等同Java TC/数据库或业务恢复验证；本轮没有新Java执行、TC/MySQL服务或事务自动重放。原Go默认兼容/WP00-R/所有后续阶段保持开放，共享锁仍延期，外部TC风险未被接受。

复现：`python tools/transport-session/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_191025_d1a9ffd7`。需要固定D/E源码及现有缓存，工具创建独立运行目录并禁止联网；不修改已封存目录。
