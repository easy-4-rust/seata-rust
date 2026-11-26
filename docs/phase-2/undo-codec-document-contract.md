# WP05 Jackson线格式文档编解码

延续[原格式契约](mysql-undo-format-contract.md)，不改变既有内存事务模型或开放提交。新增JacksonUndoLogParser及只读UndoLogDocument，是原JSON对象图的Rust文档投影，不是“全部Java Bean已迁移”的声明。

## 必须保留的差异

原始分支/SQL/镜像/字段可以有null、缺失默认字段、null集合及null集合元素。只要源类型允许，不能为通过当前SqlUndoLog事务构造器而抹去这些值。原`{}`只在原两字节输入时按getDefaultContent语义得到缺省BranchUndoLog；JSON null表示原parser返回null。缺省分支再encode时按原Bean输出完整类型标签/缺省字段，不伪称重新编码仍是`{}`。

保留Java类型标签、集合类型标签、字段JDBC码、TableRecords/EmptyTableRecords区别、完整数值和二进制。只解释声明的类名，不实例化任意Java类。原SQLType词汇可以进入文档，不表示业务SQL执行器因此支持DDL/SELECT等语句。

## 边界与确定性

解码在构造树前检查总字节及深度；对象不能丢失重复键，包括Unicode转义后的同名键。数值用原精确JSON词法/整数保持，不能经f64解析整数或DECIMAL；保留科学计数法及scale表达。拒绝尾随内容和未知类型标签，错误/Debug不输出原值。

BigDecimal声明值还必须落在原Java字符串构造器可接受的指数及32位scale内；不能因Rust任意精度JSON保留了词法就接受原parser拒绝的数值。真实原parser正反探针及Rust目标失败/修复记录应一并保留。

固定Jackson2.18.3 profile的数字读取上限为1000个数字，尾数和指数合计，符号及小数点不计入；自然数值和声明数值均受约束。原Java的1000/1001边界正反探针应保留，不把输入总字节或scalar上限当成该数字限制。

缺省上限：编码输入/输出32MiB，节点262144，嵌套32，单scalar1398104字节（容纳1MiB二进制的base64），累计scalar24MiB，实际字段文本/解码二进制1MiB。各上限同时生效，并允许配置更小的值，不允许越过硬上限。该结构/字节预算仍不是精确进程heap字节承诺。

嵌套深度统一按对象/数组容器从1起计，scalar不额外增加容器层数；空容器也占一层。准备日志的必要条件节点预检不得超过实际树的节点数；同一精确预算应能编码并解码，少一个节点/一层则拒绝。

输出具有固定JSON对象键序和保留的数组顺序；JSON键序及Unicode转义拼写不是Java案例的值比较目标。原生成资产继续原字节保存，不修改“对齐”资产。必须由原Java实际解码Rust输出，核对字段类型、值和容器；只做Rust roundtrip不能宣称互通。

## 事务内模型接入

编码真实准备日志需要明确JDBC配置，不推断默认值。原生MySQL类型身份和unsigned决定JDBC码/Java包装类；默认signed tinyint(1)仅对物理0/1允许无损Boolean映射，其他原整数不得静默丢失，可显式使用已对照的numeric配置。解码后的原对象图须在同事务MDL元数据下另行验证再用于恢复，不能仅凭日志声称schema/PK可信。

源案例声明库存当前为189文件/1135声明方法，具体参数/继承/动态案例尚未全部展开，状态保持MISSING/未差分。Java驱动5.1.42在当前MySQL8.4的两配置连接失败继续保留。完整codec源案例、同连接Undo写入、分支注册、持久围栏、提交未知及所有后续计划继续。
