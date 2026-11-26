use std::io::{self, Read, Write};

/// 查询独立 FILE 夹具的原 XID 及锁；仅测试工具，不实现 Java 恢复或操作员重试。
/// 对应 Java: DefaultCore#getStatus、AbstractCore#lockQuery。
pub(super) fn verify<T: Read + Write, R: Read + Write>(
    tm: &mut T,
    rm: &mut R,
    xid: &str,
    resource: &str,
    expected_status: u8,
) -> io::Result<()> {
    if !matches!(expected_status, 1 | 15) || xid.is_empty() || resource.is_empty() {
        return Err(crate::invalid("invalid FILE query expectation/identity"));
    }
    let mut id = 7000;
    let status = crate::transaction::global_end(tm, &mut id, 15, xid)?;
    println!("FILE_QUERY xid={xid} status={status} expected={expected_status}");
    if status != expected_status {
        return Err(crate::invalid(
            "actual FILE status differs from expectation",
        ));
    }
    let mut begin = 60_000u32.to_be_bytes().to_vec();
    crate::write_string16("timeout-file-lock-contender", &mut begin)?;
    let contender =
        crate::transaction::parse_begin(&crate::transaction::exchange(tm, id, 1, &begin)?)?;
    id += 1;
    if contender == xid {
        return Err(crate::invalid("lock contender reused target XID"));
    }
    let mut lock = Vec::new();
    crate::write_string16(&contender, &mut lock)?;
    lock.push(0);
    crate::write_string16(resource, &mut lock)?;
    crate::write_string32("orders:1", &mut lock)?;
    crate::write_string32("{}", &mut lock)?;
    let actual =
        crate::restart_probe::parse_lockable(&crate::transaction::exchange(rm, id, 21, &lock)?)?;
    id += 1;
    let cleanup = crate::transaction::global_end(tm, &mut id, 9, &contender)?;
    if cleanup != 11 || actual != (expected_status == 15) {
        return Err(crate::invalid(
            "FILE restored lock/contender cleanup mismatch",
        ));
    }
    println!("PASS FILE_QUERY xid={xid} status={status} contenderXid={contender} lockable={actual} targetMutationCount=0");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::verify;
    use std::io::{self, Cursor, Read, Write};

    struct Channel {
        input: Cursor<Vec<u8>>,
        written: Vec<u8>,
    }
    impl Channel {
        fn new(input: Vec<u8>) -> Self {
            Self {
                input: Cursor::new(input),
                written: Vec::new(),
            }
        }
        fn requests(&self) -> Vec<(u16, Vec<u8>)> {
            let mut at = 0;
            let mut requests = Vec::new();
            while at < self.written.len() {
                let length =
                    u32::from_be_bytes(self.written[at + 3..at + 7].try_into().unwrap()) as usize;
                requests.push((
                    u16::from_be_bytes(self.written[at + 16..at + 18].try_into().unwrap()),
                    self.written[at + 18..at + length].to_vec(),
                ));
                at += length;
            }
            requests
        }
    }
    impl Read for Channel {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.input.read(buffer)
        }
    }
    impl Write for Channel {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn reply(kind: u16, id: u32, body: &[u8]) -> Vec<u8> {
        let mut frame = crate::build_v1_request(kind, id, body).unwrap();
        frame[9] = 1;
        frame
    }
    fn success(status: u8) -> Vec<u8> {
        vec![1, 0, status]
    }
    fn begin(xid: &str) -> Vec<u8> {
        let mut body = vec![1, 0];
        crate::write_string16(xid, &mut body).unwrap();
        crate::write_string16("", &mut body).unwrap();
        body
    }
    #[test]
    fn observes_original_status_and_lock_using_a_separate_cleaned_up_contender() {
        for status in [1, 15] {
            let responses = [
                reply(16, 7000, &success(status)),
                reply(2, 7001, &begin("contender")),
                reply(10, 7003, &success(11)),
            ]
            .concat();
            let mut tm = Channel::new(responses);
            let mut rm = Channel::new(reply(22, 7002, &[1, 0, 0, u8::from(status == 15)]));
            verify(&mut tm, &mut rm, "old-xid", "mysql://timeout", status).unwrap();
            let requests = tm.requests();
            assert_eq!(requests.iter().map(|r| r.0).collect::<Vec<_>>(), [15, 1, 9]);
            let mut at = 0;
            assert_eq!(
                crate::read_string16(&requests[0].1, &mut at).unwrap(),
                "old-xid"
            );
            let mut at = 0;
            assert_eq!(
                crate::read_string16(&requests[2].1, &mut at).unwrap(),
                "contender"
            );
            assert_eq!(rm.requests().iter().map(|r| r.0).collect::<Vec<_>>(), [21]);
            let mut at = 0;
            assert_eq!(
                crate::read_string16(&rm.requests()[0].1, &mut at).unwrap(),
                "contender"
            );
        }
    }
    #[test]
    fn a_retained_timeout_is_not_misreported_as_finished_or_retried() {
        let mut tm = Channel::new(reply(16, 7000, &success(16)));
        let mut rm = Channel::new(Vec::new());
        assert!(verify(&mut tm, &mut rm, "old-xid", "resource", 15).is_err());
        assert_eq!(tm.requests().iter().map(|r| r.0).collect::<Vec<_>>(), [15]);
        assert!(rm.written.is_empty());
    }
    #[test]
    fn invalid_expected_status_does_not_send_anything() {
        for status in [0, 9, 11, 16, 17, 21] {
            let mut tm = Channel::new(Vec::new());
            let mut rm = Channel::new(Vec::new());
            assert!(verify(&mut tm, &mut rm, "old-xid", "resource", status).is_err());
            assert!(tm.written.is_empty() && rm.written.is_empty());
        }
    }
}
