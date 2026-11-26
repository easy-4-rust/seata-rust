# WP05 首批有界 SQL 识别验证

[最终023603](run_20261114_023603_0d037bc7/result.json)与[只读终审](readonly-review.json)验证当前MySQL AT识别子集；原[WP05十四项与Gate](../../../phase-0/v1-implementation-acceptance-checklist.md)仍未完成。[实施契约及矩阵](../../wp05-mysql-at-plan.md)延续原架构/ADR；这不是数据库提交或完整Java迁移验收。

2781输入、2927成员，catalog SHA256=`02382691b9dc5916063eba7147e5622769d9a70c2256c1a7c66fcc6eb39b3902`。Rust1.99/最低1.85分别新执行252项：既有74协议＋87传输＋28TM＋45RM，以及新增18SQL识别测试。保存56个实际执行测试产物、四组原身份的Commit/Rollback恢复记录与12个实际子进程。10份D盘隔离削弱实现均成功编译，并在指定测试产生目标断言失败；执行文件与归档原字节、记录SHA完全一致。格式、全target Clippy及严格rustdoc通过。

识别结果保留原SQL、表名大小写、有序列、VALUES行数、真实问号占位符数与WHERE参数起点。INSERT/UPDATE/DELETE按明确矩阵接受，DDL/事务控制、多语句、条件注释、跨schema/别名/连接、子查询/函数/变量和不支持修饰在执行前拒绝。字节、token、全部运算符与括号预算先于AST；AST表达式再检查深度及列/常量语义。错误与Debug不输出SQL字面值。

实际失败/修复证据区分如下：

| 记录 | 实际观察与修复 |
|---|---|
| 初始API RED | 缺公开对象的编译错误，不能计行为断言失败。 |
| 三边界 RED | IN列表越过常量矩阵；UPDATE FROM被解析器消费后不留AST；长运算链没有前置预算。三目标断言真实失败后修复。 |
| 未支持运算链 RED | `||`等未支持运算也会先形成深树，不能只计算白名单运算符；改为保守计算全部可能运算token。 |
| 裸内建项 RED | 固定MySQL解析器把部分CURRENT/UTC内建项当Identifier；显式拒绝未引用名字，反引号同名列仍可接受。 |
| 吞修饰词 RED | LOCAL/LIMIT ALL被消费而AST不保留；现对支持AST重建SQL再词法化，与原有效token逐项等价，不仅检查字段为None。 |
| 固定依赖panic RED | MySQL DIV缺失右值触发上游unwrap；首动词在AST前限制DML，未支持DIV提前拒绝。该RED是实际依赖panic，不冒称普通格式错误或断言负控。 |
| 首次完整回归023055 | 最低Rust版本旧TM测试固定20ms等待后关闭，偶发打断测试端分片登记ACK写入而报Windows10053。仅将该测试改为有界oneshot，等测试端ACK实际写完；不证明客户端Ready，生产TM未改。 |

最终已再次完整执行，不能以早期023055失败或023355旧文档字节包替代当前023603。10负控分别保护占位符计数、WHERE参数起点、重复列、VALUES宽度、IN常量、运算预算、AST/token保全、裸内建项、原始字节预算及Debug脱敏。没有独立完整SQL fuzz或真实数据库语法认证的声明。

本次新增`sqlparser=0.52.0`和`log=0.4.27`，从既有C盘公开缓存读取、按公开索引checksum核对后复制到D盘，原C缓存未修改；没有网络下载或CLI安装。24锁定依赖归档及20当前Windows/MSVC依赖的1894个实际源文件逐archive成员核验并前后冻结；其他四个仅归档验证，不声称其他平台已测试。

[源码库存](run_20261114_023603_0d037bc7/source-inventory.json)保留八个Java对应对象为PARTIAL，盘点固定Java `sqlparser/*/src/test`全部85份测试源文件，未展开具体参数/动态案例，也未执行Java或golden/live差分。该目录内非Java资产为0；嵌入Java的测试数据仍在对应原源文件快照里，不等于全项目没有资产或完整源测试已迁移。当前18测试属于Rust支持矩阵与替代依赖风险验证。

[核心绑定审计](core-binding-audit-20261114.json)确认既有四组件149个生产Rust源完全不变、31个测试源不变，只有上述一项TM测试夹具改动。根workspace只增加`seata_at`；原锁定26包条目完全保留，仅新增AT及两个依赖包。旧Java/Go Gate保持其历史执行范围，本次未重跑这些跨语言场景，252项Rust回归是新执行。

真实MySQL/SQLx、元数据/主键、镜像/锁、Undo格式与校验、业务/Undo原子提交、提交未知的围栏恢复仍待实施。WP05/06、原Go默认兼容、WP00-R、生产认证/TLS、完整V1、原生RustTC及后续Phase继续；外部TC风险未接受，共享锁按用户要求暂缓。没有启动新数据库或Java服务。

重复本轮：`python tools/formal-at-sql/run.py`，运行产物写入D/E盘；脚本绑定初始诊断包以保留旧TM测试实际失败来源。修改源码、依赖或矩阵后，受影响证据须重新取得。
