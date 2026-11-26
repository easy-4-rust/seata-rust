# 完整 SDK 与真实 MySQL AT 验证

[计划](../../docs/phase-0/go-java-v1-real-at-plan.md)使用已有0001–0006独立Go候选，通过原 `client.InitPath`、TM、RM和SQL代理执行真实MySQL操作。只操作带本次标签的专有容器及独立D盘数据，原始Java/Go和旧候选不修改。

已有JDK/Go、Docker及固定MySQL镜像缓存可用时，在仓库根目录执行：

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
gofmt -w tools/go-java-v1-real-at/main.go
python tools/go-java-v1-real-at/run.py
```

三场景分别验证：正常提交/补偿；阶段一持久提交后断连、实际RM重注册与重复回滚；原SDK进程在阶段一后退出、MySQL重启、新SDK使用原XID/BranchId/Undo补偿。第三场景TC保持运行，属于正常Begin/Registered的持久重放，不能替代重试超时16/17的恢复验收。

准备进程的受控EOF/reset独立记录，其他代理错误不允许通过；重启前/后的独立SQL须成功且非空，业务行、XID、BranchId、状态和完整Undo十六进制/哈希必须一致。真实103→104注册须先于回调，5→6及3→4按原连接/ID关联。重复回滚生成防悬挂记录是预期行为，不能要求Undo表永远为空。

Java overlay通过固定父清单→文件清单→10个class的哈希链校验，三次启动前后均核对，5个主类实际来源仍从JVM日志验证。16项Python控制覆盖来源、坏SQL、缺注册、错误关联和篡改class。Go254/Rust117/Java617明确继承前次认证，不冒充本轮重新执行。

Undo测试表根据固定Go `testdata/sql/undo_log.sql` 的协议字段构建，沿用原项目Apache-2.0义务；只在全新私有数据库创建。运行结束停止专有MySQL和TC，保留D盘数据供核对。

[归档证据](../../docs/phase-0/evidence/local-go-java-real-at/README.md)。[资源标识/凭据差异](../../docs/phase-0/go-at-resource-identity-divergence.md)仍开放；本轮不认证混合语言共享资源、完整SQL/故障原子性矩阵、TLS、生产Rust或完整WP-00。
