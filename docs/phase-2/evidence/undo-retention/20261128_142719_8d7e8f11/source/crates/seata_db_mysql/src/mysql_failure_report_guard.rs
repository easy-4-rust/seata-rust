use std::sync::atomic::{AtomicBool, Ordering};

/// 已注册准备错误的单次失败报告门闩；取消等待后仍保留已尝试事实。
/// 对应 Java: AbstractResourceManager#branchReport；Rust 为未知响应增加本地一次性所有权。
#[derive(Debug)]
pub struct MySqlFailureReportGuard {
    started: AtomicBool,
    #[cfg_attr(not(feature = "experimental_changed_commit"), allow(dead_code))]
    metrics: Option<crate::MySqlPhaseOneMetricsSampler>,
}

impl MySqlFailureReportGuard {
    /// 参数为真实来源的只读计数能力；测试无来源时传 None，不授予报告权限。
    pub(crate) fn new(metrics: Option<crate::MySqlPhaseOneMetricsSampler>) -> Self {
        Self {
            started: AtomicBool::new(false),
            metrics,
        }
    }

    /// 无参数；返回原数据源的计数能力，没有来源时不创建替代来源。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn metrics(&self) -> Option<&crate::MySqlPhaseOneMetricsSampler> {
        self.metrics.as_ref()
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
        let guard = Arc::new(MySqlFailureReportGuard::new(None));
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
