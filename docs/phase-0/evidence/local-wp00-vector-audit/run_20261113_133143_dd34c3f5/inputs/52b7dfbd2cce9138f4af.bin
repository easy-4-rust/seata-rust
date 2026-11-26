// Copyright 2026 Seata-Rust contributors.
// SPDX-License-Identifier: Apache-2.0

use super::control_message::ControlMessage;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::collections::HashSet;
use std::io;

/// 测试用完整 V1 控制帧，区分无体心跳；对应 Java: ProtocolRpcMessageV1 / ProtocolDecoderV1。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ControlFrame {
    pub(super) request_id: u32,
    pub(super) kind: u8,
    pub(super) head_map: Vec<(String, Option<String>)>,
    pub(super) body: Option<ControlMessage>,
}

impl ControlFrame {
    /// 输入完整帧，返回严格模型；不用于生产 TCP 流处理。
    pub(super) fn decode(frame: &[u8]) -> io::Result<Self> {
        if frame.len() < 16 || frame.len() > crate::MAX_FRAME_LEN || frame[..3] != [0xda, 0xda, 1] {
            return Err(crate::invalid("invalid complete V1 frame"));
        }
        let full = u32::from_be_bytes(frame[3..7].try_into().unwrap()) as usize;
        let head = u16::from_be_bytes(frame[7..9].try_into().unwrap()) as usize;
        let kind = frame[9];
        if full != frame.len()
            || !(16..=full.min(i16::MAX as usize)).contains(&head)
            || kind > 4
            || frame[10..12] != [1, 0]
        {
            return Err(crate::invalid("unsupported V1 envelope lengths/type/codec"));
        }
        let mut headers = WireReader::new(&frame[16..head])?;
        let mut head_map = Vec::new();
        let mut keys = HashSet::new();
        while headers.remaining() > 0 {
            let key = Self::header_text(&mut headers)?
                .ok_or_else(|| crate::invalid("null header key cannot re-encode"))?;
            if !keys.insert(key.clone()) {
                return Err(crate::invalid("duplicate header key"));
            }
            head_map.push((key, Self::header_text(&mut headers)?));
        }
        headers.finish()?;
        let body = if kind == 3 || kind == 4 {
            if head != full {
                return Err(crate::invalid("heartbeat has unexpected body bytes"));
            }
            None
        } else {
            Some(ControlMessage::decode(&frame[head..])?)
        };
        Ok(Self {
            request_id: u32::from_be_bytes(frame[12..16].try_into().unwrap()),
            kind,
            head_map,
            body,
        })
    }
    fn header_text(reader: &mut WireReader<'_>) -> io::Result<Option<String>> {
        let length = reader.i16()?;
        if length == -1 {
            return Ok(None);
        }
        let length = usize::try_from(length)
            .map_err(|_| crate::invalid("invalid negative header string length"))?;
        std::str::from_utf8(reader.take(length)?)
            .map(|s| Some(s.to_owned()))
            .map_err(|_| crate::invalid("invalid header UTF8"))
    }
    /// 根据对象字段重新编码完整帧，返回字节或不合法模型错误。
    pub(super) fn encode(&self) -> io::Result<Vec<u8>> {
        if self.kind > 4 {
            return Err(crate::invalid("invalid envelope kind"));
        }
        let heartbeat = self.kind == 3 || self.kind == 4;
        if heartbeat != self.body.is_none() {
            return Err(crate::invalid("heartbeat/body presence differs"));
        }
        let mut writer = WireWriter::default();
        let mut keys = HashSet::new();
        for (key, value) in &self.head_map {
            if !keys.insert(key) {
                return Err(crate::invalid("duplicate header key"));
            }
            writer.string16(key)?;
            match value {
                Some(value) => writer.string16(value)?,
                None => writer.i16(-1)?,
            }
        }
        let headers = writer.finish();
        let head = 16 + headers.len();
        let body = match &self.body {
            Some(body) => body.encode()?,
            None => Vec::new(),
        };
        let full = head + body.len();
        if head > i16::MAX as usize || full > crate::MAX_FRAME_LEN {
            return Err(crate::invalid("frame/header exceeds test limits"));
        }
        let mut out = vec![0xda, 0xda, 1];
        out.extend_from_slice(&(full as u32).to_be_bytes());
        out.extend_from_slice(&(head as u16).to_be_bytes());
        out.extend_from_slice(&[self.kind, 1, 0]);
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.extend(headers);
        out.extend(body);
        Ok(out)
    }
}
