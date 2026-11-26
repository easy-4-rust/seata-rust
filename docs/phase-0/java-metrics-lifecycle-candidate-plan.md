# Java 指标生命周期候选

针对 JAVA-METRICS-RETRY-001，延续 REC-TIMEOUT-008/009、ADR-0008 和已封存 FILE 自然16/17证据。当前是 Brownfield 增量，规格事实源仍为现有 Markdown/ADR；不初始化工具、不改变 WP00 前置或正式 Rust 规格。原 Java 源码/JAR和旧候选保持不变，新增独立0004补丁。

原 GlobalTransactionEvent Javadoc明确：未提交/回滚事件的endTime可以为null；MetricsPublisher.postSessionDoingEvent有意传null。DefaultCore重入16/17会发布此类doing事件，原subscriber却按终态先减active、增failure，再计算耗时而抛异常。不能通过修改producer、伪造结束时间或只掩盖Timer异常保留错误完成计数解决。

候选规则：Begin与三种Retrying仍按原逻辑记录进行中计数/重试Summary；其他已支持的完成状态若endTime为空，跳过该完成分派并写可核对DEBUG标记，不改事件。正常完成事件保持原consumer、metric名称、标签和计数规则；若beginTime缺失、时间倒序或差值溢出，仅跳过无效Timer样本，不丢已完成计数、不补零/当前时间。null registry/event与未知status应安全忽略。

先对原JAR运行实际CompactRegistry目标测试得到RED，再编译最小MetricsSubscriber候选并运行相同测试字节及受影响原回归。覆盖各完成状态/重试标志、合法doing事件、空时间、倒序/溢出、零时长和有效耗时；检查真实计数及Timer样本，不只看无异常。

真实FILE自然16业务链路继续使用原AT/TCC业务和两次journal恢复：正向观察原doing事件确有null endTime，候选分派被实际执行，初始自然超时的失败指标正常、人工重入不伪增失败或扣active、最后真实完成指标正常；同时完整复核SQL/Undo、原分支/上下文、真实成功回复丢失和业务幂等。指标读取不得改变事务或重置采样窗口；实际事件/指标、调用类来源、原始帧与完整性/清理均归档。

这不是所有重启指标重建、去重、Exporter或全存储/并发矩阵验收。若发现其他计数/持久化语义问题，单独记录，不以本候选关闭全部metrics/WP00。未打补丁参考问题与正式Rust仍按实际状态开放。
