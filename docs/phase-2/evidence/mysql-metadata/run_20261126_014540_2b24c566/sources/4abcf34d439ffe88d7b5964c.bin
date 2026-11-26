use std::sync::atomic::{AtomicBool, Ordering};

/// 已注册准备错误的单次失败报告门闩；取消等待后仍保留已尝试事实。
/// 对应 Java: AbstractResourceManager#branchReport；Rust 为未知响应增加本地一次性所有权。
#[derive(Debug)]
pub struct MySqlFailureReportGuard {
    started: AtomicBool,
}

impl MySqlFailureReportGuard {
    /// 无参数；由真实准备错误创建尚未发送报告的门闩。
    pub(crate) const fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
        }
    }

    /// 无参数；首次调用返回 true，后续调用返回 false；调用前须先核对回滚确认。
    #[cfg(any(feature = "experimental_changed_commit", test))]
    pub(crate) fn try_start(&self) -> bool {
        !self.started.swap(true, Ordering::AcqRel)
    }

    /// 无参数；返回报告是否已经进入可能发送阶段，不能据此推断 TC 已收到。
    pub fn started(&self) -> bool {
        self.started.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::MySqlFailureReportGuard;
    use std::sync::Arc;

    #[test]
    fn concurrent_report_attempts_grant_exactly_one_owner() {
        let guard = Arc::new(MySqlFailureReportGuard::new());
        let tasks: Vec<_> = (0..32)
            .map(|_| {
                let guard = Arc::clone(&guard);
                std::thread::spawn(move || guard.try_start())
            })
            .collect();
        assert_eq!(
            tasks
                .into_iter()
                .map(|task| task.join().unwrap_or(false))
                .filter(|started| *started)
                .count(),
            1
        );
        assert!(guard.started());
        assert!(!guard.try_start());
    }
}
