use super::UndoCodecError;
use serde_json::Value;
use std::io::{self, Write};

/// 在写入Vec之前限制最终JSON字节，不先无界序列化再检查。
/// 对应 Java: JacksonUndoLogParser#encode的Rust输出预算扩展。
pub(crate) struct BoundedOutput {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl BoundedOutput {
    pub(crate) fn encode(value: &Value, limit: usize) -> Result<Vec<u8>, UndoCodecError> {
        let mut output = Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        };
        if serde_json::to_writer(&mut output, value).is_err() {
            return Err(if output.exceeded {
                UndoCodecError::Budget {
                    limit: "encoded output",
                }
            } else {
                UndoCodecError::Syntax
            });
        }
        Ok(output.bytes)
    }
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > self.limit)
        {
            self.exceeded = true;
            return Err(io::Error::other("Undo output budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
