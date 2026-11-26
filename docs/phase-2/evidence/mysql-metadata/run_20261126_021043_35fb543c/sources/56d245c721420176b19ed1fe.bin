use super::{
    protostuff_value::{ProtostuffFields, ProtostuffValue},
    UndoCodecError, UndoCodecLimits,
};

/// 有界 Protostuff 分组读取器，解析前检查分组深度、长度及各字段标记。
/// 对应 Java: io.protostuff.ByteArrayInput。
pub(super) struct ProtostuffReader<'a> {
    remaining: &'a [u8],
    limits: UndoCodecLimits,
    nodes: usize,
    scalar_bytes: usize,
}

impl<'a> ProtostuffReader<'a> {
    pub(super) fn parse(
        bytes: &'a [u8],
        limits: UndoCodecLimits,
    ) -> Result<ProtostuffFields<'a>, UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        Self {
            remaining: bytes,
            limits,
            nodes: 0,
            scalar_bytes: 0,
        }
        .fields(None, 1)
    }

    fn fields(
        &mut self,
        end: Option<u32>,
        depth: usize,
    ) -> Result<ProtostuffFields<'a>, UndoCodecError> {
        if depth > self.limits.depth {
            return Err(UndoCodecError::Budget {
                limit: "Protostuff nodes/depth",
            });
        }
        let mut fields = ProtostuffFields::default();
        while !self.remaining.is_empty() {
            let tag = u32::try_from(self.varint()?).map_err(|_| UndoCodecError::Syntax)?;
            let (number, kind) = (tag >> 3, tag & 7);
            if number == 0 {
                return Err(UndoCodecError::Syntax);
            }
            if kind == 4 {
                return if end == Some(number) {
                    Ok(fields)
                } else {
                    Err(UndoCodecError::Syntax)
                };
            }
            self.nodes += 1;
            if self.nodes > self.limits.nodes {
                return Err(UndoCodecError::Budget {
                    limit: "Protostuff nodes/depth",
                });
            }
            let value = match kind {
                0 => {
                    let start = self.remaining.len();
                    let value = self.varint()?;
                    self.charge(start - self.remaining.len())?;
                    ProtostuffValue::Varint(value)
                }
                1 => {
                    self.charge(8)?;
                    let bytes = self
                        .bytes(8)?
                        .try_into()
                        .map_err(|_| UndoCodecError::Syntax)?;
                    ProtostuffValue::Fixed64(u64::from_le_bytes(bytes))
                }
                2 => {
                    let len =
                        usize::try_from(self.varint()?).map_err(|_| UndoCodecError::Syntax)?;
                    let bytes = self.bytes(len)?;
                    self.charge(len)?;
                    ProtostuffValue::Bytes(bytes)
                }
                3 => ProtostuffValue::Group(self.fields(Some(number), depth + 1)?),
                _ => {
                    return Err(UndoCodecError::Unsupported {
                        kind: "Protostuff wire type",
                    })
                }
            };
            fields.first.get_or_insert(number);
            fields.members.entry(number).or_default().push(value);
        }
        if end.is_some() {
            Err(UndoCodecError::Syntax)
        } else {
            Ok(fields)
        }
    }

    fn charge(&mut self, len: usize) -> Result<(), UndoCodecError> {
        if len > self.limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "Protostuff scalar bytes",
            });
        }
        self.scalar_bytes += len;
        if self.scalar_bytes > self.limits.total_scalar {
            return Err(UndoCodecError::Budget {
                limit: "Protostuff total scalar bytes",
            });
        }
        Ok(())
    }

    fn varint(&mut self) -> Result<u64, UndoCodecError> {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = self.bytes(1)?[0];
            if shift == 63 && byte > 1 {
                return Err(UndoCodecError::Syntax);
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 128 {
                return Ok(value);
            }
        }
        Err(UndoCodecError::Syntax)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], UndoCodecError> {
        let (bytes, rest) = self
            .remaining
            .split_at_checked(len)
            .ok_or(UndoCodecError::Syntax)?;
        self.remaining = rest;
        Ok(bytes)
    }
}
