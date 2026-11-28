use super::{bounded_output::BoundedOutput, fory_class::ForyClass, UndoCodecError};
use std::{collections::HashMap, io::Write};

/// Fory Java compatible 数据、局部引用及类元数据写入。
/// 对应 Java: MemoryBuffer、ClassResolver 和 MapRefResolver。
pub(super) struct ForyWriter<'a, 'data> {
    output: &'a mut BoundedOutput,
    classes: Vec<ForyClass>,
    strings: HashMap<&'data str, u32>,
    numbers: HashMap<(u8, u64), u32>,
    references: u32,
    written: usize,
}

impl<'a, 'data> ForyWriter<'a, 'data> {
    pub(super) fn new(output: &'a mut BoundedOutput) -> Self {
        Self {
            output,
            classes: Vec::new(),
            strings: HashMap::new(),
            numbers: HashMap::new(),
            references: 0,
            written: 0,
        }
    }

    pub(super) fn finish(&mut self) -> Result<i32, UndoCodecError> {
        let offset = i32::try_from(self.written).map_err(|_| Self::length_error())?;
        self.var(self.classes.len() as u32)?;
        // 固定类数很少。按正文登记顺序追加独立的 Java ClassDef。
        for index in 0..self.classes.len() {
            self.raw(self.classes[index].encoded())?;
        }
        Ok(offset)
    }

    pub(super) fn class(&mut self, class: ForyClass) -> Result<(), UndoCodecError> {
        let id = if let Some(index) = self.classes.iter().position(|item| *item == class) {
            index
        } else {
            let index = self.classes.len();
            self.classes.push(class);
            index
        };
        self.var((id as u32) * 2 + 1)
    }

    pub(super) fn fresh(&mut self) -> Result<(), UndoCodecError> {
        self.references = self.references.checked_add(1).ok_or(Self::length_error())?;
        self.byte(0)
    }

    pub(super) fn list(&mut self, count: usize, class: ForyClass) -> Result<(), UndoCodecError> {
        self.fresh()?;
        self.byte(0x42)?;
        self.var(u32::try_from(count).map_err(|_| Self::length_error())?)?;
        if count > 0 {
            self.byte(5)?;
            self.class(class)?;
        }
        Ok(())
    }

    pub(super) fn text_field(&mut self, text: Option<&str>) -> Result<(), UndoCodecError> {
        let Some(text) = text else {
            return self.byte(0xfd);
        };
        self.byte(0xff)?;
        self.text(text)
    }

    pub(super) fn string_value(&mut self, text: &'data str) -> Result<(), UndoCodecError> {
        if let Some(id) = self.strings.get(text).copied() {
            self.byte(0xfe)?;
            return self.var(id);
        }
        self.strings.insert(text, self.references);
        self.fresh()?;
        self.byte(0x2c)?;
        self.text(text)
    }

    pub(super) fn number_value(
        &mut self,
        kind: u8,
        bits: u64,
        write: impl FnOnce(&mut Self) -> Result<(), UndoCodecError>,
    ) -> Result<(), UndoCodecError> {
        if let Some(id) = self.numbers.get(&(kind, bits)).copied() {
            self.byte(0xfe)?;
            return self.var(id);
        }
        self.numbers.insert((kind, bits), self.references);
        self.fresh()?;
        self.byte(kind)?;
        write(self)
    }

    fn text(&mut self, text: &str) -> Result<(), UndoCodecError> {
        if text.is_ascii() {
            self.text_header(text.len(), 0)?;
            return self.raw(text.as_bytes());
        }
        if text.chars().all(|ch| u32::from(ch) <= 255) {
            self.text_header(text.chars().count(), 0)?;
            for ch in text.chars() {
                self.byte(u32::from(ch) as u8)?;
            }
        } else {
            let count = text
                .encode_utf16()
                .count()
                .checked_mul(2)
                .ok_or(Self::length_error())?;
            self.text_header(count, 1)?;
            for unit in text.encode_utf16() {
                self.raw(&unit.to_le_bytes())?;
            }
        }
        Ok(())
    }

    fn text_header(&mut self, len: usize, coder: u32) -> Result<(), UndoCodecError> {
        let len = u32::try_from(len).map_err(|_| Self::length_error())?;
        self.var(len.checked_mul(4).ok_or(Self::length_error())? | coder)
    }

    pub(super) fn int(&mut self, value: i32) -> Result<(), UndoCodecError> {
        self.var(((value as u32) << 1) ^ ((value >> 31) as u32))
    }

    pub(super) fn long(&mut self, value: i64) -> Result<(), UndoCodecError> {
        if (-1_073_741_824..=1_073_741_823).contains(&value) {
            return self.raw(&((value as i32) << 1).to_le_bytes());
        }
        self.byte(1)?;
        self.raw(&value.to_le_bytes())
    }

    pub(super) fn var(&mut self, mut number: u32) -> Result<(), UndoCodecError> {
        let mut bytes = [0; 5];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if number < 128 {
                *byte = number as u8;
                return self.raw(&bytes[..=index]);
            }
            *byte = number as u8 | 0x80;
            number >>= 7;
        }
        Err(UndoCodecError::Syntax)
    }

    pub(super) fn byte(&mut self, value: u8) -> Result<(), UndoCodecError> {
        self.raw(&[value])
    }

    pub(super) fn raw(&mut self, data: &[u8]) -> Result<(), UndoCodecError> {
        self.output
            .write_all(data)
            .map_err(|_| UndoCodecError::Syntax)?;
        self.written += data.len();
        Ok(())
    }

    fn length_error() -> UndoCodecError {
        UndoCodecError::Budget {
            limit: "Fory length",
        }
    }
}
