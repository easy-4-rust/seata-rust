# WP05 精确行值模型验证

[最终执行](run_20261114_042231_afaee4a5/result.json)验证Field/Row/TableRecords及DecimalValue/FieldValue的有界内存模型。契约为[行镜像数据模型](../../row-image-contract.md)，[机器状态](../../wp05-row-value-model.json)保持WP05为0/14。

固定7988输入、8148成员，catalog SHA256=`8e2ebc241143742e847a3821e7b0e06bf5435d1f8f83112d26ab94340956156c`。Rust1.99与最低1.85各新执行273工作区测试（新增14项行值测试），保存68个实际测试产物；既有RM文件恢复再次实际启动12子进程。10份D盘隔离削弱均编译并在目标断言失败，执行/归档源字节一致，分别保护十进制规范/容量、NULL、单字段预算、完整schema、PK序、原表名、累计字节/行数和Debug脱敏。格式、Clippy及严格rustdoc通过。

初始API RED是公开对象缺失的编译失败，不算行为断言。首轮042039双版本回归通过后，记录校验器误拒绝了已实际发生的自定义assert提示“accepted +1”；规则改为该目标的确切提示并保留实际panic位置、退出码及单目标失败要求，随后重跑完整验证。原失败记录保留，不能用它代替最终包。

65位DECIMAL/30小数位、i64MIN/u64MAX、任意二进制、NULL/空值分别保持精确；行必须是完整物理列序，PK按独立索引序返回。每字段1MiB、每镜像1024行/8MiB payload，可显式下调；添加失败不改变原行和计数。payload预算不是全部堆内存估算。

当前ColumnType只表示五类值域：尚未证明TINYINT/INT位宽、文本字符数或UNSIGNED DECIMAL回写合法性；连接SQLx解码和补偿前须补齐物理维度。没有数据库行读取、网络包预分配保护、before/after、锁键/排序规则、Java序列化互通或Undo完成声明。

171依赖归档/151个metadata候选依赖6806源再次逐tar核对；锁文件与[前一基础](../mysql-foundation/README.md)原字节一致。九个Java对应对象为PARTIAL，189测试源仅盘点、七资产原字节复制，未新执行Java具体案例或Go差分。[绑定审计](core-binding-audit-20261114.json)确认已有Rust源仅data-source两个声明/重导出文件变化，其余生产/测试源不变，MySQL驱动不变。前一轮两个服务器各8项真实元数据场景仍为历史证据，本轮未重新启动数据库。

WP05/06、生产认证/TLS/最小权限、WP00-R、原Go默认、完整V1、原生RustTC及其余Phase仍开放；外部TC风险未接受，共享锁暂缓。复现 `python tools/formal-row-image/run.py`，输出只写D/E盘。

[依赖范围复核](dependency-scope-audit-20261114.json)：旧151计数来自metadata候选图；两个版本实际compiler-artifact均为146个registry包。其余候选源码也已逐字节核验，但不能称为已编译活跃依赖。原封存包保持不变。
