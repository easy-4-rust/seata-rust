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
        Self::generate(limit, |output| {
            serde_json::to_writer(output, value).map_err(|_| UndoCodecError::Syntax)
        })
    }

    pub(crate) fn generate(
        limit: usize,
        write: impl FnOnce(&mut Self) -> Result<(), UndoCodecError>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let mut output = Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        };
        let result = write(&mut output);
        if output.exceeded {
            return Err(UndoCodecError::Budget {
                limit: "encoded output",
            });
        }
        result?;
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
