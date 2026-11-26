use super::abstract_transaction_response::AbstractTransactionResponse;
use crate::codec::{WireReader, WireWriter};
use crate::{GlobalStatus, ProtocolError};
use std::fmt;

/// GlobalCommitResponse的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalCommitResponse
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalCommitResponse {
    /// 结果、失败消息及异常码前缀；状态不能覆盖Failed结果。
    pub response: AbstractTransactionResponse,
    /// 全局状态，不替代响应结果码。
    pub global_status: GlobalStatus,
}

impl fmt::Debug for GlobalCommitResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalCommitResponse");
        debug.field("response", &self.response);
        debug.field("global_status", &self.global_status);
        debug.finish_non_exhaustive()
    }
}

impl GlobalCommitResponse {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            response: AbstractTransactionResponse::read(reader)?,
            global_status: GlobalStatus::from_java_v1(reader.u8()?)?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        self.response.write(writer)?;
        writer.u8(self.global_status.to_java_v1())?;
        Ok(())
    }
}
