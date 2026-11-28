use super::{UndoCodecError, UndoCodecLimits};
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;
use serde_json::{value::RawValue, Map, Number, Value};
use std::{fmt, str::FromStr};

/// 先限深，再按原RawValue词法构建精确树；不用Value默认反序列化吞掉重复键或解释私有Number键。
/// 对应 Java: JacksonUndoLogParser的Rust严格JSON/资源边界扩展。
pub(crate) struct StrictJson {
    limits: UndoCodecLimits,
    nodes: usize,
    scalar_bytes: usize,
    failure: Option<UndoCodecError>,
}
impl StrictJson {
    pub(crate) fn validate(value: &Value, limits: UndoCodecLimits) -> Result<(), UndoCodecError> {
        Self {
            limits,
            nodes: 0,
            scalar_bytes: 0,
            failure: None,
        }
        .tree(value, 0)
    }
    fn tree(&mut self, value: &Value, depth: usize) -> Result<(), UndoCodecError> {
        // 与输入扫描统一为容器深度：根对象/数组占一层，scalar不另增一层。
        let depth = depth + usize::from(value.is_array() || value.is_object());
        self.nodes += 1;
        if self.nodes > self.limits.nodes || depth > self.limits.depth {
            return Err(UndoCodecError::Budget {
                limit: "JSON nodes/depth",
            });
        }
        match value {
            Value::String(value) => self.scalar(value.len()),
            Value::Number(value) => self.number(&value.to_string()),
            Value::Array(values) => {
                for value in values {
                    self.tree(value, depth)?;
                }
                Ok(())
            }
            Value::Object(values) => {
                for (key, value) in values {
                    self.scalar(key.len())?;
                    self.tree(value, depth)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn parse(bytes: &[u8], limits: UndoCodecLimits) -> Result<Value, UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        let mut depth = 0usize;
        let mut string = false;
        let mut escaped = false;
        // RawValue内部会跳过嵌套值，因此在交给它之前独立限制整条输入深度。
        for byte in bytes {
            if string {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    string = false;
                }
            } else {
                match byte {
                    b'"' => string = true,
                    b'{' | b'[' => {
                        depth += 1;
                        if depth > limits.depth {
                            return Err(UndoCodecError::Budget {
                                limit: "JSON depth",
                            });
                        }
                    }
                    b'}' | b']' => {
                        depth = depth.checked_sub(1).ok_or(UndoCodecError::Syntax)?;
                    }
                    _ => {}
                }
            }
        }
        if depth != 0 || string {
            return Err(UndoCodecError::Syntax);
        }
        let input = std::str::from_utf8(bytes).map_err(|_| UndoCodecError::Syntax)?;
        Self {
            limits,
            nodes: 0,
            scalar_bytes: 0,
            failure: None,
        }
        .value(input, 0)
    }
    fn scalar(&mut self, size: usize) -> Result<(), UndoCodecError> {
        self.scalar_bytes = self
            .scalar_bytes
            .checked_add(size)
            .ok_or(UndoCodecError::Budget {
                limit: "scalar sum overflow",
            })?;
        if size > self.limits.scalar || self.scalar_bytes > self.limits.total_scalar {
            return Err(UndoCodecError::Budget {
                limit: "scalar bytes",
            });
        }
        Ok(())
    }
    fn number(&mut self, text: &str) -> Result<(), UndoCodecError> {
        self.scalar(text.len())?;
        // 固定Jackson2.18.3的DEFAULT_MAX_NUM_LEN；包含尾数/指数数字，不计符号及小数点。
        if text.bytes().filter(u8::is_ascii_digit).count() > 1000 {
            return Err(UndoCodecError::Budget {
                limit: "Jackson number digits",
            });
        }
        Ok(())
    }
    fn value(&mut self, input: &str, depth: usize) -> Result<Value, UndoCodecError> {
        let text = input.trim_matches([' ', '\t', '\r', '\n']);
        let depth = depth + usize::from(matches!(text.as_bytes().first(), Some(b'{' | b'[')));
        self.nodes += 1;
        if self.nodes > self.limits.nodes || depth > self.limits.depth {
            return Err(UndoCodecError::Budget {
                limit: "JSON nodes/depth",
            });
        }
        let mut parser = serde_json::Deserializer::from_str(text);
        let result = match text.as_bytes().first().copied() {
            Some(b'{') => parser.deserialize_map(ObjectVisitor { state: self, depth }),
            Some(b'[') => parser.deserialize_seq(ArrayVisitor { state: self, depth }),
            Some(b'"') => {
                let value: String = serde::Deserialize::deserialize(&mut parser)
                    .map_err(|_| UndoCodecError::Syntax)?;
                self.scalar(value.len())?;
                return parser
                    .end()
                    .map(|_| Value::String(value))
                    .map_err(|_| UndoCodecError::Syntax);
            }
            _ => {
                let value = match text {
                    "null" => Value::Null,
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    _ => {
                        self.number(text)?;
                        Value::Number(Number::from_str(text).map_err(|_| UndoCodecError::Syntax)?)
                    }
                };
                return Ok(value);
            }
        };
        match result {
            Ok(value) => {
                parser.end().map_err(|_| UndoCodecError::Syntax)?;
                Ok(value)
            }
            Err(_) => Err(self.failure.take().unwrap_or(UndoCodecError::Syntax)),
        }
    }
}
struct ObjectVisitor<'a> {
    state: &'a mut StrictJson,
    depth: usize,
}
impl<'de> Visitor<'de> for ObjectVisitor<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                self.state.failure = Some(UndoCodecError::Shape {
                    reason: "duplicate JSON key",
                });
                return Err(A::Error::custom("duplicate key"));
            }
            if let Err(error) = self.state.scalar(key.len()) {
                self.state.failure = Some(error);
                return Err(A::Error::custom("key budget"));
            }
            let raw: &'de RawValue = map.next_value()?;
            let value = match self.state.value(raw.get(), self.depth) {
                Ok(value) => value,
                Err(error) => {
                    self.state.failure = Some(error);
                    return Err(A::Error::custom("nested value"));
                }
            };
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
struct ArrayVisitor<'a> {
    state: &'a mut StrictJson,
    depth: usize,
}
impl<'de> Visitor<'de> for ArrayVisitor<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(raw) = sequence.next_element::<&'de RawValue>()? {
            let value = match self.state.value(raw.get(), self.depth) {
                Ok(value) => value,
                Err(error) => {
                    self.state.failure = Some(error);
                    return Err(A::Error::custom("nested value"));
                }
            };
            values.push(value);
        }
        Ok(Value::Array(values))
    }
}
