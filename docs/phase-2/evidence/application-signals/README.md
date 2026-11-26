# WP07 可选应用停机信号

沿用 [V1 验收清单](../../../phase-0/v1-implementation-acceptance-checklist.md)与[应用信号契约](../../v1-feature-matrix-and-recovery-guide.md)。默认无处理器；显式 application_signals 功能提供 ApplicationSignals::install / recv 和 ShutdownSignalKind。应用收到事件后关闭准入并有界排空，超时时保留管理器所有者及原作用域。接口不取消业务、不自动结束全局事务。

## 真实 Windows 事件

外部 Python 控制器把测试子进程放在独立隐藏控制台。只有子进程 PID 与已启动进程一致、原生 TM 的 Commit 帧已被受控对端接收、尚无完成标记时才发送 OS 事件。短生命周期辅助进程 AttachConsole 到该控制台，再 GenerateConsoleCtrlEvent；不广播到用户的控制台。原生 TCP 对端使用正式协议编解码器，持有原响应直到外部控制器放行。

每组检查：首次信号类别匹配，所有克隆拒绝新准入；排空返回 active=1，原业务任务未结束、TM 池未关闭；再次发送同种事件仍不强杀、不提前完成业务。放行响应之后，子进程消费第二次事件、原 Commit 成功、许可归还、排空完成、TM shutdown、对端读取 EOF。EOF 同时拒绝额外重放帧。每组独立进程退出码为 0，控制器核对完整状态字典后才写 verified.json。

- [Rust 1.99：Ctrl-C 与 Ctrl-Break 各发送两次](run-1.99-883b0616/verified.json)
- [Rust 1.85：Ctrl-C 与 Ctrl-Break 各发送两次](run-1.85-3bd5c52c/verified.json)
- [缺少公开 API 的首次失败](red.log)
- [错误类别映射的反例运行](wrong-kind-red.log)与[子进程实际失败断言](run-1.99-1a3c1d41/console_break/stderr.log)

类别映射反例只将 Windows ConsoleBreak 映射为 Interrupt，真实 Ctrl-Break 到达时断言 left=Interrupt / right=ConsoleBreak 失败。生产源码还原后再执行以上最终两工具链现场。较早单次事件记录保留原始状态，不用于证明重复信号验收。

## 构建与回归

Rust 1.99/1.85 各通过 47 项组件测试与 2 项编译文档示例。普通回归有 8 项 ignored：7 项已有外部环境用例及 1 项需要隔离控制台的新增用例。新增用例已由运行器以 --exact --ignored 显式运行，普通回归的 ignored 不作为现场通过证据。

- [1.99 回归](1.99-green.log) / [1.85 回归](1.85-green.log)
- [1.99 严格 Clippy](1.99-clippy.log) / [1.85 严格 Clippy](1.85-clippy.log)
- [1.99 默认功能构建](1.99-default-check.log) / [1.85 默认功能构建](1.85-default-check.log)
- [默认 Tokio 功能图](default-features.log) / [显式启用后的功能图](enabled-features.log)
- [格式检查](format.log)

命令：python -B tools/application-signals/run.py 1.99 或 1.85；cargo +1.99.0 / +1.85.0 test -p seata-tm --all-features --offline --locked；同配置 clippy --all-targets -- -D warnings；check --no-default-features --offline --locked。运行器只终止自己启动且尚未退出的子进程，实际完成的进程均已回收。缓存、临时目录、证据及构建均位于 D/E 盘。

Cargo.lock 新增 Tokio signal 所需的 Unix 目标依赖 signal-hook-registry 1.4.8 与 errno 0.3.15；既有版本未升级。Windows 实测不证明这些 Unix 代码已经运行。仅宿主 Windows 标准库可用，本轮没有安装其他目标或 CLI。

## 边界

Tokio 的处理器是进程级的，Drop 不保证还原 OS 默认处理方式，连续事件可能合并。第二次信号如何升级为强停必须由应用明确配置。本接口不接管 Windows 服务控制事件或系统关机期限。

此证据是受控原生 TCP 对端和真实 OS 事件，未把对端冒充原 Java TC，也未覆盖真实业务入口和部署。先前原 Java TC/MySQL 的 RM 停机窗口另见 [RM 验收](../rm-shutdown-policy/README.md)。Unix 现场、完整业务统一准入及实际部署验收继续，WP07 总项未勾选。
