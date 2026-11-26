use std::io::{self, Read, Write};

/// 独立探针在请求可能生效后失去应答的记录；不是生产 TM 错误 API。
/// 对应 Java: AbstractTransactionRequestToTC 的调用结果处理职责；无生产对象移植。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnknownOutcome {
    pub(super) operation: u16,
    pub(super) request_id: u32,
    pub(super) xid: String,
    pub(super) last_observed_status: Option<u8>,
    confirmed_terminal_status: Option<u8>,
}

impl UnknownOutcome {
    /// 输入已编码请求及已知 XID，发送恰好一次；仅确认应答丢失时返回未知记录，否则返回探针错误。
    pub(super) fn send_once<S: Read + Write>(
        stream: &mut S,
        operation: u16,
        request_id: u32,
        xid: &str,
        body: &[u8],
    ) -> io::Result<Self> {
        if !matches!(operation, 7 | 9 | 11) || xid.is_empty() {
            return Err(crate::invalid("unsupported uncertain request identity"));
        }
        let frame = crate::build_v1_request(operation, request_id, body)?;
        stream
            .write_all(&frame)
            .and_then(|_| stream.flush())
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("write may have been partial; outcome unknown, no replay: {error}"),
                )
            })?;
        match crate::transaction::read_payload(stream, request_id, operation + 1) {
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::NotConnected
                ) =>
            {
                println!("UNKNOWN operation={operation} requestId={request_id} xid={xid} cause={:?} replayed=false",error.kind());
                Ok(Self {
                    operation,
                    request_id,
                    xid: xid.into(),
                    last_observed_status: None,
                    confirmed_terminal_status: None,
                })
            }
            Err(error) => Err(error),
            Ok(_) => Err(crate::invalid(
                "fault injection missing: mutating response was delivered",
            )),
        }
    }

    /// 输入 TC 查询状态并保留观测，返回是否由匹配终态确认结束；Finished15 绝不单独证明成功。
    pub(super) fn observe(&mut self, status: u8) -> io::Result<bool> {
        if status > 20 {
            return Err(crate::invalid("unknown authoritative global status"));
        }
        self.last_observed_status = Some(status);
        if matches!((self.operation, status), (7, 9) | (9, 11)) {
            self.confirmed_terminal_status = Some(status);
        }
        Ok(self.confirmed_terminal_status.is_some())
    }

    /// 无参数，返回曾经由匹配终态查询确认的状态；单独 Finished 时仍为 None。
    pub(super) fn confirmed_status(&self) -> Option<u8> {
        self.confirmed_terminal_status
    }
}

#[cfg(test)]
mod tests {
    use super::UnknownOutcome;
    use std::io::{self, Read, Write};
    struct Lost {
        written: Vec<u8>,
        kind: io::ErrorKind,
    }
    impl Read for Lost {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(self.kind, "response lost"))
        }
    }
    impl Write for Lost {
        fn write(&mut self, value: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(value);
            Ok(value.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn lost_mutating_response_retains_identity_and_never_replays() {
        for operation in [7, 9, 11] {
            let mut stream = Lost {
                written: vec![],
                kind: io::ErrorKind::TimedOut,
            };
            let mut unknown =
                UnknownOutcome::send_once(&mut stream, operation, 77, "x", &[]).unwrap();
            assert_eq!(
                stream.written,
                crate::build_v1_request(operation, 77, &[]).unwrap()
            );
            assert_eq!(
                (unknown.operation, unknown.request_id, unknown.xid.as_str()),
                (operation, 77, "x")
            );
            assert!(!unknown.observe(15).unwrap());
            assert_eq!(unknown.last_observed_status, Some(15));
            assert_eq!(stream.written.len(), 18);
        }
    }
    #[test]
    fn only_matching_terminal_queries_confirm_end_not_pending_or_finished() {
        for operation in [7, 9, 11] {
            for status in 0..=20 {
                let mut value = UnknownOutcome {
                    operation,
                    request_id: 1,
                    xid: "x".into(),
                    last_observed_status: None,
                    confirmed_terminal_status: None,
                };
                assert_eq!(
                    value.observe(status).unwrap(),
                    matches!((operation, status), (7, 9) | (9, 11))
                );
                assert!(value.observe(21).is_err());
            }
        }
    }
    #[test]
    fn a_later_finished_query_does_not_erase_already_observed_terminal_proof() {
        let mut value = UnknownOutcome {
            operation: 7,
            request_id: 1,
            xid: "x".into(),
            last_observed_status: None,
            confirmed_terminal_status: None,
        };
        assert!(value.observe(9).unwrap());
        assert!(value.observe(15).unwrap());
        assert_eq!(value.confirmed_status(), Some(9));
    }
    #[test]
    fn unsupported_request_or_protocol_error_is_not_accepted_as_injected_loss() {
        for operation in [1, 15, 8] {
            let mut stream = Lost {
                written: vec![],
                kind: io::ErrorKind::TimedOut,
            };
            assert!(UnknownOutcome::send_once(&mut stream, operation, 1, "x", &[]).is_err());
            assert!(stream.written.is_empty());
        }
        let mut stream = Lost {
            written: vec![],
            kind: io::ErrorKind::InvalidData,
        };
        assert!(UnknownOutcome::send_once(&mut stream, 7, 1, "x", &[]).is_err());
    }

    #[test]
    fn a_delivered_response_cannot_pass_a_response_loss_test() {
        struct Delivered {
            input: io::Cursor<Vec<u8>>,
            written: Vec<u8>,
        }
        impl Read for Delivered {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.input.read(out)
            }
        }
        impl Write for Delivered {
            fn write(&mut self, out: &[u8]) -> io::Result<usize> {
                self.written.extend_from_slice(out);
                Ok(out.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut frame = crate::build_v1_request(8, 77, &[1, 0, 9]).unwrap();
        frame[9] = crate::RESPONSE;
        let mut stream = Delivered {
            input: io::Cursor::new(frame),
            written: vec![],
        };
        let error = UnknownOutcome::send_once(&mut stream, 7, 77, "x", &[]).unwrap_err();
        assert!(error.to_string().contains("fault injection missing"));
        assert_eq!(stream.written.len(), 18);
    }

    #[test]
    fn partial_write_failure_is_not_replayed_or_mislabeled_as_a_read_timeout() {
        struct WriteFailure {
            calls: usize,
        }
        impl Read for WriteFailure {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("must not read after failed write")
            }
        }
        impl Write for WriteFailure {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                self.calls += 1;
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "partial write possible",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut stream = WriteFailure { calls: 0 };
        let error = UnknownOutcome::send_once(&mut stream, 7, 77, "x", &[]).unwrap_err();
        assert!(error.to_string().contains("outcome unknown"));
        assert_eq!(stream.calls, 1);
    }
}
