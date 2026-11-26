//! 独立跨语言验证入口：调用正式组件，导出全部字段树及新编码字节；不参与产品API。
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::AbstractTransactionResponse;
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1, ProtocolEncoderV1};
use seata_protocol::{Message, PayloadCodec, PayloadLimits, ProtocolError};
use std::error::Error;
use std::fmt::Display;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn unhex(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !text.is_ascii() || text.len() % 2 != 0 {
        return Err("invalid input hex".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(Into::into))
        .collect()
}
fn text(value: &str) -> String {
    format!("\"{}\"", hex(value.as_bytes()))
}
fn opt(value: Option<&str>) -> String {
    value.map(text).unwrap_or_else(|| "null".into())
}
fn num(value: impl Display) -> String {
    value.to_string()
}
fn array(values: Vec<String>) -> String {
    format!("[{}]", values.join(","))
}
fn prefix(value: &AbstractTransactionResponse) -> Vec<String> {
    vec![
        num(value.result_code.to_java_v1()),
        opt(value.msg.as_deref()),
        num(value.transaction_exception_code.to_java_v1()),
    ]
}
fn with(mut values: Vec<String>, tail: Vec<String>) -> Vec<String> {
    values.extend(tail);
    values
}

// 字段顺序由独立验证schema固定；字符串使用hex避免日志转义及语言文本表示差异。
fn semantic(message: &Message) -> String {
    let fields = match message {
        Message::GlobalBeginRequest(v) => vec![num(v.timeout), opt(v.transaction_name.as_deref())],
        Message::GlobalBeginResponse(v) => with(
            prefix(&v.response),
            vec![opt(v.xid.as_deref()), opt(v.extra_data.as_deref())],
        ),
        Message::GlobalCommitRequest(v) => {
            vec![opt(v.xid.as_deref()), opt(v.extra_data.as_deref())]
        }
        Message::GlobalRollbackRequest(v) => {
            vec![opt(v.xid.as_deref()), opt(v.extra_data.as_deref())]
        }
        Message::GlobalStatusRequest(v) => {
            vec![opt(v.xid.as_deref()), opt(v.extra_data.as_deref())]
        }
        Message::GlobalReportRequest(v) => vec![
            opt(v.xid.as_deref()),
            opt(v.extra_data.as_deref()),
            v.global_status
                .map(|s| num(s.to_java_v1()))
                .unwrap_or_else(|| "null".into()),
        ],
        Message::GlobalCommitResponse(v) => {
            with(prefix(&v.response), vec![num(v.global_status.to_java_v1())])
        }
        Message::GlobalRollbackResponse(v) => {
            with(prefix(&v.response), vec![num(v.global_status.to_java_v1())])
        }
        Message::GlobalStatusResponse(v) => {
            with(prefix(&v.response), vec![num(v.global_status.to_java_v1())])
        }
        Message::GlobalReportResponse(v) => {
            with(prefix(&v.response), vec![num(v.global_status.to_java_v1())])
        }
        Message::BranchCommitRequest(v) => vec![
            text(&v.xid),
            num(v.branch_id),
            num(v.branch_type.to_java_v1()),
            text(&v.resource_id),
            opt(v.application_data.as_deref()),
        ],
        Message::BranchRollbackRequest(v) => vec![
            text(&v.xid),
            num(v.branch_id),
            num(v.branch_type.to_java_v1()),
            text(&v.resource_id),
            opt(v.application_data.as_deref()),
        ],
        Message::BranchCommitResponse(v) => with(
            prefix(&v.response),
            vec![
                opt(v.xid.as_deref()),
                num(v.branch_id),
                num(v.branch_status.to_java_v1()),
            ],
        ),
        Message::BranchRollbackResponse(v) => with(
            prefix(&v.response),
            vec![
                opt(v.xid.as_deref()),
                num(v.branch_id),
                num(v.branch_status.to_java_v1()),
            ],
        ),
        Message::BranchRegisterRequest(v) => vec![
            opt(v.xid.as_deref()),
            num(v.branch_type.to_java_v1()),
            opt(v.resource_id.as_deref()),
            opt(v.lock_key.as_deref()),
            opt(v.application_data.as_deref()),
        ],
        Message::GlobalLockQueryRequest(v) => vec![
            opt(v.xid.as_deref()),
            num(v.branch_type.to_java_v1()),
            opt(v.resource_id.as_deref()),
            opt(v.lock_key.as_deref()),
            opt(v.application_data.as_deref()),
        ],
        Message::BranchRegisterResponse(v) => with(prefix(&v.response), vec![num(v.branch_id)]),
        Message::BranchReportRequest(v) => vec![
            opt(v.xid.as_deref()),
            num(v.branch_id),
            num(v.status.to_java_v1()),
            opt(v.resource_id.as_deref()),
            opt(v.application_data.as_deref()),
            num(v.branch_type.to_java_v1()),
        ],
        Message::BranchReportResponse(v) => prefix(&v.response),
        Message::GlobalLockQueryResponse(v) => with(prefix(&v.response), vec![num(v.lockable)]),
        Message::RegisterTMRequest(v) => vec![
            text(&v.version),
            text(&v.application_id),
            text(&v.transaction_service_group),
            text(&v.extra_data),
        ],
        Message::RegisterRMRequest(v) => vec![
            text(&v.version),
            opt(v.application_id.as_deref()),
            text(&v.transaction_service_group),
            opt(v.extra_data.as_deref()),
            opt(v.resource_ids.as_deref()),
        ],
        Message::RegisterTMResponse(v) => vec![
            num(v.identified),
            opt(v.wire_version.as_deref()),
            text(v.java_version("2.8.0-SNAPSHOT")),
        ],
        Message::RegisterRMResponse(v) => vec![
            num(v.identified),
            opt(v.wire_version.as_deref()),
            text(v.java_version("2.8.0-SNAPSHOT")),
        ],
        Message::UndoLogDeleteRequest(v) => vec![
            num(v.branch_type.to_java_v1()),
            text(&v.resource_id),
            num(v.save_days),
        ],
        Message::MergedWarpMessage(v) => vec![
            array(v.msgs.iter().map(semantic).collect()),
            array(v.msg_ids.iter().map(num).collect()),
        ],
        Message::MergeResultMessage(v) => vec![v
            .msgs
            .as_ref()
            .map(|values| array(values.iter().map(semantic).collect()))
            .unwrap_or_else(|| "null".into())],
    };
    format!(
        "{{\"type\":{},\"fields\":{}}}",
        message.message_type().to_java_v1(),
        array(fields)
    )
}
fn rpc_semantic(message: &RpcMessage) -> String {
    let headers = array(
        message
            .head_map
            .iter()
            .map(|(k, v)| array(vec![text(k), opt(v.as_deref())]))
            .collect(),
    );
    format!(
        "{{\"id\":{},\"kind\":{},\"codec\":{},\"compressor\":{},\"headers\":{},\"body\":{}}}",
        message.id,
        message.message_type.to_java_v1(),
        message.codec.to_java_v1(),
        message.compressor.to_java_v1(),
        headers,
        message
            .body
            .as_ref()
            .map(semantic)
            .unwrap_or_else(|| "null".into())
    )
}

fn encoded_result(
    semantic: String,
    result: Result<Vec<u8>, ProtocolError>,
) -> (&'static str, String, String) {
    match result {
        Ok(bytes) => ("accepted", semantic, hex(&bytes)),
        Err(_) => ("encode_error", semantic, String::new()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let input = std::env::args_os()
        .nth(1)
        .ok_or("one input TSV path required")?;
    // 此处为与候选作比较的显式测试预算，不修改正式组件默认或声称各实现边界完全一致。
    let payload = PayloadLimits::new(8 * 1024 * 1024, 1024, 2048, 8)?;
    let frames = FrameLimits::new(8 * 1024 * 1024, 32767, (32767 - 16) / 4, payload)?;
    let codec = PayloadCodec::with_limits(payload);
    for line in std::fs::read_to_string(input)?.lines() {
        let cols: Vec<_> = line.split('\t').collect();
        if cols.len() != 3 {
            return Err("three TSV columns required".into());
        }
        let wire = unhex(cols[2])?;
        let result = match cols[1] {
            "payload" => codec
                .decode(&wire)
                .map(|value| encoded_result(semantic(&value), codec.encode(&value))),
            "frame" => ProtocolDecoderV1::new(frames).decode(&wire).map(|value| {
                encoded_result(
                    rpc_semantic(&value),
                    ProtocolEncoderV1::new(frames).encode(&value),
                )
            }),
            _ => return Err("unknown input kind".into()),
        };
        match result {
            Ok((status, fields, bytes)) => println!(
                "PEER_RESULT\t{}\t{}\t{}\t{}",
                cols[0], status, fields, bytes
            ),
            Err(_) => println!("PEER_RESULT\t{}\trejected\tnull\t", cols[0]),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::encoded_result;
    use seata_protocol::ProtocolError;

    #[test]
    fn encode_failure_after_successful_decode_is_not_normal_rejection() {
        let outcome = encoded_result(
            "{\"type\":1,\"fields\":[]}".into(),
            Err(ProtocolError::InvalidValue {
                field: "injected encoder error",
                reason: "successful decode must not hide encoder failure",
            }),
        );
        assert_eq!(outcome.0, "encode_error");
    }
}
