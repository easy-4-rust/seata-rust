# WP05 MySQL物理列约束与真实行读取

沿用[原WP05](wp05-mysql-at-plan.md)、[元数据基础](mysql-foundation-contract.md)及[行值模型](row-image-contract.md)。本增量补足数据模型与UPDATE/DELETE前镜像读取；不以只读成功代替业务执行、after image、Undo或完整Gate。

物理元数据在ColumnMeta中可选附加ColumnDomain，保留原八参数构造与旧内存模型行为。真实MySQL适配器必须提供域，真实行读取拒绝缺失域。域验证整数8/16/24/32/64位与unsigned，CHAR/VARCHAR字符数、字符集、定长属性，TEXT字节容量，BINARY定长补零后的长度与VARBINARY/BLOB字节容量，以及DECIMAL的unsigned。UTF8MB3不能接受补充平面字符，ASCII不能接受非ASCII；UTF8MB4不做Unicode规范化。物理域参与完整schema相等性，不进行数据库collation唯一性推断。

`before_image`只接受已封闭构造的正式SQL识别结果及完整原绑定参数，当前读取UPDATE/DELETE；INSERT镜像与业务执行保留后续步骤。参数总数必须完全一致，WHERE复用原参数起点后的切片，不能把SET参数移入WHERE。更新PK、未知赋值列均拒绝；不会在此执行原业务SQL。值使用SQLx原生i64/u64/文本/字节/NULL与精确BigDecimal绑定，十进制不能伪装成字符串绑定改变比较语义。

读取链共享一次绝对期限并暂时拥有原本地事务，失败/取消保持poison并释放连接。先取得MDL和真实元数据，再按完整PK索引序`ORDER BY`，同事务`FOR UPDATE`只读长度与空值标记，最多1025行。超过1024行、任意字段1MiB或整镜像8MiB payload立即拒绝。确认预算后以同一条件/参数/顺序/限制读取完整列并逐行核对实际长度。当前RR、受信MySQL服务器与同事务行/间隙锁是两次读取间稳定性的前提；这不是恶意数据库服务端网络包分配的硬上限声明。原SELECT没有业务副作用，受限表触发器/FK等仍按元数据矩阵拒绝。

真实类型解码保留i64MIN/u64MAX、65位DECIMAL/30位scale、二进制、UTF-8、NULL与空值；不可经过f64或截断。CHAR按MySQL读取语义保留逻辑值，BINARY读取含补零字节。条件错误、超限、超时或取消均不得发布部分镜像。空结果仍保留完整表元数据。

验证须包含真实物理边界、WHERE参数偏移、复合PK索引序、全列值、空结果、1025行/大字段/总预算拒绝、持锁阻止并发更改、取消后释放、独立SQL观察确认业务表未变，以及双Rust回归、目标断言削弱和只读审查。业务SQL/after image/Undo/提交未知与WP06仍保持原验收，不降低范围。

依据固定SQLx0.8.6本地源码与MySQL8.4官方[CHAR/VARCHAR](https://dev.mysql.com/doc/refman/8.4/en/char.html)、[字符串类型](https://dev.mysql.com/doc/refman/8.4/en/string-type-syntax.html)。依赖候选BigDecimal0.4.10仅从既有公开缓存按checksum复制到D盘；锁文件、Rust1.85和真实绑定验证通过前不声称正式可用。
