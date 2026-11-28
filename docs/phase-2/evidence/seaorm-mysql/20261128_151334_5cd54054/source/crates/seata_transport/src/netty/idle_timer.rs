use crate::TransportError;
use std::time::Duration;
use tokio::time::Instant;

/// 每个读/写worker独占的单调空闲计时器，无后台任务、追赶tick或共享锁。
/// 对应 Java: io.netty.handler.timeout.IdleStateHandler 的单方向空闲计时职责。
pub(crate) struct IdleTimer {
    interval: Option<Duration>,
    last_activity: Instant,
}
impl IdleTimer {
    /// 参数是可选方向间隔；None关闭计时，初始活动时点是当前单调时间。
    pub(crate) fn new(interval: Option<Duration>) -> Self {
        Self {
            interval,
            last_activity: Instant::now(),
        }
    }
    fn deadline(&self) -> Result<Option<Instant>, TransportError> {
        self.interval
            .map(|duration| {
                self.last_activity.checked_add(duration).ok_or(
                    TransportError::InvalidConfiguration {
                        reason: "idle deadline overflow",
                    },
                )
            })
            .transpose()
    }
    /// 返回当前活动期限是否届满；没有开启时始终为false，溢出返回类型化原因。
    pub(crate) fn expired(&self) -> Result<bool, TransportError> {
        Ok(self
            .deadline()?
            .is_some_and(|deadline| deadline <= Instant::now()))
    }
    /// 等待一个绝对空闲期限；None永不就绪，取消本future不改变计时状态。
    pub(crate) async fn wait(&self) -> Result<(), TransportError> {
        match self.deadline()? {
            Some(deadline) => {
                tokio::time::sleep_until(deadline).await;
                Ok(())
            }
            None => std::future::pending().await,
        }
    }
    /// 完整有效帧处理/写入后记录当前活动；从当前时间重新计时，不追赶漏掉的窗口。
    pub(crate) fn reset(&mut self) {
        self.last_activity = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::IdleTimer;
    use std::time::Duration;
    #[tokio::test(start_paused = true)]
    async fn absolute_boundary_is_inclusive_and_reset_moves_it_forward() {
        let mut timer = IdleTimer::new(Some(Duration::from_millis(10)));
        tokio::time::advance(Duration::from_millis(9)).await;
        assert_eq!(timer.expired(), Ok(false));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(timer.expired(), Ok(true));
        timer.reset();
        assert_eq!(timer.expired(), Ok(false));
        assert_eq!(timer.wait().await, Ok(()));
        assert_eq!(timer.expired(), Ok(true));
    }
    #[tokio::test(start_paused = true)]
    async fn complete_activity_suppresses_probe_across_many_original_windows() {
        let mut timer = IdleTimer::new(Some(Duration::from_millis(10)));
        for _ in 0..20 {
            tokio::time::advance(Duration::from_millis(8)).await;
            assert_eq!(timer.expired(), Ok(false));
            timer.reset();
        }
        tokio::time::advance(Duration::from_millis(10)).await;
        assert_eq!(timer.expired(), Ok(true));
    }
    #[tokio::test(start_paused = true)]
    async fn disabled_timer_never_fires_or_expires() {
        let timer = IdleTimer::new(None);
        assert!(
            tokio::time::timeout(Duration::from_secs(86400), timer.wait())
                .await
                .is_err()
        );
        assert_eq!(timer.expired(), Ok(false));
    }
    #[tokio::test(start_paused = true)]
    async fn delayed_scheduler_resets_from_now_without_catch_up_burst() {
        let mut timer = IdleTimer::new(Some(Duration::from_millis(10)));
        tokio::time::advance(Duration::from_secs(100)).await;
        assert_eq!(timer.expired(), Ok(true));
        assert_eq!(timer.wait().await, Ok(()));
        timer.reset();
        assert_eq!(timer.expired(), Ok(false));
        assert!(tokio::time::timeout(Duration::from_millis(9), timer.wait())
            .await
            .is_err());
    }
}
