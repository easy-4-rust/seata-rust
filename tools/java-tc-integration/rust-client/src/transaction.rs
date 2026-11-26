//! Test-only no-branch transaction checks; not a production TM or AT resource manager.
use std::io::{self, Read, Write};

#[derive(Debug, PartialEq)]
struct TransactionResult {
    success: bool,
    error_code: u8,
    message: Option<String>,
    status: u8,
}

/// 输入读取器、请求 ID 和预期响应类型，返回严格关联的响应体；不发送或重试请求。
/// 对应 Java: ProtocolDecoderV1 / AbstractNettyRemoting 的响应关联职责。
pub(super) fn read_payload<R: Read>(reader: &mut R, id: u32, kind: u16) -> io::Result<Vec<u8>> {
    let mut header = [0u8; super::HEADER_LEN];
    reader.read_exact(&mut header)?;
    let full = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    let head = u16::from_be_bytes([header[7], header[8]]) as usize;
    let actual_id = u32::from_be_bytes(header[12..16].try_into().unwrap());
    if header[..3] != [0xda, 0xda, 1]
        || !(super::HEADER_LEN + 2..=super::MAX_FRAME_LEN).contains(&full)
        || head != super::HEADER_LEN
        || header[9..12]
            != [
                super::RESPONSE,
                super::SERIALIZER_SEATA,
                super::COMPRESSOR_NONE,
            ]
        || actual_id != id
    {
        return Err(super::invalid(format!(
            "unsupported or malformed V1 response: header={header:02x?}, expectedId={id}"
        )));
    }
    let mut body = vec![0; full - head];
    reader.read_exact(&mut body)?;
    let actual_kind = u16::from_be_bytes([body[0], body[1]]);
    if actual_kind != kind {
        return Err(super::invalid(format!(
            "response type {actual_kind}, expected {kind}"
        )));
    }
    Ok(body[2..].to_vec())
}

/// 从响应体读取公共结果前缀，更新偏移量，返回成功标志、错误码及可选错误消息。
/// 对应 Java: AbstractTransactionResponseCodec#decode。
pub(super) fn prefix(body: &[u8], at: &mut usize) -> io::Result<(bool, u8, Option<String>)> {
    let success = match body.first() {
        Some(0) => false,
        Some(1) => true,
        _ => return Err(super::invalid("invalid or missing result code")),
    };
    *at = 1;
    let message = if success {
        None
    } else {
        Some(super::read_string16(body, at)?)
    };
    let error_code = *body
        .get(*at)
        .ok_or_else(|| super::invalid("missing transaction error code"))?;
    *at += 1;
    Ok((success, error_code, message))
}

/// 校验 GlobalBegin 响应体，返回 TC 分配的非空 XID；失败或格式错误时返回错误。
/// 对应 Java: GlobalBeginResponseCodec#decode。
pub(super) fn parse_begin(body: &[u8]) -> io::Result<String> {
    let mut at = 0;
    let (success, code, message) = prefix(body, &mut at)?;
    let xid = super::read_string16(body, &mut at)?;
    let _extra_data = super::read_string16(body, &mut at)?;
    if at != body.len() {
        return Err(super::invalid("trailing bytes in GlobalBegin response"));
    }
    if !success {
        return Err(super::invalid(format!(
            "GlobalBegin rejected: code={code}, message={message:?}"
        )));
    }
    if xid.is_empty() {
        return Err(super::invalid(
            "GlobalBegin succeeded without a TC-issued XID",
        ));
    }
    Ok(xid)
}

fn parse_status(body: &[u8]) -> io::Result<TransactionResult> {
    let mut at = 0;
    let (success, error_code, message) = prefix(body, &mut at)?;
    let status = *body
        .get(at)
        .ok_or_else(|| super::invalid("missing GlobalStatus"))?;
    if status > 20 || at + 1 != body.len() {
        return Err(super::invalid("unknown GlobalStatus or trailing bytes"));
    }
    Ok(TransactionResult {
        success,
        error_code,
        message,
        status,
    })
}

/// 在给定通道发送指定类型和请求 ID 的消息，返回关联响应体；传输失败不会自动重发。
/// 对应 Java: AbstractNettyRemoting#sendSync；此处仅实现测试所需的严格子集。
pub(super) fn exchange<S: Read + Write>(
    stream: &mut S,
    id: u32,
    kind: u16,
    body: &[u8],
) -> io::Result<Vec<u8>> {
    let frame = super::build_v1_request(kind, id, body)?;
    // A failed write may have sent a prefix. Never replay a transaction request here.
    stream
        .write_all(&frame)
        .and_then(|_| stream.flush())
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("request {kind}/{id} write failed; outcome unknown; no retry: {e}"),
            )
        })?;
    read_payload(stream, id, kind + 1).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("request {kind}/{id} response unavailable; outcome unknown; no retry: {e}"),
        )
    })
}

