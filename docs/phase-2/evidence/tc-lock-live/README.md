# 原TC真实锁记录与登记观察

本步把[原锁键组件](../lock-key-gate/README.md)接到未修改固定原Java TC的数据库后端，记录已观察的排他锁覆盖问题。**观察包通过完整性和交叉核对；TC安全、正式登记整体门槛、SQL/Undo原子性与公开COMMIT仍未通过。**

| 案例与真实MySQL输入 | owner原键 | owner实际锁行 | contender结果 |
|---|---|---|---|
| 同一文本主键`1`，正常争锁控制 | `t:1` | `t/1` | query=false，明确拒绝，异常码2 |
| 相同空串主键 | `t:` | 无 | query=true，两分支均登记成功 |
| 相同ASCII空格主键 | `t: ` | 无 | query=true，两分支均登记成功 |
| 相同U+2003主键 | `t:U+2003` | 无 | query=true，两分支均登记成功 |
| 同一INSERT主键为空串及`1`，contender争空串 | `t:,1` | 只有`t/1` | 空串query=true，登记成功 |
| 同一INSERT主键为`x;broken`及`z`，contender争`z` | `t:x;broken,z` | 只有`t/x` | `z`query=true，登记成功 |
| 相同二进制`00ff` | `t_binary:[0, -1]` | `[0`与` -1]`两片段 | query=false，明确拒绝 |
| owner=`00ff`、contender=`00fe` | `[0, -1]`与`[0, -2]` | owner两片段 | 不同主键共享`[0`，query=false，额外冲突 |

分号案例已连接实际MySQL原主键与两行真实镜像，明确看到`z`未被owner持锁。二进制案例证明当前范围的额外冲突，不能据此推断所有二进制身份均失去互斥。没有改写原协议锁键，也没有把这些问题转为用户已接受的限制。

实际证据：

- [来源方法15例](../tc-lock-source/run_20261116_013725_d46fee3c/result.json)：catalog `ae17ddd3e0c05d1ea0162de5533e707f6a0b3b121644ce11b3fef06bbcf46544`，307成员、633运行输入、285原server fallback；只证明真实原方法与录制后端。
- [真实TC网络/存储8例](run_20261116_020229_df8ce4db/result.json)、[逐例结果](run_20261116_020229_df8ce4db/verified.json)：catalog `58b48297821a70c461f6bca502a6597c9f83c5f8c4cd713b8b40b395c31eede2`，11931输入、12016成员。Rust1.99，16个原XID、16次Begin、16次登记、8次锁查询、16次全局Rollback、13个实际回调与13次记录资源效果。25个真实DB确认点，原global/branch/lock及准备业务/Undo最终均为0；自有进程和精确容器已清理，D盘数据保留。
- [实际证据16项负控制](../tc-lock-controls/run_20261116_020525_d6b970d3/result.json)：catalog `86d8b58b6a94933a1dd9dcf3e8f413aec8d8b85a5ba009a330000bde11c05258`，实际副本中伪造PK值/类型/顺序、物理锁行、身份、状态、清理与完整请求对顺序均命中目标拒绝。
- [同源Rust1.85客户端构建](../tc-lock-msrv-client/run_20261116_020646_d344d84a/result.json)：catalog `08e6db586718e2c41b0546351f5077085e0af58f71e00a4c4201b9f6bfd156d9`。相同11931编译输入在前后逐SHA一致，实际MSRV构建和2项夹具测试通过；不记为再次执行8个网络场景。
- 共用RuntimeBinding增加tc_lock family后，新执行[原锁方法1029 MATCH](../lock-key-peer/run_20261116_020428_034cfb32/result.json)，catalog `ec9d361e0a0417e2bc99ca21bb16f12b3e47316a1ab95fb08def14c9f8dbfa9c`；[文档40/40双向MATCH](../undo-document-peer/run_20261116_020428_8d5b0ba4/result.json)，catalog `8fbbc99f5a66b07534d8c11d2252f84b10d8d1df6f95d0210be8903706f9f413`。

相对前一锁键组件，280个crate源/测试/配置输入逐字节未变；本轮没有重跑原325项普通回归或旧38个其他MySQL场景，沿用其证据。新客户端通过fmt、clippy及2项资源夹具测试。真实SQL准备与登记为分离的验证步骤，业务数据全部回滚；模拟资源只记录回调，没有生产SQL补偿。Java pin/JAR、完整载入类来源、外部JDBC8.0.33、registry归档与解压源码封存于观察包。

依赖保持185个归档候选与150个实际编译包；候选的归档checksum、解包成员字节与集合均已逐项核对，5478个非JDK文件类加载来源绑定到原server JAR、外部JDBC或认证SPI。最终只读审查无剩余P1/P2，相关证据与限定范围记录在[审查结论](readonly-review-20261116.json)。

现场配置范围：固定缓存MySQL8.4.13、JDK17、非TLS本机测试、文本utf8mb4_0900_as_cs，TC的原清理阈值设置1000ms。默认70秒阈值下先前20秒等待失败的批次保留，不记作通过；不把本轮扩大为默认时序/TLS/全存储或全原JUnit验收。另一次utf8mb4_0900_bin准备读取返回`text image`错误，保留为后续文本解码覆盖问题；未用改变夹具排除它的未完成状态。

下一步继续既有WP05/06：记录原TC覆盖候选及可支持身份策略，补齐原源测试案例展开，推进同连接Undo原子持久化、真实登记、持久围栏与故障恢复。修改已批准核心验收前需单独确认；当前未修改这些验收标准。共享锁继续暂缓，全部后续Phase保持原范围。
