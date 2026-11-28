use super::abstract_transaction_response::AbstractTransactionResponse;
use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// BranchRegisterResponse的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchRegisterResponse
#[derive(Clone, PartialEq, Eq)]
pub struct BranchRegisterResponse {
    /// 结果、失败消息及异常码前缀；状态不能覆盖Failed结果。
    pub response: AbstractTransactionResponse,
    /// Java有符号long分支标识，保留全部线缆位。
    pub branch_id: i64,
}

impl fmt::Debug for BranchRegisterResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("BranchRegisterResponse");
        debug.field("response", &self.response);
        debug.field("branch_id", &self.branch_id);
        debug.finish_non_exhaustive()
    }
}

impl BranchRegisterResponse {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            response: AbstractTransactionResponse::read(reader)?,
            branch_id: reader.i64()?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        self.response.write(writer)?;
        writer.i64(self.branch_id)?;
        Ok(())
    }
}
