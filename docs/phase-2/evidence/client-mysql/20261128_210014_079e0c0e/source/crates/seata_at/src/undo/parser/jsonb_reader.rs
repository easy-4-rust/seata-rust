use super::{
    jsonb_value::JsonbValue, restored_format::RestoredFormat, UndoCodecError, UndoCodecLimits,
};
use num_bigint::BigInt;
use std::collections::BTreeMap;

/// Fastjson2 JSONB 文档的有界读取器，解析局部符号，保留类型名作为数据。
/// 对应 Java: com.alibaba.fastjson2.JSONReaderJSONB 的 Undo 值域。
pub(super) struct JsonbReader<'a> {
    remaining: &'a [u8],
    symbols: Vec<String>,
    limits: UndoCodecLimits,
    nodes: usize,
    scalar_bytes: usize,
}

impl<'a> JsonbReader<'a> {
    pub(super) fn parse(
        bytes: &'a [u8],
        limits: UndoCodecLimits,
    ) -> Result<JsonbValue, UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        let mut reader = Self {
            remaining: bytes,
            symbols: Vec::new(),
            limits,
            nodes: 0,
            scalar_bytes: 0,
        };
        let root = reader.value(0)?;
        if !reader.remaining.is_empty() {
            return Err(UndoCodecError::Syntax);
        }
        Ok(root)
    }

    fn value(&mut self, depth: usize) -> Result<JsonbValue, UndoCodecError> {
        self.nodes += 1;
        let token = self.take()?;
        let container = matches!(token, 0x92 | 0x94..=0xa4 | 0xa6);
        if self.nodes > self.limits.nodes || depth + usize::from(container) > self.limits.depth {
            return Err(UndoCodecError::Budget {
                limit: "JSONB nodes/depth",
            });
        }
        match token {
            0x92 => {
                let class = self.symbol()?;
                if class == "[B" {
                    self.expect(0x91)?;
                    let len = self.len()?;
                    self.charge(len)?;
                    return Ok(JsonbValue::Binary(self.bytes(len)?.to_vec()));
                }
                self.expect(0xa6)?;
                self.object(depth, Some(class))
            }
            0xa6 => self.object(depth, None),
            0x94..=0xa3 => self.list(depth, usize::from(token - 0x94)),
            0xa4 => {
                let len = self.len()?;
                self.list(depth, len)
            }
            0xaf => Ok(JsonbValue::Null),
            0xb0 | 0xb1 => {
                self.charge(1)?;
                Ok(JsonbValue::Bool(token == 0xb1))
            }
            73..=125 => Ok(JsonbValue::Text(self.text(token)?)),
            0xb8..=0xbb | 190..=239 | 0..=72 | 240..=255 => self.number(token),
            _ => Err(UndoCodecError::Unsupported {
                kind: "JSONB token",
            }),
        }
    }

    fn number(&mut self, token: u8) -> Result<JsonbValue, UndoCodecError> {
        let available = self.remaining.len();
        let value = match token {
            0xb8 | 0xb9 => self.decimal(token).map(JsonbValue::Decimal),
            0xba | 0xbb => u64::try_from(self.big_integer(token)?)
                .map(JsonbValue::BigInteger)
                .map_err(|_| UndoCodecError::Unsupported {
                    kind: "JSONB BigInteger outside MySQL unsigned range",
                }),
            190..=239 => Ok(JsonbValue::Long(self.long(token)?)),
            _ => Ok(JsonbValue::Int(self.integer(token)?)),
        }?;
        // 紧凑标记本身携带数值；按实际消耗字节统一计入单值和总量预算。
        self.charge(1 + available - self.remaining.len())?;
        Ok(value)
    }

    fn object(
        &mut self,
        depth: usize,
        class: Option<String>,
    ) -> Result<JsonbValue, UndoCodecError> {
        let mut fields = BTreeMap::new();
        while self.remaining.first() != Some(&0xa5) {
            self.expect(0x7f)?;
            let name = self.symbol()?;
            let value = self.value(depth + 1)?;
            if fields.insert(name, value).is_some() {
                return Err(UndoCodecError::Shape {
                    reason: "duplicate JSONB field",
                });
            }
        }
        self.expect(0xa5)?;
        Ok(JsonbValue::Object(class, fields))
    }

    fn list(&mut self, depth: usize, len: usize) -> Result<JsonbValue, UndoCodecError> {
        if len > self.limits.nodes.saturating_sub(self.nodes) {
            return Err(UndoCodecError::Budget {
                limit: "JSONB nodes/depth",
            });
        }
        if len > self.remaining.len() {
            return Err(UndoCodecError::Syntax);
        }
        (0..len)
            .map(|_| self.value(depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(JsonbValue::List)
    }

    fn symbol(&mut self) -> Result<String, UndoCodecError> {
        let token = self.take()?;
        if (73..=125).contains(&token) {
            let text = self.text(token)?;
            let index = self.len()?;
            if index != self.symbols.len() {
                return Err(UndoCodecError::Shape {
                    reason: "JSONB symbol declaration order",
                });
            }
            self.symbols.push(text.clone());
            return Ok(text);
        }
        let index = usize::try_from(self.integer(token)?).map_err(|_| UndoCodecError::Syntax)?;
        let len = self
            .symbols
            .get(index)
            .ok_or(UndoCodecError::Shape {
                reason: "unknown JSONB symbol",
            })?
            .len();
        self.charge(len)?;
        Ok(self.symbols[index].clone())
    }

    fn text(&mut self, token: u8) -> Result<String, UndoCodecError> {
        let len = if token <= 120 {
            usize::from(token - 73)
        } else {
            self.len()?
        };
        let bytes = self.bytes(len)?;
        match token {
            73..=121 if bytes.is_ascii() => {
                self.charge(len)?;
                Ok(std::str::from_utf8(bytes)
                    .map_err(|_| UndoCodecError::Syntax)?
                    .to_owned())
            }
            73..=121 => {
                self.charge(len + bytes.iter().filter(|byte| **byte >= 128).count())?;
                Ok(bytes.iter().map(|byte| char::from(*byte)).collect())
            }
            122 => {
                self.charge(len)?;
                Ok(std::str::from_utf8(bytes)
                    .map_err(|_| UndoCodecError::Syntax)?
                    .to_owned())
            }
            124 | 125 if len % 2 == 0 => {
                let units = bytes.chunks_exact(2).map(|x| {
                    if token == 124 {
                        u16::from_le_bytes([x[0], x[1]])
                    } else {
                        u16::from_be_bytes([x[0], x[1]])
                    }
                });
                let mut text = String::new();
                for ch in char::decode_utf16(units) {
                    let ch = ch.map_err(|_| UndoCodecError::Syntax)?;
                    if text.len() + ch.len_utf8() > self.limits.scalar {
                        return Err(UndoCodecError::Budget {
                            limit: "JSONB scalar bytes",
                        });
                    }
                    self.charge_total(ch.len_utf8())?;
                    text.push(ch);
                }
                Ok(text)
            }
            _ => Err(UndoCodecError::Unsupported {
                kind: "JSONB string encoding",
            }),
        }
    }

    fn charge(&mut self, len: usize) -> Result<(), UndoCodecError> {
        if len > self.limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "JSONB scalar bytes",
            });
        }
        self.charge_total(len)
    }

    fn charge_total(&mut self, len: usize) -> Result<(), UndoCodecError> {
        self.scalar_bytes += len;
        if self.scalar_bytes > self.limits.total_scalar {
            return Err(UndoCodecError::Budget {
                limit: "JSONB total scalar bytes",
            });
        }
        Ok(())
    }

    fn integer(&mut self, token: u8) -> Result<i32, UndoCodecError> {
        match token {
            0..=47 | 240..=255 => Ok(i32::from(token as i8)),
            48..=63 => Ok(((i32::from(token) - 56) << 8) | i32::from(self.take()?)),
            64..=71 => Ok(((i32::from(token) - 68) << 16) | i32::from(self.u16()?)),
            72 => Ok(i32::from_be_bytes(self.fixed()?)),
            _ => Err(UndoCodecError::Shape {
                reason: "JSONB int32",
            }),
        }
    }

    fn long(&mut self, token: u8) -> Result<i64, UndoCodecError> {
        match token {
            216..=239 => Ok(i64::from(token) - 224),
            200..=215 => Ok(((i64::from(token) - 208) << 8) | i64::from(self.take()?)),
            192..=199 => Ok(((i64::from(token) - 196) << 16) | i64::from(self.u16()?)),
            191 => Ok(i64::from(i32::from_be_bytes(self.fixed()?))),
            190 => Ok(i64::from_be_bytes(self.fixed()?)),
            _ => Err(UndoCodecError::Shape {
                reason: "JSONB int64",
            }),
        }
    }

    fn big_integer(&mut self, token: u8) -> Result<BigInt, UndoCodecError> {
        if token == 0xba {
            let token = self.take()?;
            return self.long(token).map(BigInt::from);
        }
        let len = self.len()?;
        // 65 位十进制系数最多占用 28 个带符号字节。
        if len == 0 {
            return Err(UndoCodecError::Syntax);
        }
        if len > 28 {
            return Err(UndoCodecError::Budget {
                limit: "JSONB BigInteger bytes",
            });
        }
        Ok(BigInt::from_signed_bytes_be(self.bytes(len)?))
    }

    fn decimal(&mut self, token: u8) -> Result<seata_datasource::DecimalValue, UndoCodecError> {
        let (scale, coefficient) = if token == 0xb8 {
            let token = self.take()?;
            (0, BigInt::from(self.long(token)?))
        } else {
            let scale_token = self.take()?;
            let scale = self.integer(scale_token)?;
            let coefficient_token = self.take()?;
            let coefficient = match coefficient_token {
                0xba | 0xbb => self.big_integer(coefficient_token)?,
                190..=239 => BigInt::from(self.long(coefficient_token)?),
                _ => BigInt::from(self.integer(coefficient_token)?),
            };
            (scale, coefficient)
        };
        if !(0..=30).contains(&scale) {
            return Err(UndoCodecError::Shape {
                reason: "JSONB physical decimal scale",
            });
        }
        RestoredFormat::decimal(&format!("{coefficient}e-{scale}"))
    }

    fn u16(&mut self) -> Result<u16, UndoCodecError> {
        Ok(u16::from_be_bytes(self.fixed()?))
    }

    fn len(&mut self) -> Result<usize, UndoCodecError> {
        let token = self.take()?;
        usize::try_from(self.integer(token)?).map_err(|_| UndoCodecError::Syntax)
    }

    fn expect(&mut self, expected: u8) -> Result<(), UndoCodecError> {
        if self.take()? == expected {
            Ok(())
        } else {
            Err(UndoCodecError::Syntax)
        }
    }

    fn take(&mut self) -> Result<u8, UndoCodecError> {
        Ok(self.fixed::<1>()?[0])
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], UndoCodecError> {
        self.bytes(N)?
            .try_into()
            .map_err(|_| UndoCodecError::Syntax)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], UndoCodecError> {
        let (value, remain) = self
            .remaining
            .split_at_checked(len)
            .ok_or(UndoCodecError::Syntax)?;
        self.remaining = remain;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonbReader, UndoCodecError, UndoCodecLimits};

    #[test]
    fn compact_numbers_are_charged_to_the_same_scalar_budget() -> Result<(), UndoCodecError> {
        // 两个 BigDecimal(long 42) 各消费 3 字节，数组标记属于结构。
        let bytes = [0x96, 0xb8, 0xd0, 42, 0xb8, 0xd0, 42];
        let limits = UndoCodecLimits::new(7, 3, 1, 3, 5)?;
        assert!(matches!(
            JsonbReader::parse(&bytes, limits),
            Err(UndoCodecError::Budget {
                limit: "JSONB total scalar bytes"
            })
        ));
        let limits = UndoCodecLimits::new(7, 3, 1, 3, 6)?;
        JsonbReader::parse(&bytes, limits)?;
        Ok(())
    }
}
