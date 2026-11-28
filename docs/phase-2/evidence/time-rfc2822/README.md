# time RFC 2822 源码安全回移

权威现场：[20261128_111842_d1ec46e6](20261128_111842_d1ec46e6/report.json)，对应 [修复契约](../../time-rfc2822-security-contract.md)。

原发布版 time 0.3.45 的六个公共解析入口，在独立 256 KiB 栈进程中对十万层嵌套注释全部实际栈溢出；原 31/32 边界测试也失败。维护版本 `0.3.45+seata.1` 仅改变维护标识和共用 RFC 2822 模块，保留 Rust 1.85，三个递归函数和 32 层常量与官方 0.3.47 修复逐一一致。

Rust 1.85.0/1.99.0 各八项功能测试及严格 Clippy 通过，十二个显式小栈工作进程对同一输入正常返回解析错误。31 层仍接受，32 层拒绝；普通 RFC 值、时区、折行、长串浅注释、转义括号和 serde/option 通过。普通测试中被 ignored 的攻击 worker 已由运行器逐一执行。

双版本工作区各 **853 passed / 419 ignored**，rbatis 各八项原生兼容测试和严格 Clippy 通过。MySQL 正常依赖图没有因补丁引入 time/ORM。1418 个执行输入的起止摘要一致，96 个原始证据成员的 catalog SHA-256：`b29fd24530ff60bbaa95c06686625432b48854cdb19d089e9ae466cf1c83a25b`。

安全审计不设置 ignore：原上游锁文件返回 1 并保留 RUSTSEC-2026-0009；维护依赖所在根工作区返回 1（libsqlite3-sys 原发现），rbatis 独立锁返回 0，但仍保留 lru 0.14.0 的 RUSTSEC-2026-0002/2026-0253 unsound 警告。工具未对维护版 path time 返回原告警，退出码 0 不作为源码安全证明；本修复由原崩溃对照、官方算法和实际有界行为独立证明。完整生产安全及下游发布补丁集成仍开放。

复验：`python -B tools/formal-source-junit/validate_time_security.py docs/phase-2/evidence/time-rfc2822/20261128_111842_d1ec46e6`。重跑使用 `native_time_security.py`。rbatis 正式事务执行器、真实 MySQL/原 Java TC 联试和完整计划尚未完成。
