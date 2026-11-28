use std::{fmt, io};
/// 格式化及 JSON 编码共用的字节上限写入器，超限不保留部分追加片段。
/// 对应 Java: 无直接对象；WP07 Rust 日志预算实现。
pub(crate) struct LogRecordBuffer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl LogRecordBuffer {
    /// 参数为本次编码上限；返回空的内存写入器，不持有文件或网络。
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
    /// 无参数；消费写入器并返回已有字节。
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    fn append(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
impl fmt::Write for LogRecordBuffer {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.append(value.as_bytes()).map_err(|()| fmt::Error)
    }
}
impl io::Write for LogRecordBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.append(bytes).map_err(|()| {
            io::Error::new(io::ErrorKind::InvalidData, "structured log byte budget")
        })?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
