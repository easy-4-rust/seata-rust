# 正式连接层首批：流解码与连接内编号唯一性

[seata-transport](../../../../crates/seata_transport/README.md)已加入正式workspace，实现增量FrameDecoder、RequestIdAllocator及类型化基础错误。只完成WP-02的流解码和编号分配两项实施任务，不是完整TCP连接或NET-001–012验收。

正式运行：[run_20261113_182417_a9c1efb9/result.json](run_20261113_182417_a9c1efb9/result.json)，[741成员清单](run_20261113_182417_a9c1efb9/catalog.json) SHA-256 `ab4ff9fd23accc104dc0ec2962439a8e58ffbcc6fe620f1b734c41bef1902f18`。718输入与5实际测试binary冻结并前后核对；终审独立验证SHA与测试记录，无阻断。

| 实际验证 | 结果及边界 |
|---|---|
| 11项transport测试 | 10个集成测试函数＋1个有效完整ID空间尾部状态单测；Rust1.99/最低1.85分别通过，不重复计为22项独立功能测试 |
| 46条Java认证帧 | 九控制帧所有二分/单字节＋37完整帧的固定头/头尾/帧尾边界分段和全部单字节；结果与已独立认证的完整协议codec一致，不新增Java执行 |
| 连续帧/内存预算 | 1000个16字节帧的合并输入超过单帧预算仍逐条交付，单次只返回一帧且保留外部suffix；恶意长度/能力在固定头保留容量<=16下拒绝；有效输入容量不超过配置预算 |
| EOF及错误 | 空片段不等于EOF；半头/半body EOF返回类型化错误；EOF/协议错误永久终止，不能将旧残片接入新连接；好帧先交付，后续坏帧再终止 |
| 共享协议语法 | 正式ProtocolDecoderV1.frame_length逐字段检查已有前缀；完整API复用同一头规则仍拒绝尾随，前缀API只看固定头、忽略后续连续数据 |
| 编号 | 全signed-i32位模式包括负ID/0；单连接不回收已发编号、quota1..2^32、耗尽只要求换连接；不实现Clone或reset。最后两次发号用有效历史状态初始化，不声称实际运行四十亿次 |
| 协议新回归 | [新peer181728](../protocol-peer-profile/run_20261113_181728_849d40a0/result.json)对当前共享解析实现新执行74协议＋1验证器/MSRV、628正向/2050负向、7Python与12实际输出负控；本脚本只读引用，不再次执行Go/Java |
| 绑定与质量 | 69个实际协议编译/Cargo输入与新peer运行相同，1915子成员哈希完整；本批workspace fmt/Clippy全target/rustdoc拒绝warning；原独立probe117及fmt/Clippy新跑通过 |

子peer catalog SHA-256=`0894066182ecae3458b9804a43318f01031b3cb4ffc7d0422c1707e2c0ff32a4`，1848源输入。其README在执行后补充了新前缀API说明，历史源快照仍完整；这里只对69个编译输入作当前一致性声明，不声称所有旧输入当前均不变。

初始API RED是缺少公共连接/前缀API的编译失败，不是11个运行期测试失败。最早peer181302因原验证器把workspace测试数固定为74，在加入11个新transport测试后触发计数门禁；已将该工具测试/质量范围限定seata-protocol，并冻结所有workspace成员Cargo manifest。该诊断不是协议实现失败，完整workspace质量仍由本批单独执行。

TCP连接/池、pending futures、写入尝试与未知结果、超时/晚到/重复、代际标签、重连/RM重注册、心跳调度与关闭所有等待请求继续未完成。编号器数学唯一性不等于完整NET-007或跨连接关联通过；WP-02 Gate保持开放。原Go默认差异、完整WP00-R、TC风险与共享锁延期不变，没有安装依赖、启动服务或创建分支。
