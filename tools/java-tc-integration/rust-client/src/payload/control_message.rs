// Copyright 2026 Seata-Rust contributors.
// SPDX-License-Identifier: Apache-2.0

use super::merge_result_message::MergeResultMessage;
use super::merged_warp_message::MergedWarpMessage;
use super::payload_message::PayloadMessage;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 独立测试工具的合并／普通消息分派；对应 Java: AbstractMessage / SeataSerializerV1。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ControlMessage {
    Payload(Box<PayloadMessage>),
    MergedWarp(MergedWarpMessage),
    MergeResult(MergeResultMessage),
}

impl ControlMessage {
    fn visit(depth: usize, budget: &mut usize) -> io::Result<()> {
        if depth > 8 || *budget == 0 {
            return Err(crate::invalid("control nesting/message budget exceeded"));
        }
        *budget -= 1;
        Ok(())
    }
    fn count(reader: &mut WireReader<'_>) -> io::Result<usize> {
        let count =
            usize::try_from(reader.i16()?).map_err(|_| crate::invalid("negative merge count"))?;
        if count > 1024 {
            return Err(crate::invalid("merge count exceeds test limit"));
        }
        Ok(count)
    }
    fn result_type(code: u16) -> bool {
        matches!(
            code,
            2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 18 | 22 | 102 | 104
        )
    }
    fn read(reader: &mut WireReader<'_>, depth: usize, budget: &mut usize) -> io::Result<Self> {
        Self::visit(depth, budget)?;
        let code = reader.peek_u16()?;
        if code != 59 && code != 60 {
            return PayloadMessage::read_prefix(reader).map(|m| Self::Payload(Box::new(m)));
        }
        reader.u16()?;
        let length =
            usize::try_from(reader.i32()?).map_err(|_| crate::invalid("negative merge length"))?;
        let mut content = WireReader::new(reader.take(length)?)?;
        let count = Self::count(&mut content)?;
        if code == 59 {
            let mut msgs = Vec::new();
            for _ in 0..count {
                msgs.push(Self::read(&mut content, depth + 1, budget)?);
            }
            let mut msg_ids = Vec::new();
            if content.remaining() != 0 {
                if content.remaining() != count * 4 {
                    return Err(crate::invalid("partial or extra merged request IDs"));
                }
                for _ in 0..count {
                    msg_ids.push(content.i32()?);
                }
            }
            content.finish()?;
            Ok(Self::MergedWarp(MergedWarpMessage { msgs, msg_ids }))
        } else {
            let mut msgs = Vec::new();
            for _ in 0..count {
                if !Self::result_type(content.peek_u16()?) {
                    return Err(crate::invalid("non-result in MergeResultMessage"));
                }
                Self::visit(depth + 1, budget)?;
                msgs.push(PayloadMessage::read_prefix(&mut content)?);
            }
            content.finish()?;
            Ok(Self::MergeResult(MergeResultMessage { msgs }))
        }
    }
    /// 输入完整有类型消息，返回普通或合并对象；长度／递归／总消息数均受限。
    pub(super) fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut reader = WireReader::new(bytes)?;
        let message = Self::read(&mut reader, 0, &mut 2048)?;
        reader.finish()?;
        Ok(message)
    }
    fn write(&self, writer: &mut WireWriter, depth: usize, budget: &mut usize) -> io::Result<()> {
        Self::visit(depth, budget)?;
        match self {
            Self::Payload(message) => writer.raw(&message.encode()?),
            Self::MergedWarp(message) => {
                if message.msgs.len() > 1024
                    || (!message.msg_ids.is_empty() && message.msg_ids.len() != message.msgs.len())
                {
                    return Err(crate::invalid("invalid merge message/ID count"));
                }
                let mut content = WireWriter::default();
                content.i16(message.msgs.len() as i16)?;
                for child in &message.msgs {
                    child.write(&mut content, depth + 1, budget)?;
                }
                for id in &message.msg_ids {
                    content.i32(*id)?;
                }
                let bytes = content.finish();
                writer.u16(59)?;
                writer.i32(bytes.len() as i32)?;
                writer.raw(&bytes)
            }
            Self::MergeResult(message) => {
                if message.msgs.len() > 1024 {
                    return Err(crate::invalid("merge result count exceeded"));
                }
                let mut content = WireWriter::default();
                content.i16(message.msgs.len() as i16)?;
                for child in &message.msgs {
                    Self::visit(depth + 1, budget)?;
                    let bytes = child.encode()?;
                    if !Self::result_type(u16::from_be_bytes([bytes[0], bytes[1]])) {
                        return Err(crate::invalid("non-result in MergeResultMessage"));
                    }
                    content.raw(&bytes)?;
                }
                let bytes = content.finish();
                writer.u16(60)?;
                writer.i32(bytes.len() as i32)?;
                writer.raw(&bytes)
            }
        }
    }
    /// 根据类型字段重新编码，返回字节；不缓存原始消息，编码也限制深度和总节点。
    pub(super) fn encode(&self) -> io::Result<Vec<u8>> {
        let mut writer = WireWriter::default();
        self.write(&mut writer, 0, &mut 2048)?;
        Ok(writer.finish())
    }
}
