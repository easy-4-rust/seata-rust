use super::{kryo_class::KryoClass, UndoCodecError, UndoCodecLimits};

/// 固定 Kryo 5.4 原 Undo 格式读取器，保留文档内类表和字符串引用。
/// 对应 Java: Kryo Input、DefaultClassResolver 与 MapReferenceResolver。
pub(super) struct KryoReader<'a> {
    remaining: &'a [u8],
    limits: UndoCodecLimits,
    dynamic_classes: Vec<KryoClass>,
    references: Vec<Option<String>>,
    scalar_bytes: usize,
    nodes: usize,
}

impl<'a> KryoReader<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: UndoCodecLimits) -> Result<Self, UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        Ok(Self {
            remaining: bytes,
            limits,
            dynamic_classes: Vec::new(),
            references: Vec::new(),
            scalar_bytes: 0,
            nodes: 0,
        })
    }

    pub(super) fn class(&mut self) -> Result<Option<KryoClass>, UndoCodecError> {
        match self.u32()? {
            0 => Ok(None),
            1 => {
                let id = self.u32()? as usize;
                if let Some(class) = self.dynamic_classes.get(id).copied() {
                    return Ok(Some(class));
                }
                if id != self.dynamic_classes.len() {
                    return Err(UndoCodecError::Shape {
                        reason: "Kryo class name index",
                    });
                }
                let name = self.text()?.ok_or(UndoCodecError::Syntax)?;
                let class = KryoClass::by_name(&name).ok_or(UndoCodecError::Unsupported {
                    kind: "Kryo dynamic class",
                })?;
                self.dynamic_classes.push(class);
                Ok(Some(class))
            }
            id => KryoClass::by_token(id)
                .map(Some)
                .ok_or(UndoCodecError::Unsupported {
                    kind: "Kryo registered class",
                }),
        }
    }

    pub(super) fn node(&mut self, depth: usize) -> Result<(), UndoCodecError> {
        self.nodes += 1;
        if depth > self.limits.depth || self.nodes > self.limits.nodes {
            return Err(UndoCodecError::Budget {
                limit: "Kryo nodes/depth",
            });
        }
        Ok(())
    }

    pub(super) fn fresh(&mut self, depth: usize) -> Result<(), UndoCodecError> {
        self.node(depth)?;
        if self.u32()? != 1 {
            return Err(UndoCodecError::Unsupported {
                kind: "Kryo non-string object reference",
            });
        }
        self.references.push(None);
        Ok(())
    }

    pub(super) fn string_ref(&mut self, depth: usize) -> Result<Option<String>, UndoCodecError> {
        self.node(depth)?;
        match self.u32()? {
            0 => Ok(None),
            1 => {
                let text = self.text()?.ok_or(UndoCodecError::Syntax)?;
                self.references.push(Some(text.clone()));
                Ok(Some(text))
            }
            id => {
                let index = (id - 2) as usize;
                let len = self
                    .references
                    .get(index)
                    .and_then(Option::as_ref)
                    .ok_or(UndoCodecError::Shape {
                        reason: "Kryo string reference",
                    })?
                    .len();
                self.charge(len)?;
                Ok(self.references[index].clone())
            }
        }
    }

    pub(super) fn list(
        &mut self,
        depth: usize,
        maximum: usize,
        expected: KryoClass,
    ) -> Result<Option<usize>, UndoCodecError> {
        match self.class()? {
            None => return Ok(None),
            Some(KryoClass::ArrayList) => {}
            _ => {
                return Err(UndoCodecError::Shape {
                    reason: "Kryo list class",
                })
            }
        }
        self.fresh(depth)?;
        let first = self.byte()?;
        if first == 1 {
            return Ok(Some(0));
        }
        if first & 0x80 == 0 {
            return Err(UndoCodecError::Unsupported {
                kind: "Kryo heterogeneous list",
            });
        }
        let count = self
            .flagged_value(first)?
            .checked_sub(1)
            .ok_or(UndoCodecError::Syntax)? as usize;
        if count == 0 {
            return Err(UndoCodecError::Syntax);
        }
        if count > maximum || count > self.limits.nodes.saturating_sub(self.nodes) {
            return Err(UndoCodecError::Budget {
                limit: "Kryo Undo collection elements",
            });
        }
        if self.class()? != Some(expected) || self.byte()? != 0 {
            return Err(UndoCodecError::Shape {
                reason: "Kryo list element type/nullability",
            });
        }
        Ok(Some(count))
    }

    fn text(&mut self) -> Result<Option<String>, UndoCodecError> {
        let first = *self.remaining.first().ok_or(UndoCodecError::Syntax)?;
        if first & 0x80 == 0 {
            return self.ascii().map(Some);
        }
        self.byte()?;
        let size = self.flagged_value(first)?;
        if size == 0 {
            return Ok(None);
        }
        let count = (size - 1) as usize;
        self.check_size(count)?;
        if count > self.remaining.len() {
            return Err(UndoCodecError::Syntax);
        }
        let mut text = String::new();
        let mut units = 0;
        while units < count {
            let first = self.utf16_unit()?;
            units += 1;
            let ch = if (0xd800..=0xdbff).contains(&first) && units < count {
                units += 1;
                let second = self.utf16_unit()?;
                char::decode_utf16([first, second])
                    .next()
                    .ok_or(UndoCodecError::Syntax)?
                    .map_err(|_| UndoCodecError::Syntax)?
            } else {
                char::from_u32(u32::from(first)).ok_or(UndoCodecError::Syntax)?
            };
            if text.len() + ch.len_utf8() > self.limits.scalar {
                return Err(UndoCodecError::Budget {
                    limit: "Kryo scalar bytes",
                });
            }
            self.charge(ch.len_utf8())?;
            text.push(ch);
        }
        Ok(Some(text))
    }

    fn ascii(&mut self) -> Result<String, UndoCodecError> {
        let end = self
            .remaining
            .iter()
            .take(self.limits.scalar)
            .position(|byte| *byte & 0x80 != 0);
        let Some(end) = end else {
            return if self.remaining.len() > self.limits.scalar {
                Err(UndoCodecError::Budget {
                    limit: "Kryo scalar bytes",
                })
            } else {
                Err(UndoCodecError::Syntax)
            };
        };
        self.charge(end + 1)?;
        let mut text = self.bytes(end + 1)?.to_vec();
        text[end] &= 0x7f;
        String::from_utf8(text).map_err(|_| UndoCodecError::Syntax)
    }

    fn utf16_unit(&mut self) -> Result<u16, UndoCodecError> {
        let start = self.byte()?;
        match start {
            0..=0x7f => Ok(u16::from(start)),
            0xc2..=0xdf => Ok((u16::from(start & 0x1f) << 6) | self.utf_tail()?),
            0xe0..=0xef => {
                let unit =
                    (u16::from(start & 0x0f) << 12) | (self.utf_tail()? << 6) | self.utf_tail()?;
                if unit < 0x800 {
                    return Err(UndoCodecError::Syntax);
                }
                Ok(unit)
            }
            _ => Err(UndoCodecError::Syntax),
        }
    }

    fn utf_tail(&mut self) -> Result<u16, UndoCodecError> {
        match self.byte()? {
            value @ 0x80..=0xbf => Ok(u16::from(value & 0x3f)),
            _ => Err(UndoCodecError::Syntax),
        }
    }

    fn flagged_value(&mut self, first: u8) -> Result<u32, UndoCodecError> {
        let low = u32::from(first & 0x3f);
        if first & 0x40 == 0 {
            return Ok(low);
        }
        let mut number = low;
        for shift in [6, 13, 20, 27] {
            let byte = self.byte()?;
            if shift == 27 && byte >= 32 {
                return Err(UndoCodecError::Syntax);
            }
            number |= u32::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(number);
            }
        }
        Err(UndoCodecError::Syntax)
    }

    pub(super) fn i32(&mut self) -> Result<i32, UndoCodecError> {
        let number = self.u32()?;
        Ok(((number >> 1) as i32) ^ -((number & 1) as i32))
    }

    pub(super) fn i64(&mut self) -> Result<i64, UndoCodecError> {
        let number = self.u64()?;
        Ok(((number >> 1) as i64) ^ -((number & 1) as i64))
    }

    pub(super) fn u32(&mut self) -> Result<u32, UndoCodecError> {
        let mut number = 0;
        for shift in [0, 7, 14, 21, 28] {
            let byte = self.byte()?;
            if shift == 28 && byte >= 16 {
                return Err(UndoCodecError::Syntax);
            }
            number |= u32::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(number);
            }
        }
        Err(UndoCodecError::Syntax)
    }

    pub(super) fn u64(&mut self) -> Result<u64, UndoCodecError> {
        let mut number = 0;
        for shift in (0..56).step_by(7) {
            let byte = self.byte()?;
            number |= u64::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(number);
            }
        }
        Ok(number | (u64::from(self.byte()?) << 56))
    }

    pub(super) fn byte(&mut self) -> Result<u8, UndoCodecError> {
        Ok(self.bytes(1)?[0])
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], UndoCodecError> {
        let (bytes, rest) = self
            .remaining
            .split_at_checked(len)
            .ok_or(UndoCodecError::Syntax)?;
        self.remaining = rest;
        Ok(bytes)
    }

    pub(super) fn fixed64(&mut self) -> Result<u64, UndoCodecError> {
        let bytes = self
            .bytes(8)?
            .try_into()
            .map_err(|_| UndoCodecError::Syntax)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(super) fn numeric<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, UndoCodecError>,
    ) -> Result<T, UndoCodecError> {
        let start = self.remaining.len();
        let value = read(self)?;
        self.charge(start - self.remaining.len())?;
        Ok(value)
    }

    fn check_size(&self, len: usize) -> Result<(), UndoCodecError> {
        if len > self.limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "Kryo scalar bytes",
            });
        }
        if len > self.limits.total_scalar.saturating_sub(self.scalar_bytes) {
            return Err(UndoCodecError::Budget {
                limit: "Kryo total scalar bytes",
            });
        }
        Ok(())
    }

    fn charge(&mut self, len: usize) -> Result<(), UndoCodecError> {
        self.check_size(len)?;
        self.scalar_bytes += len;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), UndoCodecError> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(UndoCodecError::Syntax)
        }
    }
}
