use crate::codec::{WireReader, WireWriter};
use crate::{BranchStatus, BranchType, ProtocolError};
use std::fmt;

/// BranchReportRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchReportRequest
#[derive(Clone, PartialEq, Eq)]
pub struct BranchReportRequest {
    /// 全局事务标识。
    pub xid: Option<String>,
    /// Java有符号long分支标识，保留全部线缆位。
    pub branch_id: i64,
    /// 报告的分支状态。
    pub status: BranchStatus,
    /// 资源标识；原样传输，不猜测或重写资源命名空间。
    pub resource_id: Option<String>,
    /// 业务上下文，Java零长度解码为None。
    pub application_data: Option<String>,
    /// 分支模式；可表示不等于RM执行器已实现该模式。
    pub branch_type: BranchType,
}

impl fmt::Debug for BranchReportRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("BranchReportRequest");
        debug.field("xid_present", &self.xid.is_some());
        debug.field("branch_id", &self.branch_id);
        debug.field("status", &self.status);
        debug.field("resource_id_present", &self.resource_id.is_some());
        debug.field("application_data_present", &self.application_data.is_some());
        debug.field("branch_type", &self.branch_type);
        debug.finish_non_exhaustive()
    }
}

impl BranchReportRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            xid: reader.string16("xid")?,
            branch_id: reader.i64()?,
            status: BranchStatus::from_java_v1(reader.u8()?)?,
            resource_id: reader.string16("resource_id")?,
            application_data: reader.string32("application_data")?,
            branch_type: BranchType::from_java_v1(reader.u8()?)?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.string16(self.xid.as_deref(), "xid")?;
        writer.i64(self.branch_id)?;
        writer.u8(self.status.to_java_v1())?;
        writer.string16(self.resource_id.as_deref(), "resource_id")?;
        writer.string32(self.application_data.as_deref(), "application_data")?;
        writer.u8(self.branch_type.to_java_v1())?;
        Ok(())
    }
}
