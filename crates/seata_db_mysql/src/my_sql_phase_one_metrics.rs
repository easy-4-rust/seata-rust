use crate::{my_sql_phase_one_outcome::OUTCOME_COUNT, MySqlPhaseOneOutcome};
/// 一阶段编排的逐字段计数快照，不是数据库或 TC 持久状态。
/// 对应 Java: 无直接对象；WP07 Rust 一阶段编排调用观测扩展。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MySqlPhaseOneMetrics {
    /// 当前已轮询、尚未返回结果的本阶段调用数。
    pub active: u64,
    pub(crate) outcomes: [u64; OUTCOME_COUNT],
}
impl MySqlPhaseOneMetrics {
    /// 参数为固定结果；返回本数据源累计次数，不适用或未观察到的结果为零。
    pub fn count(&self, outcome: MySqlPhaseOneOutcome) -> u64 {
        self.outcomes[outcome as usize]
    }
}
