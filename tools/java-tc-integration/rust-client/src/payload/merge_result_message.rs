// Copyright 2026 Seata-Rust contributors.
// SPDX-License-Identifier: Apache-2.0

use super::payload_message::PayloadMessage;

/// 依原请求顺序排列的合并结果；对应 Java: org.apache.seata.core.protocol.MergeResultMessage。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MergeResultMessage {
    pub(super) msgs: Vec<PayloadMessage>,
}
