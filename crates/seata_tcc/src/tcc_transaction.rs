use crate::{BusinessActionContext, TccError};
use seata_datasource::FieldValue;
use std::{fmt::Debug, future::Future, pin::Pin};
/// 业务作用域内的有界参数化 DML 与可选显式报告能力，不暴露连接、提交、DDL 或控制表写入。
/// 对应 Java: FenceHandler 事务中传递给业务方法的本地事务；Rust 显式受限能力扩展。
pub trait TccTransaction: Debug + Send {
    /// 参数为当前可变业务上下文；确认单次报告后返回是否报告，不支持的事务明确拒绝。
    /// 报告失败或取消必须封闭支持此能力的事务；成功不表示业务事务已经提交。
    fn report_context<'a>(
        &'a mut self,
        _context: &'a mut BusinessActionContext,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async { Err(TccError::ContextReportUnavailable) })
    }
    /// 参数为目标后端接受的单条 DML 与完整参数；返回影响行数，失败或取消后作用域必须封闭。
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        parameters: &'a [FieldValue],
    ) -> Pin<Box<dyn Future<Output = Result<u64, TccError>> + Send + 'a>>;
}
