# seata-at — AT引擎（实施中）

当前已实现有界MySQL SQL识别：`SqlRecognizerFactory::recognize`完整验证单条INSERT/UPDATE/DELETE，并返回原SQL、原大小写表名、有序列、VALUES行数、实际占位符数和WHERE参数起点。识别结果不是数据库已执行或表已安全的证明。

支持与拒绝边界见[WP05实施契约](../../docs/phase-2/wp05-mysql-at-plan.md)。字节/token/运算符/括号预算先于AST；表达式再做深度及语义检查。完整AST重建token比对拒绝上游吞词。MySQL裸内建项、条件注释、跨表/别名/schema、子查询/函数/变量、DDL/事务控制及未支持修饰明确拒绝；未引用DIV在进入固定解析器的内部panic路径前拒绝。错误与Debug不输出原SQL、字面值或凭据。

首批18测试覆盖绑定与矩阵，以及上游误分类、吞词和异常路径；[执行证据](../../docs/phase-2/evidence/at-sql/README.md)分别记录Rust1.99/最低1.85、全工作区回归与隔离削弱断言。源码一对象一文件，Java来源逐一标注；八个相关Java对象仅部分实现，85份源测试文件尚未完成逐案例迁移与差分，不能称为完整Java模块迁移。

真实MySQL、SQLx驱动、元数据/主键、前后镜像、Undo序列化及本地事务尚未接入，WP05十四项/Gate与WP06继续开放。现有文件资源恢复不证明SQL/Undo原子性，原生RustTC与其余Phase仍在完整计划中。

本地验证：`cargo test --locked --offline -p seata-at --all-targets`。
