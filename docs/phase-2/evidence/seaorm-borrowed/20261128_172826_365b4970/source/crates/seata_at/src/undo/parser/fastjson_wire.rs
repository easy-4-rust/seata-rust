use super::{strict_json::StrictJson, UndoCodecError, UndoCodecLimits};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;

pub(crate) const BINARY: &str = "@seata.fast.binary";
pub(crate) const LONG: &str = "@seata.fast.long";
pub(crate) const DECIMAL: &str = "@seata.fast.decimal";

/// 将有限的 Fastjson 原生字面量投影到严格 JSON，保留非 JSON 标记的来源。
/// 对应 Java: FastjsonUndoLogParser#decode 的 WriteClassName 字面量边界。
pub(crate) struct FastjsonWire;
impl FastjsonWire {
    pub(crate) fn parse(
        bytes: &[u8],
        limits: UndoCodecLimits,
    ) -> Result<(Value, usize), UndoCodecError> {
        if bytes.len() > limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        let mut remaining = bytes;
        let mut output = Vec::with_capacity(bytes.len());
        while let Some(&first) = remaining.first() {
            let consumed = match first {
                b'"' => Self::string(&mut output, remaining)?,
                b'x' if remaining.starts_with(b"x'") => {
                    Self::binary(&mut output, remaining, limits)?
                }
                b'-' | b'0'..=b'9' => Self::number(&mut output, remaining, limits)?,
                _ => {
                    output.push(first);
                    1
                }
            };
            if output.len() > limits.encoded {
                return Err(UndoCodecError::Budget {
                    limit: "normalized Fastjson bytes",
                });
            }
            remaining = &remaining[consumed..];
        }
        Ok((StrictJson::parse(&output, limits)?, output.len()))
    }

    fn string(output: &mut Vec<u8>, input: &[u8]) -> Result<usize, UndoCodecError> {
        let mut escaped = false;
        for (index, &byte) in input.iter().enumerate().skip(1) {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let end = index + 1;
                if input[end..].iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b':') {
                    let name: String = serde_json::from_slice(&input[..end])
                        .map_err(|_| UndoCodecError::Syntax)?;
                    if matches!(name.as_str(), BINARY | LONG | DECIMAL) {
                        return Err(UndoCodecError::Shape {
                            reason: "reserved Fastjson literal marker",
                        });
                    }
                }
                output.extend_from_slice(&input[..end]);
                return Ok(end);
            }
        }
        Err(UndoCodecError::Syntax)
    }

    fn binary(
        output: &mut Vec<u8>,
        input: &[u8],
        limits: UndoCodecLimits,
    ) -> Result<usize, UndoCodecError> {
        let hex = &input[2..];
        let end = hex
            .iter()
            .position(|byte| *byte == b'\'')
            .ok_or(UndoCodecError::Syntax)?;
        if end % 2 != 0 {
            return Err(UndoCodecError::Syntax);
        }
        if end / 2 > limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "Fastjson binary bytes",
            });
        }
        let mut bytes = Vec::with_capacity(end / 2);
        for pair in hex[..end].chunks_exact(2) {
            let nibble = |byte: u8| char::from(byte).to_digit(16).ok_or(UndoCodecError::Syntax);
            bytes.push((nibble(pair[0])? * 16 + nibble(pair[1])?) as u8);
        }
        Self::tag(output, BINARY, STANDARD.encode(bytes).as_bytes(), true);
        Ok(end + 3)
    }

    fn number(
        output: &mut Vec<u8>,
        input: &[u8],
        limits: UndoCodecLimits,
    ) -> Result<usize, UndoCodecError> {
        let end = input
            .iter()
            .take_while(|&&byte| matches!(byte, b'0'..=b'9' | b'-' | b'+' | b'e' | b'E' | b'.'))
            .count();
        let token = &input[..end];
        if end > limits.scalar {
            return Err(UndoCodecError::Budget {
                limit: "Fastjson number bytes",
            });
        }
        if input.get(end) == Some(&b'L') {
            if !Self::integer_token(token) {
                return Err(UndoCodecError::Syntax);
            }
            Self::tag(output, LONG, token, false);
            Ok(end + 1)
        } else if let Some(whole) = token.strip_suffix(b".") {
            if !Self::integer_token(whole) {
                return Err(UndoCodecError::Syntax);
            }
            Self::tag(output, DECIMAL, whole, false);
            Ok(end)
        } else {
            output.extend_from_slice(token);
            Ok(end)
        }
    }

    fn integer_token(value: &[u8]) -> bool {
        let digits = value.strip_prefix(b"-").unwrap_or(value);
        !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
    }

    fn tag(output: &mut Vec<u8>, key: &str, value: &[u8], quote: bool) {
        output.extend_from_slice(b"{\"");
        output.extend_from_slice(key.as_bytes());
        output.extend_from_slice(b"\":");
        if quote {
            output.push(b'"');
        }
        output.extend_from_slice(value);
        if quote {
            output.push(b'"');
        }
        output.push(b'}');
    }
}
