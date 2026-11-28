use super::abstract_transaction_response::AbstractTransactionResponse;
use crate::codec::{WireReader, WireWriter};
use crate::{BranchStatus, ProtocolError};
use std::fmt;

/// BranchRollbackResponse的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchRollbackResponse
#[derive(Clone, PartialEq, Eq)]
pub struct BranchRollbackResponse {
    /// 结果、失败消息及异常码前缀；状态不能覆盖Failed结果。
    pub response: AbstractTransactionResponse,
    /// 全局事务标识。
    pub xid: Option<String>,
    /// Java有符号long分支标识，保留全部线缆位。
    pub branch_id: i64,
    /// 分支状态。
    pub branch_status: BranchStatus,
}

impl fmt::Debug for BranchRollbackResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("BranchRollbackResponse");
        debug.field("response", &self.response);
        debug.field("xid_present", &self.xid.is_some());
        debug.field("branch_id", &self.branch_id);
        debug.field("branch_status", &self.branch_status);
        debug.finish_non_exhaustive()
    }
}

impl BranchRollbackResponse {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            response: AbstractTransactionResponse::read(reader)?,
            xid: reader.string16("xid")?,
            branch_id: reader.i64()?,
            branch_status: BranchStatus::from_java_v1(reader.u8()?)?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        self.response.write(writer)?;
        writer.string16(self.xid.as_deref(), "xid")?;
        writer.i64(self.branch_id)?;
        writer.u8(self.branch_status.to_java_v1())?;
        Ok(())
    }
}
