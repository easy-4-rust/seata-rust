use super::{fory_class::ForyClass, JacksonUndoFieldValue, UndoCodecError, UndoCodecLimits};

/// 固定 Fory 0.12.3 数据、类描述与字符串引用的有界读取。
/// 对应 Java: MemoryBuffer、ClassResolver 和 MapRefResolver。
pub(super) struct ForyReader<'a> {
    remaining: &'a [u8],
    limits: UndoCodecLimits,
    classes: Vec<ForyClass>,
    references: Vec<Option<(JacksonUndoFieldValue, usize)>>,
    nodes: usize,
    scalar: usize,
}

impl<'a> ForyReader<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: UndoCodecLimits) -> Result<Self, UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        let (header, rest) = bytes.split_at_checked(4).ok_or(UndoCodecError::Syntax)?;
        let offset = i32::from_le_bytes(header.try_into().map_err(|_| UndoCodecError::Syntax)?);
        let offset = usize::try_from(offset).map_err(|_| UndoCodecError::Shape {
            reason: "Fory metadata offset",
        })?;
        let (payload, metadata) = rest.split_at_checked(offset).ok_or(UndoCodecError::Shape {
            reason: "Fory metadata offset",
        })?;
        let mut reader = Self {
            remaining: metadata,
            limits,
            classes: Vec::new(),
            references: Vec::new(),
            nodes: 0,
            scalar: 0,
        };
        let count = reader.var()? as usize;
        if !(1..=ForyClass::ALL.len()).contains(&count) {
            return Err(UndoCodecError::Shape {
                reason: "Fory class count",
            });
        }
        for _ in 0..count {
            let class = ForyClass::ALL
                .iter()
                .find(|class| reader.remaining.starts_with(class.encoded()))
                .copied()
                .ok_or(UndoCodecError::Unsupported {
                    kind: "Fory class definition",
                })?;
            if reader.classes.contains(&class) {
                return Err(UndoCodecError::Shape {
                    reason: "duplicate Fory class definition",
                });
            }
            reader.bytes(class.encoded().len())?;
            reader.classes.push(class);
        }
        if !reader.remaining.is_empty() {
            return Err(UndoCodecError::Syntax);
        }
        reader.remaining = payload;
        Ok(reader)
    }

    pub(super) fn node(&mut self, depth: usize) -> Result<(), UndoCodecError> {
        self.nodes += 1;
        if depth > self.limits.depth || self.nodes > self.limits.nodes {
            return Err(UndoCodecError::Budget {
                limit: "Fory nodes/depth",
            });
        }
        Ok(())
    }

    pub(super) fn class(&mut self) -> Result<ForyClass, UndoCodecError> {
        let value = self.var()?;
        if value & 1 == 0 {
            return Err(UndoCodecError::Shape {
                reason: "Fory class reference",
            });
        }
        self.classes
            .get((value >> 1) as usize)
            .copied()
            .ok_or(UndoCodecError::Shape {
                reason: "Fory class reference",
            })
    }

    pub(super) fn define_reference(&mut self) -> usize {
        let index = self.references.len();
        self.references.push(None);
        index
    }

    pub(super) fn define_scalar(
        &mut self,
        index: usize,
        value: &JacksonUndoFieldValue,
        len: usize,
    ) {
        self.references[index] = Some((value.clone(), len));
    }

    pub(super) fn read_scalar_reference(
        &mut self,
    ) -> Result<JacksonUndoFieldValue, UndoCodecError> {
        let index = self.var()? as usize;
        let (_, len) =
            self.references
                .get(index)
                .and_then(Option::as_ref)
                .ok_or(UndoCodecError::Shape {
                    reason: "Fory scalar reference",
                })?;
        self.charge(*len)?;
        self.references[index]
            .as_ref()
            .map(|(value, _)| value.clone())
            .ok_or(UndoCodecError::Syntax)
    }

    pub(super) fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    pub(super) fn fresh(&mut self, depth: usize) -> Result<(), UndoCodecError> {
        self.node(depth)?;
        self.expect(0)?;
        self.define_reference();
        Ok(())
    }

    pub(super) fn list(
        &mut self,
        depth: usize,
        maximum: usize,
        expected: ForyClass,
    ) -> Result<Option<usize>, UndoCodecError> {
        self.node(depth)?;
        match self.byte()? {
            0xfd => return Ok(None),
            0 => {
                self.define_reference();
            }
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "Fory non-string collection reference",
                })
            }
        }
        self.expect(0x42)?;
        let count = self.var()? as usize;
        if count > maximum || count > self.limits.nodes.saturating_sub(self.nodes) {
            return Err(UndoCodecError::Budget {
                limit: "Fory Undo collection elements",
            });
        }
        if count != 0 {
            self.expect(5)?;
            if self.class()? != expected {
                return Err(UndoCodecError::Shape {
                    reason: "Fory collection element class",
                });
            }
        }
        Ok(Some(count))
    }

    pub(super) fn text_field(&mut self, depth: usize) -> Result<Option<String>, UndoCodecError> {
        self.node(depth)?;
        match self.byte()? {
            0xfd => Ok(None),
            0xff => self.text().map(Some),
            _ => Err(UndoCodecError::Shape {
                reason: "Fory string field marker",
            }),
        }
    }

    pub(super) fn text(&mut self) -> Result<String, UndoCodecError> {
        let header = self.var()?;
        let count = (header >> 2) as usize;
        let bytes = self.bytes(count)?;
        match header & 3 {
            0 => {
                let size = count + bytes.iter().filter(|byte| **byte >= 128).count();
                self.charge(size)?;
                Ok(bytes.iter().map(|byte| char::from(*byte)).collect())
            }
            1 if count % 2 == 0 => {
                let units = bytes
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
                let mut text = String::new();
                for ch in char::decode_utf16(units) {
                    let ch = ch.map_err(|_| UndoCodecError::Syntax)?;
                    if text.len() + ch.len_utf8() > self.limits.scalar {
                        return Err(UndoCodecError::Budget {
                            limit: "Fory scalar bytes",
                        });
                    }
                    self.charge(ch.len_utf8())?;
                    text.push(ch);
                }
                Ok(text)
            }
            _ => Err(UndoCodecError::Unsupported {
                kind: "Fory string coder",
            }),
        }
    }

    pub(super) fn var(&mut self) -> Result<u32, UndoCodecError> {
        let mut value = 0;
        for shift in [0, 7, 14, 21, 28] {
            let byte = self.byte()?;
            if shift == 28 && byte >= 16 {
                return Err(UndoCodecError::Syntax);
            }
            value |= u32::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(value);
            }
        }
        Err(UndoCodecError::Syntax)
    }

    pub(super) fn int(&mut self) -> Result<i32, UndoCodecError> {
        let value = self.var()?;
        Ok(((value >> 1) as i32) ^ -((value & 1) as i32))
    }

    pub(super) fn long(&mut self) -> Result<i64, UndoCodecError> {
        let first = *self.remaining.first().ok_or(UndoCodecError::Syntax)?;
        if first & 1 == 0 {
            let data = self
                .bytes(4)?
                .try_into()
                .map_err(|_| UndoCodecError::Syntax)?;
            return Ok(i64::from(i32::from_le_bytes(data) >> 1));
        }
        self.expect(1)?;
        Ok(self.fixed64()? as i64)
    }

    pub(super) fn fixed64(&mut self) -> Result<u64, UndoCodecError> {
        let data = self
            .bytes(8)?
            .try_into()
            .map_err(|_| UndoCodecError::Syntax)?;
        Ok(u64::from_le_bytes(data))
    }

    pub(super) fn numeric<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, UndoCodecError>,
    ) -> Result<T, UndoCodecError> {
        let start = self.remaining.len();
        let result = read(self)?;
        self.charge(start - self.remaining.len())?;
        Ok(result)
    }

    fn charge(&mut self, bytes: usize) -> Result<(), UndoCodecError> {
        if bytes > self.limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "Fory scalar bytes",
            });
        }
        self.scalar += bytes;
        if self.scalar > self.limits.total_scalar {
            return Err(UndoCodecError::Budget {
                limit: "Fory total scalar bytes",
            });
        }
        Ok(())
    }

    pub(super) fn byte(&mut self) -> Result<u8, UndoCodecError> {
        Ok(self.bytes(1)?[0])
    }

    pub(super) fn expect(&mut self, value: u8) -> Result<(), UndoCodecError> {
        if self.byte()? == value {
            Ok(())
        } else {
            Err(UndoCodecError::Syntax)
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], UndoCodecError> {
        let (value, rest) = self
            .remaining
            .split_at_checked(len)
            .ok_or(UndoCodecError::Syntax)?;
        self.remaining = rest;
        Ok(value)
    }

    pub(super) fn finish(self) -> Result<(), UndoCodecError> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(UndoCodecError::Syntax)
        }
    }
}
