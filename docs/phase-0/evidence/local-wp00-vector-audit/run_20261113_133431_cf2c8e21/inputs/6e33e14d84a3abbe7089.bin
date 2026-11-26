//! 现场 RAFT 原事务只读状态查询；不是 Rust TC 或生产恢复实现。
use std::io::{self, Read, Write};

/// 查询两个原 XID 的明确状态，参数为已注册通道、身份与预期状态；只发送 GlobalStatus，不重放事务。
/// 对应 Java: DefaultCoordinator#doGlobalStatus、SessionHolder 的 RAFT 恢复语义。
pub(super) fn verify<S: Read + Write>(
    stream: &mut S,
    xids: &[&str],
    expected: &[u8],
) -> io::Result<()> {
    if xids.len() != 2
        || xids[0] == xids[1]
        || xids.iter().any(|x| x.is_empty())
        || !matches!(expected, [16, 16] | [17, 17] | [5, 15])
    {
        return Err(crate::invalid(
            "invalid original RAFT diagnostic identities/status pair",
        ));
    }
    let mut id = 20100;
    for (xid, status) in xids.iter().zip(expected) {
        let actual = crate::transaction::global_end(stream, &mut id, 15, xid)?;
        if actual != *status {
            return Err(crate::invalid("original RAFT status differs"));
        }
        println!("RAFT_STATUS xid={xid} status={actual} originalMutations=0");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::verify;
    use std::io::{self, Read, Write};
    struct Channel {
        input: io::Cursor<Vec<u8>>,
        written: Vec<u8>,
    }
    impl Read for Channel {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.input.read(b)
        }
    }
    impl Write for Channel {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn channel(statuses: &[u8]) -> Channel {
        let input = statuses
            .iter()
            .enumerate()
            .flat_map(|(index, status)| {
                let mut frame =
                    crate::build_v1_request(16, 20100 + index as u32, &[1, 0, *status]).unwrap();
                frame[9] = 1;
                frame
            })
            .collect();
        Channel {
            input: io::Cursor::new(input),
            written: vec![],
        }
    }
    #[test]
    fn both_original_xids_are_queried_once_and_no_mutation_is_sent() {
        for statuses in [[16, 16], [17, 17], [5, 15]] {
            let mut stream = channel(&statuses);
            verify(&mut stream, &["old-a", "old-b"], &statuses).unwrap();
            assert_eq!(
                stream.input.position() as usize,
                stream.input.get_ref().len()
            );
            let mut at = 0;
            let mut count = 0;
            while at < stream.written.len() {
                let length =
                    u32::from_be_bytes(stream.written[at + 3..at + 7].try_into().unwrap()) as usize;
                assert_eq!(&stream.written[at + 16..at + 18], &[0, 15]);
                let mut offset = at + 18;
                assert_eq!(
                    crate::read_string16(&stream.written, &mut offset).unwrap(),
                    if count == 0 { "old-a" } else { "old-b" }
                );
                at += length;
                count += 1;
            }
            assert_eq!(count, 2);
        }
    }
    #[test]
    fn finished_does_not_count_as_retained_retry_timeout() {
        let mut stream = channel(&[15, 16]);
        assert!(verify(&mut stream, &["old-a", "old-b"], &[16, 16]).is_err());
    }
    #[test]
    fn invalid_or_reused_original_identities_fail_before_network_io() {
        for (xids, statuses) in [(["same", "same"], [16, 16]), (["a", "b"], [15, 15])] {
            let mut stream = channel(&[]);
            assert!(verify(&mut stream, &xids, &statuses).is_err());
            assert!(stream.written.is_empty());
        }
    }
}
