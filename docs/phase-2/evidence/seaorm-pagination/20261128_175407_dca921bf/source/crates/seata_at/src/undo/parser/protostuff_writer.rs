use super::{bounded_output::BoundedOutput, UndoCodecError};
use std::io::Write;

/// 有界 Protostuff 字段写入器。
/// 对应 Java: io.protostuff.ProtostuffOutput。
pub(super) struct ProtostuffWriter<'a>(pub(super) &'a mut BoundedOutput);

impl ProtostuffWriter<'_> {
    pub(super) fn number(&mut self, tag: u32, number: u64) -> Result<(), UndoCodecError> {
        self.tag(tag, 0)?;
        self.varint(number)
    }

    pub(super) fn fixed64(&mut self, tag: u32, number: u64) -> Result<(), UndoCodecError> {
        self.tag(tag, 1)?;
        self.raw(&number.to_le_bytes())
    }

    pub(super) fn bytes(&mut self, tag: u32, bytes: &[u8]) -> Result<(), UndoCodecError> {
        self.tag(tag, 2)?;
        self.varint(bytes.len() as u64)?;
        self.raw(bytes)
    }

    pub(super) fn group(
        &mut self,
        tag: u32,
        write: impl FnOnce(&mut Self) -> Result<(), UndoCodecError>,
    ) -> Result<(), UndoCodecError> {
        self.tag(tag, 3)?;
        write(self)?;
        self.tag(tag, 4)
    }

    fn tag(&mut self, tag: u32, kind: u64) -> Result<(), UndoCodecError> {
        self.varint((u64::from(tag) << 3) | kind)
    }

    fn varint(&mut self, mut number: u64) -> Result<(), UndoCodecError> {
        let mut bytes = [0; 10];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (number & 0x7f) as u8;
            number >>= 7;
            if number == 0 {
                return self.raw(&bytes[..=index]);
            }
            *byte |= 0x80;
        }
        Err(UndoCodecError::Syntax)
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), UndoCodecError> {
        self.0.write_all(bytes).map_err(|_| UndoCodecError::Syntax)
    }
}
