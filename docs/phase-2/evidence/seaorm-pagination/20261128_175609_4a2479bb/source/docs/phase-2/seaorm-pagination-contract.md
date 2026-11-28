# SeaORM 单表计数、分页与 LIKE

延续 WP05/SeaORM 连接规格中的未完成分页与更多谓词要求。新增独立密封 COUNT(*) 计划，只接收原生内建、无修饰的单项 COUNT(*)，可直接作用于一个已认证物理表，或作用于一层已完整验证的单表普通 SELECT 派生表。派生表的原 LIMIT/OFFSET 保留；没有分页的统计不得插入普通行读取的 1025 行探测上限，更不能把截断后的长度冒充总数。仅输出一行非空精确非负整数计数，保留 MySQL 原生 SIGNED BIGINT 的 SQLx 行与标签以兼容原 Paginator，不进行浮点转换。

LIKE/NOT LIKE 仅扩展读取谓词，包含绑定模式与单字符 ESCAPE；占位符顺序、MySQL NULL/排序规则与原 NO_BACKSLASH_ESCAPES 会话语义不改写。DML 原谓词准入不因此扩大。COUNT 参数、元数据、原 RR 事务、超时/取消/poison/close 边界与普通读取共用同一实现，不另开业务连接或提交。COUNT 客户端结果恒定有界；服务端扫描仍受原执行时限限制。

继续拒绝自定义/带 schema/带引号的 COUNT 名称、非 COUNT 函数、非星号参数、DISTINCT/FILTER/窗口/组内排序、组查询、集合运算、嵌套派生表、外层派生表过滤/分页，以及任何赋值、文件输出、锁定读、注释或多语句。通用聚合/关联/流式协议和完整 ORM 仍开放。

验收必须用真实 SeaORM count/Paginator 的 num_items/num_pages/num_items_and_pages/fetch_page/fetch/fetch_and_next/分页流，测试总行数超过 1025、最后页/越界页/空集、LIKE/NOT LIKE/中文与 ESCAPE、NULL、原派生表分页不被丢弃、RR 外部提交与本事务未提交写入可见性。双 Rust、原 Java TC 的真实实体提交/补偿须继续验证原 Undo/SHA/锁/回调；原生分页接口仍遵循已声明的行数/偏移预算，不宣称修改上游分页实现或认证所有输入。
