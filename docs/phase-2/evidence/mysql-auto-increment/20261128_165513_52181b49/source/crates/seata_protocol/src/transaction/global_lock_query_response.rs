use super::abstract_transaction_response::AbstractTransactionResponse;
use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// GlobalLockQueryResponse的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalLockQueryResponse
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalLockQueryResponse {
    /// 结果、失败消息及异常码前缀；状态不能覆盖Failed结果。
    pub response: AbstractTransactionResponse,
    /// 原始锁查询结果；即使为true也不能覆盖失败结果。
    pub lockable: bool,
}

impl fmt::Debug for GlobalLockQueryResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalLockQueryResponse");
        debug.field("response", &self.response);
        debug.field("lockable", &self.lockable);
        debug.finish_non_exhaustive()
    }
}

impl GlobalLockQueryResponse {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            response: AbstractTransactionResponse::read(reader)?,
            lockable: reader.boolean16("lockable")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        self.response.write(writer)?;
        writer.boolean16(self.lockable)?;
        Ok(())
    }
}
