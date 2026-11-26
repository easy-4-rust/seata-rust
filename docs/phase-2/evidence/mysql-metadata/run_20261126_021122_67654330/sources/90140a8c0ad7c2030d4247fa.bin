use crate::codec::{WireReader, WireWriter};
use crate::{BranchType, ProtocolError};
use std::fmt;

/// BranchCommitRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchCommitRequest
#[derive(Clone, PartialEq, Eq)]
pub struct BranchCommitRequest {
    /// 全局事务标识。完整V1消息要求非空，编解码时明确校验。
    pub xid: String,
    /// Java有符号long分支标识，保留全部线缆位。
    pub branch_id: i64,
    /// 分支模式；可表示不等于RM执行器已实现该模式。
    pub branch_type: BranchType,
    /// 资源标识；原样传输，不猜测或重写资源命名空间。完整V1消息要求非空，编解码时明确校验。
    pub resource_id: String,
    /// 业务上下文，Java零长度解码为None。
    pub application_data: Option<String>,
}

impl fmt::Debug for BranchCommitRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("BranchCommitRequest");
        debug.field("xid_bytes", &self.xid.len());
        debug.field("branch_id", &self.branch_id);
        debug.field("branch_type", &self.branch_type);
        debug.field("resource_id_bytes", &self.resource_id.len());
        debug.field("application_data_present", &self.application_data.is_some());
        debug.finish_non_exhaustive()
    }
}

impl BranchCommitRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            xid: reader.string16("xid")?.ok_or(ProtocolError::InvalidValue {
                field: "xid",
                reason: "complete message requires a nonempty field",
            })?,
            branch_id: reader.i64()?,
            branch_type: BranchType::from_java_v1(reader.u8()?)?,
            resource_id: reader
                .string16("resource_id")?
                .ok_or(ProtocolError::InvalidValue {
                    field: "resource_id",
                    reason: "complete message requires a nonempty field",
                })?,
            application_data: reader.string32("application_data")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        if self.xid.is_empty() {
            return Err(ProtocolError::InvalidValue {
                field: "xid",
                reason: "complete message requires a nonempty field",
            });
        }
        writer.string16(Some(&self.xid), "xid")?;
        writer.i64(self.branch_id)?;
        writer.u8(self.branch_type.to_java_v1())?;
        if self.resource_id.is_empty() {
            return Err(ProtocolError::InvalidValue {
                field: "resource_id",
                reason: "complete message requires a nonempty field",
            });
        }
        writer.string16(Some(&self.resource_id), "resource_id")?;
        writer.string32(self.application_data.as_deref(), "application_data")?;
        Ok(())
    }
}
