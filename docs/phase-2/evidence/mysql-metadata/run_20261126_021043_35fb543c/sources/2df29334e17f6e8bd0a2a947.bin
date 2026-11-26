use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// RegisterTMResponse的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.RegisterTMResponse
#[derive(Clone, PartialEq, Eq)]
pub struct RegisterTMResponse {
    /// 身份是否被接受；原生V1不携带普通事务错误前缀。
    pub identified: bool,
    /// 原始线缆版本是否非空；None与Java getter的构建默认值分开表示。
    pub wire_version: Option<String>,
}

impl fmt::Debug for RegisterTMResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("RegisterTMResponse");
        debug.field("identified", &self.identified);
        debug.field("wire_version_present", &self.wire_version.is_some());
        debug.finish_non_exhaustive()
    }
}

impl RegisterTMResponse {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            identified: {
                match reader.u8()? {
                    0 => false,
                    1 => true,
                    value => {
                        return Err(ProtocolError::InvalidBoolean {
                            field: "identified",
                            value: u16::from(value),
                        })
                    }
                }
            },
            wire_version: reader.string16("wire_version")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.u8(u8::from(self.identified))?;
        writer.string16(self.wire_version.as_deref(), "wire_version")?;
        Ok(())
    }

    /// 返回此线缆值在Java解码后的version投影；参数java_current由调用方提供，库不猜测默认版本。
    /// None或Some空串均编码为缺省长度0，因此使用给定构建默认值。
    pub fn java_version<'a>(&'a self, java_current: &'a str) -> &'a str {
        self.wire_version
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(java_current)
    }
}
