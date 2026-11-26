# Undo编解码后续实施记录

本文件仅整理[正式格式契约](mysql-undo-format-contract.md)之后的实施依赖和待验证问题，不改变原验收标准。[选定Rust编解码组件](evidence/undo-codec-gate/README.md)现已通过40双向文档、实际四日志双JDBC配置对照及最终双Rust316回归；其他38个数据库场景明确继承首次整轮。完整源案例与Undo落库尚未完成。

1. 展开原BranchUndoLog、SQLUndoLog、Field/Row/TableRecords和parser的源测试方法、参数/继承/动态具体案例，继续保留MISSING/未差分状态；189文件库存不能代替案例分母。
2. 区分原可空/可变的线格式对象与已校验的事务内模型。现有SqlUndoLog构造会拒绝INSERT两侧皆空，原parser却能编码这种DTO（保留undo_empty生成样本）。不应为通过Rust构造器而删除原资产或削弱Java样本；先忠实解析，再单独验证能否安全用于恢复。
3. 原默认内容是原两字节`{}`，与常规分支空SQL列表不同；空镜像还有TableRecords$EmptyTableRecords标签。元数据transient，解码后须用同事务MDL下的真实元数据绑定，不能伪造或从未知旧缓存推断。
4. 明确保留默认signed tinyint(1)的Boolean线值与numeric配置的Integer值。实际物理2与默认Boolean true不是可逆关系；在配置/模型明确之前，禁止静默把原整数降为bool或把bool恢复成任意整数。无符号tinyint实际声明不保留display1，两个已测配置都为Integer。
5. 已实现声明类/集合形式、精确数值词法、原可空对象图与有界输出。默认Bean展开预算、BigDecimal指数/scale、空容器深度及数字读取1000位限制已有实际失败/修复证据；40文档双向、实际四准备日志双配置已比较。完整源测试、未覆盖时间/数组类别和所有配置仍需继续，不能据选定probe宣称全codec完成。
6. 检查输出编码长度及累计结构预算。现有8MiB镜像/16MiB准备日志是字段payload预算，并非进程堆内存保证；NULL密集行、完整元数据克隆和日志累计带来的结构开销尚需专门验证，不能据payload通过推断内存上界。
7. codec之后才在同连接/同事务写入原undo_log及上下文/唯一身份。原BranchRegister、提交未知的持久围栏与真实竞争/断连恢复通过前，仍不开放COMMIT，不自动重放业务SQL。

5.1.42在本MySQL8.4配置的两个连接失败必须继续保留；若扩展旧驱动/旧MySQL配对，需要独立固定运行配置，不得修改原驱动或把8.0.27通过当成它通过。完整Java源案例、生产TLS/权限、WP00-R与所有后续计划继续，外部TC风险未接受，共享锁暂缓。
