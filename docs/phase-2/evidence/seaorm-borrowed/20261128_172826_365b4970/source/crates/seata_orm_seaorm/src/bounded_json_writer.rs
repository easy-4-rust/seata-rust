use std::io::{Error, ErrorKind, Result, Write};

/// JSON 绑定的有界 UTF-8 序列化缓冲，超过单值预算即停止分配。
/// 对应 Java: 无直接对象；StatementProxy 绑定值预算的 Rust 扩展。
pub(crate) struct BoundedJsonWriter(Vec<u8>);
impl BoundedJsonWriter {
    /// 无参数；返回空的单值预算缓冲。
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
    /// 消耗完整序列化缓冲；返回经 UTF-8 核验的 JSON 文本。
    pub(crate) fn finish(self) -> std::result::Result<String, crate::SeaOrmMySqlError> {
        String::from_utf8(self.0).map_err(|_| crate::SeaOrmMySqlError::Input {
            reason: "JSON UTF-8 required",
        })
    }
}
impl Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if bytes.len() > 1_048_576_usize.saturating_sub(self.0.len()) {
            return Err(Error::new(ErrorKind::InvalidInput, "JSON parameter budget"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
