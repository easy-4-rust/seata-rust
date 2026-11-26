# 独立只读审查

审查者 ddl_snapshot_review，仅读取生产/测试源码，未修改文件或访问数据库。未发现阻断项。

原 FOR SHARE 先打开实际对象并取得 MDL；当前 SHOW CREATE 引擎检查处于同一事务与 MDL 内，位于 registration_started=true 和 BranchRegister 调用之前。MyISAM 值读取没有写副作用，失败会关闭被消费的原事务；外层 timeout_at 也涵盖检查，取消不返回可复用作用域。InnoDB 的原共享行锁仍持续至准备作用域结束。

迟到视图即使被旧字典误认作 InnoDB 基表，当前建表定义也没有 canonical ENGINE 行，会拒绝。未增加连接或修改认证/TLS。新测试由严格管理器检查未要求绑定的原事务，覆盖任一端要求绑定即启用的既有策略；验证无登记/回执、非未知，业务/Undo/恢复记录零，末尾清理视图。

完整绑定表列与约束规范不在本局部修复声明内，稳定单主及统一资源 ID 部署要求保持。
