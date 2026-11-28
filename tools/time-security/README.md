# time RFC 2822 源码安全回移

运行 `python -B tools/formal-source-junit/native_time_security.py`。

该独立 workspace 同时验证维护源码和真实栈边界；运行器为原发布版建立独立对照，六个公共入口各在独立小栈子进程中复现崩溃，维护版使用相同输入返回错误。原 `attack_worker` 被普通测试忽略，但由运行器逐个显式执行，不能把 ignored 当作已验证。

两版功能测试、工作区回归、rbatis 兼容测试、精确源码/归档摘要和原始审计报告均保留到现场证据。维护标识 `0.3.45+seata.1` 不是官方修复版；未修改的上游锁文件另做审计并保留原告警。维护版 path 依赖的扫描结果不能单独证明安全。源码修复不等于全部生产安全门禁完成。详见 [契约](../../docs/phase-2/time-rfc2822-security-contract.md)。