/// 使用 XID 执行指定全局操作或状态查询，推进请求 ID 并返回成功响应中的 GlobalStatus。
/// 对应 Java: AbstractGlobalEndRequestCodec / GlobalStatusResponseCodec。
pub(super) fn global_end<S: Read + Write>(
    stream: &mut S,
    id: &mut u32,
    kind: u16,
    xid: &str,
) -> io::Result<u8> {
    global_end_observed(stream, id, kind, xid, false)
}

fn global_end_observed<S: Read + Write>(
    stream: &mut S,
    id: &mut u32,
    kind: u16,
    xid: &str,
    allow_retrying_rollback: bool,
) -> io::Result<u8> {
    let mut body = Vec::new();
    super::write_string16(xid, &mut body)?;
    super::write_string16("", &mut body)?;
    let response = parse_status(&exchange(stream, *id, kind, &body)?)?;
    *id += 1;
    if !response.success && !(allow_retrying_rollback && kind == 9 && response.status == 5) {
        return Err(super::invalid(format!(
            "TC rejected type {kind}, xid={xid}: {response:?}"
        )));
    }
    if !response.success {
        println!("OBSERVE initial GlobalRollback FAILED xid={xid} errorCode={} message={:?} status=RollbackRetrying(5); outcome pending, no replay",response.error_code,response.message);
    }
    Ok(response.status)
}

/// 在已注入回调应答丢失的场景发送一次回滚，返回初始状态；失败的重试态仍须后续查询收敛。
/// 对应 Java: AbstractTCInboundHandler#handle(GlobalRollbackRequest) 的失败状态回填。
pub(super) fn rollback_after_callback_loss<S: Read + Write>(
    stream: &mut S,
    id: &mut u32,
    xid: &str,
) -> io::Result<u8> {
    global_end_observed(stream, id, 9, xid, true)
}

