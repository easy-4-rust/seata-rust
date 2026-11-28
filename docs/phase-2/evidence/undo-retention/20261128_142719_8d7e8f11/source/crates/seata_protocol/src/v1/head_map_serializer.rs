use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::collections::HashSet;

/// 有序且可空值的严格HeadMap字段编码，不允许null/重复key或非规范负长。
/// 对应 Java: org.apache.seata.core.rpc.netty.v1.HeadMapSerializer
pub(crate) struct HeadMapSerializer;
impl HeadMapSerializer {
    /// 参数为完整头字段切片和条目预算；逐项检查后返回有序字段，不读取body。
    pub(crate) fn decode(
        input: &[u8],
        maximum_entries: usize,
    ) -> Result<Vec<(String, Option<String>)>, ProtocolError> {
        let mut reader = WireReader::new(input, input.len())?;
        let mut entries = Vec::new();
        let mut keys = HashSet::new();
        while reader.remaining() != 0 {
            if entries.len() >= maximum_entries {
                return Err(ProtocolError::LimitExceeded {
                    limit: "header entries",
                    maximum: maximum_entries,
                    actual: entries.len().saturating_add(1),
                });
            }
            let key =
                reader
                    .nullable_string16("header key")?
                    .ok_or(ProtocolError::InvalidValue {
                        field: "header key",
                        reason: "null header key is not representable by the Java encoder",
                    })?;
            if !keys.insert(key.clone()) {
                return Err(ProtocolError::InvalidValue {
                    field: "header key",
                    reason: "duplicate header keys are ambiguous",
                });
            }
            let value = reader.nullable_string16("header value")?;
            entries.push((key, value));
        }
        Ok(entries)
    }
    /// 参数为有序字段、头字段字节和条目预算；先检查数量，再编码新字节，失败不返回部分头。
    pub(crate) fn encode(
        entries: &[(String, Option<String>)],
        maximum_bytes: usize,
        maximum_entries: usize,
    ) -> Result<Vec<u8>, ProtocolError> {
        if entries.len() > maximum_entries {
            return Err(ProtocolError::LimitExceeded {
                limit: "header entries",
                maximum: maximum_entries,
                actual: entries.len(),
            });
        }
        let mut writer = WireWriter::new(maximum_bytes);
        let mut keys = HashSet::new();
        for (key, value) in entries {
            // 先按O(1)字节长度/剩余预算校验并写入，避免在限额前哈希巨型外部key。
            // 之后发现重复时整个临时头会被丢弃，不向调用方返回部分输出。
            writer.nullable_string16(Some(key), "header key")?;
            if !keys.insert(key.as_str()) {
                return Err(ProtocolError::InvalidValue {
                    field: "header key",
                    reason: "duplicate header keys are ambiguous",
                });
            }
            writer.nullable_string16(value.as_deref(), "header value")?;
        }
        Ok(writer.into_bytes())
    }
}
