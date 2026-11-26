# WP05 MySQL 元数据与本地事务基础

[最终父记录](run_20261114_035455_0dec6b55/result.json)、[只读终核](readonly-review.json)及[部分状态](../../wp05-mysql-foundation.json)验证正式 `seata_datasource`、`seata_db_mysql` 的基础实现。延续[实施契约](../../mysql-foundation-contract.md)，不关闭原 WP05 十四项或 Gate。

父记录冻结7979输入、8126成员，catalog SHA256=`151350d6ba34f40d03a30521b313a77345ae4fe2f890fb09272854a8423188fc`。Rust1.99与最低1.85分别新执行259项工作区测试，其中新增7项元数据/配置测试；保存66个工作区测试产物。四份D盘隔离实现削弱均编译成功并在目标断言失败，执行文件与归档字节一致。既有RM文件恢复四组12子进程再次通过；这些不是数据库崩溃恢复测试。

两个独立自有MySQL容器分别运行八项实际数据库测试，各保存独立的live测试产物（不与工作区产物混称同一二进制）：

| Rust | 实际子记录 | catalog SHA256 |
|---|---|---|
| 1.99 | [035724](../mysql-metadata/run_20261114_035724_bb191917/result.json)，249成员 | `96a3d3b467fadb116fe46b6f09b48f6b5081d40a43b26586e86c5475a43a7735` |
| 1.85 | [035750](../mysql-metadata/run_20261114_035750_67aa35c4/result.json)，249成员 | `1ba756d739100970970da25cc35b4c332338aee802cfc39b1a9b9ca20d91673e` |

现场服务器均为8.4.13，`lower_case_table_names=2`。验证物理列顺序与独立复合PK顺序、整型/DECIMAL/二进制/NULL元数据、不安全表拒绝、MDL阻止真实DDL、显式回滚及Drop后DDL成功、TRIGGER不足/部分撤权/PROCESS不足拒绝。两次真实代理均扣留服务器43字节，完整元数据操作在客户端预算内返回Deadline。业务行数与整数边界聚合前后一致、触发器效果为零；剩余数据库会话为零，自有容器已停止删除，端口无监听。

封存父字段 `actual_MySQL_processes=2` 指两组自有容器中的受测服务器实例；官方入口还有初始化服务等过程，不能解释为所有 mysqld 启动次数。发布状态使用 `actual_owned_MySQL_containers=2`，原封存结果保持原字节及执行时pending-review状态。

本地事务在进入SQLx拥有式Transaction前标记close_on_drop；当前每次使用后丢弃物理连接。table_meta在首次await前移交事务所有权并设置poison，取消时丢弃事务；正常错误只允许回滚。客户端connect、begin、完整元数据链、rollback各使用单一绝对期限。源所有权审查不代替尚未执行的调用方取消现场案例，也没有严格实时精度或吞吐认证声明。

真实RED须区分：032614响应扣留产生外层4秒断言失败；033108部分撤权错误返回带触发器的成功元数据，属于实际行为缺陷。031738缺UPDATE授权、032128文本元数据解码及033108另一数值解码失败保留为各自诊断，前两者不算部分撤权行为RED。父tdd目录收录日志/结果/输入清单，原失败包保留对应源码快照；不声称父tdd目录独立包含这些旧快照。

锁文件178包：171份registry归档均与checksum匹配，metadata列出的151个Windows/MSVC候选依赖的6806实际源文件逐tar成员核验；其余20仅归档身份验证。SQLx固定0.8.6，公开缓存只读后复制D盘，无网络下载或安装。六个Java对象为PARTIAL，189份原测试源仅盘点、七份资产按原字节复制；未执行Java具体案例差分或新的Java/Go Gate。

当前权限方案需要直接TRIGGER与全局PROCESS，并拒绝部分撤权；尚非生产最小权限方案。TLS默认VerifyIdentity，但现场显式使用loopback禁用TLS，生产TLS未验收。大小写其他模式、特殊字典名称、精确行值解码、业务SQL、镜像、Undo及提交未知恢复仍待验证。WP05/06、WP00-R、原Go默认、生产认证、完整V1、原生RustTC及其余Phase继续；外部TC风险未接受，共享锁按用户要求暂缓。

复现：`python tools/formal-mysql-foundation/run.py`。输出只写D/E盘；修改源码或依赖后须重取受影响证据。

[依赖范围复核](dependency-scope-audit-20261114.json)：旧151计数来自metadata候选图；两个版本实际compiler-artifact均为146个registry包。其余候选源码也已逐字节核验，但不能称为已编译活跃依赖。原封存包保持不变。
