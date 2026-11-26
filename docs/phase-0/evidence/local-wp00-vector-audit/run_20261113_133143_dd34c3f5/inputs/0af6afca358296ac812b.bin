// Copyright 2026 Seata-Rust contributors.
// SPDX-License-Identifier: Apache-2.0

use super::control_message::ControlMessage;

/// 合并请求及可省略的子请求 ID 列表；对应 Java: org.apache.seata.core.protocol.MergedWarpMessage。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MergedWarpMessage {
    pub(super) msgs: Vec<ControlMessage>,
    pub(super) msg_ids: Vec<i32>,
}
