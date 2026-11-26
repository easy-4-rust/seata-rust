use crate::codec::{WireReader, WireWriter};
use crate::{BranchType, ProtocolError};
use std::fmt;

/// GlobalLockQueryRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalLockQueryRequest
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalLockQueryRequest {
    /// 全局事务标识。
    pub xid: Option<String>,
    /// 分支模式；可表示不等于RM执行器已实现该模式。
    pub branch_type: BranchType,
    /// 资源标识；原样传输，不猜测或重写资源命名空间。
    pub resource_id: Option<String>,
    /// 锁键文本；不在协议层改变大小写或主键编码。
    pub lock_key: Option<String>,
    /// 业务上下文，Java零长度解码为None。
    pub application_data: Option<String>,
}

impl fmt::Debug for GlobalLockQueryRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalLockQueryRequest");
        debug.field("xid_present", &self.xid.is_some());
        debug.field("branch_type", &self.branch_type);
        debug.field("resource_id_present", &self.resource_id.is_some());
        debug.field("lock_key_present", &self.lock_key.is_some());
        debug.field("application_data_present", &self.application_data.is_some());
        debug.finish_non_exhaustive()
    }
}

impl GlobalLockQueryRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            xid: reader.string16("xid")?,
            branch_type: BranchType::from_java_v1(reader.u8()?)?,
            resource_id: reader.string16("resource_id")?,
            lock_key: reader.string32("lock_key")?,
            application_data: reader.string32("application_data")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.string16(self.xid.as_deref(), "xid")?;
        writer.u8(self.branch_type.to_java_v1())?;
        writer.string16(self.resource_id.as_deref(), "resource_id")?;
        writer.string32(self.lock_key.as_deref(), "lock_key")?;
        writer.string32(self.application_data.as_deref(), "application_data")?;
        Ok(())
    }
}
