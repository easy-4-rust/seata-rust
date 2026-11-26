use crate::codec::{WireReader, WireWriter};
use crate::{BranchType, ProtocolError};
use std::fmt;

/// UndoLogDeleteRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.UndoLogDeleteRequest
#[derive(Clone, PartialEq, Eq)]
pub struct UndoLogDeleteRequest {
    /// 分支模式；可表示不等于RM执行器已实现该模式。
    pub branch_type: BranchType,
    /// 资源标识；原样传输，不猜测或重写资源命名空间。完整V1消息要求非空，编解码时明确校验。
    pub resource_id: String,
    /// Java有符号short的Undo保留天数，业务校验另行进行。
    pub save_days: i16,
}

impl fmt::Debug for UndoLogDeleteRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("UndoLogDeleteRequest");
        debug.field("branch_type", &self.branch_type);
        debug.field("resource_id_bytes", &self.resource_id.len());
        debug.field("save_days", &self.save_days);
        debug.finish_non_exhaustive()
    }
}

impl UndoLogDeleteRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            branch_type: BranchType::from_java_v1(reader.u8()?)?,
            resource_id: reader
                .string16("resource_id")?
                .ok_or(ProtocolError::InvalidValue {
                    field: "resource_id",
                    reason: "complete message requires a nonempty field",
                })?,
            save_days: reader.i16()?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.u8(self.branch_type.to_java_v1())?;
        if self.resource_id.is_empty() {
            return Err(ProtocolError::InvalidValue {
                field: "resource_id",
                reason: "complete message requires a nonempty field",
            });
        }
        writer.string16(Some(&self.resource_id), "resource_id")?;
        writer.i16(self.save_days)?;
        Ok(())
    }
}
