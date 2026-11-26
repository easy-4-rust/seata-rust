use super::{bounded_output::BoundedOutput, kryo_class::KryoClass, UndoCodecError};
use std::{collections::HashMap, io::Write};

/// 固定 Kryo 5.4 字符串、变长整数及引用写入。
/// 对应 Java: Kryo Output、DefaultClassResolver 和 MapReferenceResolver。
pub(super) struct KryoWriter<'a, 'data> {
    output: &'a mut BoundedOutput,
    dynamic_classes: Vec<KryoClass>,
    strings: HashMap<&'data str, u32>,
    references: u32,
}

impl<'a, 'data> KryoWriter<'a, 'data> {
    pub(super) fn new(output: &'a mut BoundedOutput) -> Self {
        Self {
            output,
            dynamic_classes: Vec::new(),
            strings: HashMap::new(),
            references: 0,
        }
    }

    pub(super) fn class(&mut self, class: KryoClass) -> Result<(), UndoCodecError> {
        if let Some(token) = class.registered() {
            return self.u32(token);
        }
        self.byte(1)?;
        if let Some(id) = self.dynamic_classes.iter().position(|item| *item == class) {
            return self.u32(Self::length(id)?);
        }
        self.u32(Self::length(self.dynamic_classes.len())?)?;
        self.dynamic_classes.push(class);
        self.ascii(class.name())
    }

    pub(super) fn fresh(&mut self) -> Result<(), UndoCodecError> {
        self.references = self.references.checked_add(1).ok_or(Self::length_error())?;
        self.byte(1)
    }

    pub(super) fn string_ref(&mut self, value: Option<&'data str>) -> Result<(), UndoCodecError> {
        let Some(value) = value else {
            return self.byte(0);
        };
        if let Some(id) = self.strings.get(value).copied() {
            return self.u32(id.checked_add(2).ok_or(Self::length_error())?);
        }
        self.strings.insert(value, self.references);
        self.fresh()?;
        self.string(value)
    }

    pub(super) fn list_header(
        &mut self,
        len: usize,
        class: KryoClass,
    ) -> Result<(), UndoCodecError> {
        self.class(KryoClass::ArrayList)?;
        self.fresh()?;
        if len == 0 {
            return self.byte(1);
        }
        let size = Self::length(len)?
            .checked_add(1)
            .ok_or(Self::length_error())?;
        self.flagged(size)?;
        self.class(class)?;
        self.byte(0)
    }

    fn string(&mut self, text: &str) -> Result<(), UndoCodecError> {
        if (2..=32).contains(&text.len()) && text.is_ascii() {
            return self.ascii(text);
        }
        let units = Self::length(text.encode_utf16().count())?;
        self.flagged(units.checked_add(1).ok_or(Self::length_error())?)?;
        // Kryo 按 UTF-16 单元写入，补充字符的代理对各占三字节。
        for unit in text.encode_utf16() {
            if unit <= 0x7f {
                self.byte(unit as u8)?;
            } else if unit <= 0x7ff {
                self.raw(&[(0xc0 | (unit >> 6)) as u8, (0x80 | (unit & 0x3f)) as u8])?;
            } else {
                self.raw(&[
                    (0xe0 | (unit >> 12)) as u8,
                    (0x80 | ((unit >> 6) & 0x3f)) as u8,
                    (0x80 | (unit & 0x3f)) as u8,
                ])?;
            }
        }
        Ok(())
    }

    fn ascii(&mut self, value: &str) -> Result<(), UndoCodecError> {
        let Some((last, first)) = value.as_bytes().split_last() else {
            return Err(UndoCodecError::Syntax);
        };
        self.raw(first)?;
        self.byte(*last | 0x80)
    }

    fn flagged(&mut self, number: u32) -> Result<(), UndoCodecError> {
        let first = (number as u8 & 0x3f) | 0x80;
        if number <= 0x3f {
            return self.byte(first);
        }
        self.byte(first | 0x40)?;
        self.u32(number >> 6)
    }

    pub(super) fn i32(&mut self, number: i32) -> Result<(), UndoCodecError> {
        self.u32(((number as u32) << 1) ^ ((number >> 31) as u32))
    }

    pub(super) fn i64(&mut self, number: i64) -> Result<(), UndoCodecError> {
        self.u64(((number as u64) << 1) ^ ((number >> 63) as u64))
    }

    pub(super) fn u32(&mut self, number: u32) -> Result<(), UndoCodecError> {
        self.u64(u64::from(number))
    }

    pub(super) fn u64(&mut self, mut number: u64) -> Result<(), UndoCodecError> {
        let mut bytes = [0; 9];
        for (index, byte) in bytes[..8].iter_mut().enumerate() {
            if number < 128 {
                *byte = number as u8;
                return self.raw(&bytes[..=index]);
            }
            *byte = number as u8 | 0x80;
            number >>= 7;
        }
        bytes[8] = number as u8;
        self.raw(&bytes)
    }

    pub(super) fn byte(&mut self, byte: u8) -> Result<(), UndoCodecError> {
        self.raw(&[byte])
    }

    pub(super) fn raw(&mut self, data: &[u8]) -> Result<(), UndoCodecError> {
        self.output
            .write_all(data)
            .map_err(|_| UndoCodecError::Syntax)
    }

    fn length(len: usize) -> Result<u32, UndoCodecError> {
        u32::try_from(len).map_err(|_| Self::length_error())
    }

    fn length_error() -> UndoCodecError {
        UndoCodecError::Budget {
            limit: "Kryo length",
        }
    }
}