pub(super) fn verify_empty_transactions<S: Read + Write>(stream: &mut S) -> io::Result<()> {
    let mut id = 1000;
    for (operation, expected, name) in [(7, 9, "commit"), (9, 11, "rollback")] {
        let mut body = 60_000u32.to_be_bytes().to_vec();
        super::write_string16(&format!("seata-rust-it-empty-{name}"), &mut body)?;
        let xid = parse_begin(&exchange(stream, id, 1, &body)?)?;
        id += 1;
        println!("GlobalBegin xid={xid} scenario=empty-{name}");
        let before = global_end(stream, &mut id, 15, &xid)?;
        if before != 1 {
            return Err(super::invalid(format!(
                "new XID {xid} status={before}, expected Begin=1"
            )));
        }
        let decision = global_end(stream, &mut id, operation, &xid)?;
        if decision != expected {
            return Err(super::invalid(format!(
                "{name} xid={xid} responseStatus={decision}, expected {expected}"
            )));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let observed = global_end(stream, &mut id, 15, &xid)?;
            // Finished is accepted ONLY after the matching successful end response.
            if observed == expected || observed == 15 {
                println!("PASS empty-transaction {name} xid={xid} responseStatus={decision} observedStatus={observed}");
                break;
            }
            let pending = if operation == 7 {
                matches!(observed, 2 | 3 | 8)
            } else {
                matches!(observed, 4 | 5)
            };
            if !pending || std::time::Instant::now() >= deadline {
                return Err(super::invalid(format!(
                    "{name} xid={xid} final status not verified: {observed}"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
        }
    }
    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn drives_two_separate_empty_transactions_without_replaying_requests() {
        let mut input = Vec::new();
        let mut id = 1000;
        for (ending_type, ending_status, xid) in [
            (8, 9, "127.0.0.1:18091:12345"),
            (10, 11, "127.0.0.1:18091:12346"),
        ] {
            let mut begin = vec![1, 0];
            crate::write_string16(xid, &mut begin).unwrap();
            crate::write_string16("", &mut begin).unwrap();
            for (kind, body) in [
                (2, begin),
                (16, vec![1, 0, 1]),
                (ending_type, vec![1, 0, ending_status]),
                (16, vec![1, 0, 15]),
            ] {
                let mut frame = crate::build_v1_request(kind, id, &body).unwrap();
                frame[9] = crate::RESPONSE;
                input.extend_from_slice(&frame);
                id += 1;
            }
        }
        let mut stream = Duplex {
            input: Cursor::new(input),
            output: Vec::new(),
        };
        verify_empty_transactions(&mut stream).unwrap();
        let mut at = 0;
        let mut types = Vec::new();
        while at < stream.output.len() {
            let len =
                u32::from_be_bytes(stream.output[at + 3..at + 7].try_into().unwrap()) as usize;
            types.push(u16::from_be_bytes(
                stream.output[at + 16..at + 18].try_into().unwrap(),
            ));
            at += len;
        }
        assert_eq!(types, vec![1, 15, 7, 15, 1, 15, 9, 15]);
    }

    #[test]
    fn lost_begin_response_is_unknown_and_is_not_retried() {
        let mut stream = Duplex {
            input: Cursor::new(Vec::new()),
            output: Vec::new(),
        };
        let err = verify_empty_transactions(&mut stream).unwrap_err();
        assert!(err.to_string().contains("outcome unknown"));
        assert_eq!(
            u32::from_be_bytes(stream.output[3..7].try_into().unwrap()) as usize,
            stream.output.len()
        );
    }

    fn response(kind: u16, body: &[u8]) -> Vec<u8> {
        let mut frame = crate::build_v1_request(kind, 42, body).unwrap();
        frame[9] = crate::RESPONSE;
        frame
    }

    #[test]
    fn injected_rollback_loss_retains_failed_retrying_outcome_for_later_query() {
        // Java 现场语义：Failed、错误消息 timed out、异常码 Unknown=0、RollbackRetrying=5。
        let body = [
            0, 0, 9, b't', b'i', b'm', b'e', b'd', b' ', b'o', b'u', b't', 0, 5,
        ];
        let mut stream = Duplex {
            input: Cursor::new(response(10, &body)),
            output: Vec::new(),
        };
        assert_eq!(
            super::rollback_after_callback_loss(&mut stream, &mut 42, "x").unwrap(),
            5
        );
        // 只发出一次 GlobalRollback；后续由 TC 重试分支，客户端不重发全局请求。
        assert_eq!(stream.output.len(), 23);
        assert_eq!(&stream.output[16..], &[0, 9, 0, 1, b'x', 0, 0]);
    }

    #[test]
    fn normal_path_and_unexpected_failed_states_remain_errors() {
        for status in [5, 7, 11, 15] {
            let body = [
                0, 0, 9, b't', b'i', b'm', b'e', b'd', b' ', b'o', b'u', b't', 0, status,
            ];
            let mut normal = Duplex {
                input: Cursor::new(response(10, &body)),
                output: Vec::new(),
            };
            assert!(super::global_end(&mut normal, &mut 42, 9, "x").is_err());
            if status != 5 {
                let mut fault = Duplex {
                    input: Cursor::new(response(10, &body)),
                    output: Vec::new(),
                };
                assert!(super::rollback_after_callback_loss(&mut fault, &mut 42, "x").is_err());
            }
        }
    }

    #[test]
    fn accepts_complete_response_and_removes_type_prefix() {
        assert_eq!(
            read_payload(&mut Cursor::new(response(16, &[1, 0, 1])), 42, 16).unwrap(),
            vec![1, 0, 1]
        );
    }

    #[test]
    fn rejects_truncated_frame() {
        let mut frame = response(16, &[1, 0, 1]);
        frame.pop();
        assert!(read_payload(&mut Cursor::new(frame), 42, 16).is_err());
    }

    #[test]
    fn rejects_wrong_id_type_and_unsupported_envelope() {
        for offset in [0, 2, 7, 9, 10, 11, 15, 17] {
            let mut frame = response(16, &[1, 0, 1]);
            frame[offset] ^= 0x7f;
            assert!(
                read_payload(&mut Cursor::new(frame), 42, 16).is_err(),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn begin_preserves_tc_issued_xid() {
        let mut body = vec![1, 0];
        crate::write_string16("127.0.0.1:18091:12345", &mut body).unwrap();
        crate::write_string16("", &mut body).unwrap();
        assert_eq!(parse_begin(&body).unwrap(), "127.0.0.1:18091:12345");
    }

    #[test]
    fn begin_rejects_empty_xid_even_on_success() {
        assert!(parse_begin(&[1, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn retains_failure_code_message_and_authoritative_status() {
        let mut body = vec![0];
        crate::write_string16("commit pending", &mut body).unwrap();
        body.extend_from_slice(&[3, 3]);
        assert_eq!(
            parse_status(&body).unwrap(),
            TransactionResult {
                success: false,
                error_code: 3,
                message: Some("commit pending".into()),
                status: 3,
            }
        );
    }

    #[test]
    fn distinguishes_finished_from_committed() {
        assert_eq!(parse_status(&[1, 0, 9]).unwrap().status, 9);
        assert_eq!(parse_status(&[1, 0, 15]).unwrap().status, 15);
    }

    #[test]
    fn rejects_malformed_result_status_and_trailing_bytes() {
        for bytes in [
            &[2, 0, 1][..],
            &[1, 0],
            &[1, 0, 255],
            &[1, 0, 1, 0],
            &[0, 0, 8, b'x'],
        ] {
            assert!(parse_status(bytes).is_err());
        }
    }
}
