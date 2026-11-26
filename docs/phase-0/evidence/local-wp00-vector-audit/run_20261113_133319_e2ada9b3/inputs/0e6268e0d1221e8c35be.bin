//! 独立测试探针的分支协议验证，不执行数据库或 Undo Log 操作。
use std::io::{self, Read, Write};

/// 在已注册的 TM/RM 通道上验证两笔带模拟 AT 分支的事务，返回验证结果。
/// 对应 Java: DefaultCore#commit/#rollback、AbstractCore#branchRegister；仅测试工具。
pub(super) fn verify_branch_transactions(
    tm: &mut std::net::TcpStream,
    rm: &mut std::net::TcpStream,
    resource: &str,
) -> io::Result<()> {
    verify_transactions(tm, rm, resource, false)
}

/// 在给定已注册通道上丢弃每笔分支的首次回调响应，要求 TC 重投并验证模拟动作仅应用一次。
/// 返回两笔事务的验证结果；对应 Java: AbstractCore#branchCommitSend / DefaultCoordinator 重试链。
pub(super) fn verify_callback_loss(
    tm: &mut std::net::TcpStream,
    rm: &mut std::net::TcpStream,
    resource: &str,
) -> io::Result<()> {
    verify_transactions(tm, rm, resource, true)
}

fn verify_transactions(
    tm: &mut std::net::TcpStream,
    rm: &mut std::net::TcpStream,
    resource: &str,
    lose_first: bool,
) -> io::Result<()> {
    let mut id = 2000;
    let mut previous_xid = None;
    for (operation, callback, expected, name) in [(7, 3, 9, "commit"), (9, 5, 11, "rollback")] {
        let mut body = 60_000u32.to_be_bytes().to_vec();
        crate::write_string16(&format!("seata-rust-it-branch-{name}"), &mut body)?;
        let xid =
            crate::transaction::parse_begin(&crate::transaction::exchange(tm, id, 1, &body)?)?;
        id += 1;
        println!("GlobalBegin xid={xid} scenario=branch-{name}");
        if previous_xid.as_ref() == Some(&xid) {
            return Err(crate::invalid(format!(
                "TC reused XID {xid}; refusing a second branch scenario"
            )));
        }
        previous_xid = Some(xid.clone());
        let mut body = Vec::new();
        crate::write_string16(&xid, &mut body)?;
        body.push(0); // BranchType.AT；仅模拟资源协议，不实现 AT 数据补偿。
        crate::write_string16(resource, &mut body)?;
        crate::write_string32("orders:1", &mut body)?;
        crate::write_string32("{}", &mut body)?;
        let branch_id = parse_branch_id(&crate::transaction::exchange(rm, id, 11, &body)?)?;
        id += 1;
        println!("BranchRegister xid={xid} branchId={branch_id} resource={resource}");
        if crate::transaction::global_end(tm, &mut id, 15, &xid)? != 1 {
            return Err(crate::invalid(format!(
                "new branch transaction {xid} is not Begin"
            )));
        }
        // RM 与 TM 各有一个读取者。必须先启动 RM 接收，再等待同步回滚或异步提交结果。
        let (decision, callback_result) = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                if lose_first {
                    receive_redelivery(rm, callback, &xid, branch_id, resource)
                } else {
                    acknowledge_callback(rm, callback, &xid, branch_id, resource)
                }
            });
            let decision = if lose_first && operation == 9 {
                crate::transaction::rollback_after_callback_loss(tm, &mut id, &xid)
            } else {
                crate::transaction::global_end(tm, &mut id, operation, &xid)
            };
            let callback_result = worker
                .join()
                .unwrap_or_else(|_| Err(crate::invalid("RM callback worker panicked")));
            (decision, callback_result)
        });
        callback_result?;
        let decision = decision?;
        if decision != expected && !(lose_first && operation == 9 && decision == 5) {
            return Err(crate::invalid(format!(
                "{name} xid={xid} responseStatus={decision}, expected={expected}"
            )));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let observed = crate::transaction::global_end(tm, &mut id, 15, &xid)?;
            if observed == 15 {
                println!("PASS branch-{name} xid={xid} branchId={branch_id} responseStatus={decision} observedStatus={observed} callbackLoss={lose_first}; simulated RM only");
                break;
            }
            let pending = if operation == 7 {
                matches!(observed, 2 | 3 | 8 | 9)
            } else {
                matches!(observed, 4 | 5 | 11)
            };
            if !pending || std::time::Instant::now() >= deadline {
                return Err(crate::invalid(format!(
                    "{name} xid={xid} did not finish after callback: {observed}"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    Ok(())
}

/// 从指定 RM 通道校验并应答一个预期分支回调，参数为操作及分支身份，返回处理结果。
/// 对应 Java: RmBranchCommitProcessor / RmBranchRollbackProcessor。
pub(super) fn acknowledge_callback<S: Read + Write>(
    stream: &mut S,
    kind: u16,
    xid: &str,
    branch_id: i64,
    resource: &str,
) -> io::Result<()> {
    let request = crate::branch_end_request::BranchEndRequest::read(stream)?;
    let mut state =
        crate::branch_callback_state::BranchCallbackState::new(kind, xid, branch_id, resource);
    let frame = state.reply(&request, || Ok(()))?;
    let status = if kind == 3 { 5 } else { 8 };
    stream.write_all(&frame)?;
    stream.flush()?;
    println!("RM callback acknowledged requestId={} type={kind} xid={xid} branchId={branch_id} branchStatus={status}",request.request_id);
    Ok(())
}

fn receive_redelivery<S: Read + Write>(
    stream: &mut S,
    kind: u16,
    xid: &str,
    branch_id: i64,
    resource: &str,
) -> io::Result<()> {
    let mut state =
        crate::branch_callback_state::BranchCallbackState::new(kind, xid, branch_id, resource);
    let mut actions = 0;
    let first = crate::branch_end_request::BranchEndRequest::read(stream)?;
    let dropped = state.reply(&first, || {
        actions += 1;
        Ok(())
    })?;
    // 故障注入：模拟动作已成功，但首次响应没有交给 TCP；只能等待 TC 主动重投。
    drop(dropped);
    println!("INJECT drop-first-callback-response xid={xid} branchId={branch_id} type={kind} requestId={} simulatedActions={actions}",first.request_id);
    let retry = crate::branch_end_request::BranchEndRequest::read(stream)?;
    if retry.request_id == first.request_id {
        return Err(crate::invalid(
            "expected fresh TC retry RPC ID after withheld response",
        ));
    }
    let frame = state.reply(&retry, || {
        actions += 1;
        Ok(())
    })?;
    if actions != 1 {
        return Err(crate::invalid(
            "simulated action was reapplied on redelivery",
        ));
    }
    stream.write_all(&frame)?;
    stream.flush()?;
    println!("PASS TC redelivery xid={xid} branchId={branch_id} type={kind} firstRequestId={} retryRequestId={} deliveries=2 simulatedActions={actions}",first.request_id,retry.request_id);
    Ok(())
}

/// 从 BranchRegister 响应体返回 TC 分配的正数分支 ID，失败或格式错误时拒绝。
/// 对应 Java: BranchRegisterResponseCodec#decode。
pub(super) fn parse_branch_id(body: &[u8]) -> io::Result<i64> {
    let mut at = 0;
    let (success, code, message) = crate::transaction::prefix(body, &mut at)?;
    if body.len().saturating_sub(at) != 8 {
        return Err(crate::invalid("invalid BranchRegister response length"));
    }
    let id = i64::from_be_bytes(body[at..].try_into().unwrap());
    if !success || id <= 0 {
        return Err(crate::invalid(format!(
            "BranchRegister rejected: id={id} code={code} message={message:?}"
        )));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::{acknowledge_callback, parse_branch_id};
    use std::io::{self, Cursor, Read, Write};

    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for Duplex {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.input.read(bytes)
        }
    }
    impl Write for Duplex {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn hex(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&value[at..at + 2], 16).unwrap())
            .collect()
    }
    fn stream(kind: u8) -> Duplex {
        // 手工核对 Java AbstractBranchEndRequestCodec：xid=x、branchId=42、AT、resource=r、data={}。
        let mut bytes =
            hex("dada010000002700100001000000004d0003000178000000000000002a00000172000000027b7d");
        bytes[17] = kind;
        Duplex {
            input: Cursor::new(bytes),
            output: Vec::new(),
        }
    }
    #[test]
    fn commit_callback_echoes_request_id_and_returns_committed_branch_status() {
        let mut channel = stream(3);
        acknowledge_callback(&mut channel, 3, "x", 42, "r").unwrap();
        assert_eq!(
            channel.output,
            hex("dada010000002000100101000000004d00040100000178000000000000002a05")
        );
    }

    #[test]
    fn first_response_is_withheld_and_only_redelivery_is_acknowledged() {
        let mut channel = stream(3);
        let mut retry = stream(3).input.into_inner();
        retry[15] = 78;
        channel.input.get_mut().extend_from_slice(&retry);
        super::receive_redelivery(&mut channel, 3, "x", 42, "r").unwrap();
        assert_eq!(
            channel.output,
            hex("dada010000002000100101000000004e00040100000178000000000000002a05")
        );
    }

    #[test]
    fn missing_or_same_rpc_redelivery_cannot_pass_loss_probe() {
        for repeat_same_id in [false, true] {
            let mut channel = stream(3);
            if repeat_same_id {
                channel
                    .input
                    .get_mut()
                    .extend_from_slice(&stream(3).input.into_inner());
            }
            assert!(super::receive_redelivery(&mut channel, 3, "x", 42, "r").is_err());
            assert!(channel.output.is_empty());
        }
    }
    #[test]
    fn rollback_callback_echoes_request_id_and_returns_rollbacked_branch_status() {
        let mut channel = stream(5);
        acknowledge_callback(&mut channel, 5, "x", 42, "r").unwrap();
        assert_eq!(
            channel.output,
            hex("dada010000002000100101000000004d00060100000178000000000000002a08")
        );
    }
    #[test]
    fn wrong_identity_or_callback_operation_never_gets_success_ack() {
        for (kind, xid, id, resource) in [
            (5, "x", 42, "r"),
            (3, "other", 42, "r"),
            (3, "x", 43, "r"),
            (3, "x", 42, "other"),
        ] {
            let mut channel = stream(3);
            assert!(acknowledge_callback(&mut channel, kind, xid, id, resource).is_err());
            assert!(channel.output.is_empty());
        }
    }
    #[test]
    fn malformed_callback_never_gets_success_ack() {
        for offset in [0, 2, 6, 8, 9, 10, 11, 29, 34, 38] {
            let mut channel = stream(3);
            channel.input.get_mut()[offset] ^= 0x7f;
            assert!(
                acknowledge_callback(&mut channel, 3, "x", 42, "r").is_err(),
                "offset={offset}"
            );
            assert!(channel.output.is_empty());
        }
        let mut channel = stream(3);
        channel.input.get_mut().pop();
        assert!(acknowledge_callback(&mut channel, 3, "x", 42, "r").is_err());
        assert!(channel.output.is_empty());
    }
    #[test]
    fn accepts_positive_tc_branch_id() {
        assert_eq!(parse_branch_id(&hex("0100000000000000002a")).unwrap(), 42);
    }
    #[test]
    fn rejects_failure_invalid_id_truncation_and_trailing_data() {
        for bytes in [
            "0000017803000000000000002a",
            "01000000000000000000",
            "0100ffffffffffffffff",
            "01000000",
            "0100000000000000002a00",
        ] {
            assert!(parse_branch_id(&hex(bytes)).is_err());
        }
    }

    #[test]
    fn branch_lifecycle_services_rm_before_tm_end_can_finish() {
        exercise_lifecycle(false).unwrap();
    }

    #[test]
    fn rejects_reused_xid_before_registering_the_second_branch() {
        let error = exercise_lifecycle(true).unwrap_err();
        assert!(error.to_string().contains("reused XID"), "{error}");
    }

    fn exercise_lifecycle(repeat_xid: bool) -> io::Result<()> {
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;
        fn request(stream: &mut TcpStream, kind: u16) -> (u32, Vec<u8>) {
            let mut header = [0; 16];
            stream.read_exact(&mut header).unwrap();
            assert_eq!(header[9], 0);
            let len = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
            let mut body = vec![0; len - 16];
            stream.read_exact(&mut body).unwrap();
            assert_eq!(u16::from_be_bytes([body[0], body[1]]), kind);
            (
                u32::from_be_bytes(header[12..16].try_into().unwrap()),
                body[2..].to_vec(),
            )
        }
        fn respond(stream: &mut TcpStream, id: u32, kind: u16, body: &[u8]) {
            let mut bytes = vec![0xda, 0xda, 1];
            bytes.extend_from_slice(&((18 + body.len()) as u32).to_be_bytes());
            bytes.extend_from_slice(&[0, 16, 1, 1, 0]);
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.extend_from_slice(&kind.to_be_bytes());
            bytes.extend_from_slice(body);
            stream.write_all(&bytes).unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut tm = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server_tm, _) = listener.accept().unwrap();
        let mut rm = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server_rm, _) = listener.accept().unwrap();
        for socket in [&tm, &rm, &server_tm, &server_rm] {
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
        }
        let server = std::thread::spawn(move || {
            for (round, (operation, callback, terminal, branch_status)) in
                [(7, 3, 9, 5), (9, 5, 11, 8)].into_iter().enumerate()
            {
                let (id, _) = request(&mut server_tm, 1);
                let xid_char = if round == 1 && !repeat_xid {
                    b'y'
                } else {
                    b'x'
                };
                let mut begin = hex("01000001780000");
                begin[4] = xid_char;
                respond(&mut server_tm, id, 2, &begin);
                if round == 1 && repeat_xid {
                    // 重复 XID 必须在第二次分支注册前拒绝，不能继续向 RM 通道发送请求。
                    let mut byte = [0];
                    assert_eq!(server_rm.read(&mut byte).unwrap(), 0);
                    return;
                }
                let (id, body) = request(&mut server_rm, 11);
                // 独立期望值：xid=x/y、AT、资源=r、锁 orders:1、applicationData={}。
                let mut register = hex("00017800000172000000086f72646572733a31000000027b7d");
                register[2] = xid_char;
                assert_eq!(body, register);
                respond(&mut server_rm, id, 12, &hex("0100000000000000002a"));
                let (id, _) = request(&mut server_tm, 15);
                respond(&mut server_tm, id, 16, &[1, 0, 1]);
                let (id, body) = request(&mut server_tm, operation);
                let mut end = hex("0001780000");
                end[2] = xid_char;
                assert_eq!(body, end);
                let mut callback = stream(callback as u8).input.into_inner();
                callback[20] = xid_char;
                server_rm.write_all(&callback).unwrap();
                // TM 响应故意等待 RM 应答；串行实现会触发超时。
                let mut ack = vec![0; 32];
                server_rm.read_exact(&mut ack).unwrap();
                let mut expected =
                    hex("dada010000002000100101000000004d00040100000178000000000000002a05");
                expected[17] = (u16::from_be_bytes([callback[16], callback[17]]) + 1) as u8;
                expected[31] = branch_status;
                expected[22] = xid_char;
                assert_eq!(ack, expected);
                respond(&mut server_tm, id, operation + 1, &[1, 0, terminal]);
                let (id, _) = request(&mut server_tm, 15);
                respond(&mut server_tm, id, 16, &[1, 0, 15]);
            }
        });
        let result = super::verify_branch_transactions(&mut tm, &mut rm, "r");
        drop(tm);
        drop(rm);
        let server_result = server.join();
        server_result.unwrap();
        result
    }
}
