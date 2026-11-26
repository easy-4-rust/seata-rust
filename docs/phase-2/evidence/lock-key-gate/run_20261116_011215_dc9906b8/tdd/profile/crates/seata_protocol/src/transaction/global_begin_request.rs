use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// GlobalBeginRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalBeginRequest
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalBeginRequest {
    /// Java有符号毫秒超时；合法业务范围由TM/TC另行校验。
    pub timeout: i32,
    /// 事务名称；Java零长度字段解码为None。
    pub transaction_name: Option<String>,
}

impl fmt::Debug for GlobalBeginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalBeginRequest");
        debug.field("timeout", &self.timeout);
        debug.field("transaction_name_present", &self.transaction_name.is_some());
        debug.finish_non_exhaustive()
    }
}

impl GlobalBeginRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            timeout: reader.i32()?,
            transaction_name: reader.string16("transaction_name")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.i32(self.timeout)?;
        writer.string16(self.transaction_name.as_deref(), "transaction_name")?;
        Ok(())
    }
}
