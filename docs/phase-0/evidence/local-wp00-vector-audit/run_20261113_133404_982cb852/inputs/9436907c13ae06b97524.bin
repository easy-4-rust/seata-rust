use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;

fn initial<T: Read + Write, R: Read + Write>(
    tm: &mut T,
    rm: &mut R,
    xid: &str,
    resource: &str,
    status: u8,
    has_branch: bool,
) -> io::Result<u32> {
    if !matches!(status, 16 | 17) || xid.is_empty() || resource.is_empty() {
        return Err(crate::invalid("invalid backend retry identity/status"));
    }
    let mut id = 9000;
    let observed = crate::transaction::global_end(tm, &mut id, 15, xid)?;
    if observed != status {
        return Err(crate::invalid(
            "retry-timeout record not retained under its original XID",
        ));
    }
    check_lock(tm, rm, &mut id, xid, resource, !has_branch)?;
    println!(
        "BACKEND_INITIAL xid={xid} status={observed} branches={} targetMutationCount=0",
        u8::from(has_branch)
    );
    Ok(id)
}

fn check_lock<T: Read + Write, R: Read + Write>(
    tm: &mut T,
    rm: &mut R,
    id: &mut u32,
    xid: &str,
    resource: &str,
    expected: bool,
) -> io::Result<()> {
    let mut begin = 60_000u32.to_be_bytes().to_vec();
    crate::write_string16("backend-lock-contender", &mut begin)?;
    let contender =
        crate::transaction::parse_begin(&crate::transaction::exchange(tm, *id, 1, &begin)?)?;
    *id += 1;
    if contender == xid {
        return Err(crate::invalid("lock contender reused original XID"));
    }
    let mut body = Vec::new();
    crate::write_string16(&contender, &mut body)?;
    body.push(0);
    crate::write_string16(resource, &mut body)?;
    crate::write_string32("orders:1", &mut body)?;
    crate::write_string32("{}", &mut body)?;
    let actual =
        crate::restart_probe::parse_lockable(&crate::transaction::exchange(rm, *id, 21, &body)?)?;
    *id += 1;
    let cleanup = crate::transaction::global_end(tm, id, 9, &contender)?;
    println!("BACKEND_LOCK targetXid={xid} contenderXid={contender} lockable={actual} expected={expected}");
    if cleanup != 11 || actual != expected {
        return Err(crate::invalid(
            "backend lock state/contender cleanup differs",
        ));
    }
    Ok(())
}

/// 验证人工重试前后的原 XID、分支回调和锁；输入就绪文件只供外部运行器协调，返回验证结果。
/// 对应 Java: AbstractGlobalService#changeGlobalStatus、DefaultCore；仅模拟 RM 探针。
pub(super) fn run(
    tm: &mut TcpStream,
    rm: &mut TcpStream,
    xid: &str,
    resource: &str,
    status: u8,
    branch_id: i64,
    ready: &Path,
) -> io::Result<()> {
    if branch_id < 0 {
        return Err(crate::invalid("negative backend branch identity"));
    }
    let mut id = initial(tm, rm, xid, resource, status, branch_id > 0)?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(ready)?;
    writeln!(file, "{}", std::process::id())?;
    file.sync_all()?;
    let (observation, callback) = std::thread::scope(|scope| {
        let worker=scope.spawn(||->io::Result<u32>{
            if branch_id==0{return Ok(0);}
            let request=crate::branch_end_request::BranchEndRequest::read(rm)?;
            let mut state=crate::branch_callback_state::BranchCallbackState::new(if status==16{3}else{5},xid,branch_id,resource);
            let mut actions=0;
            let response=state.reply(&request,||{actions+=1;Ok(())})?;
            rm.write_all(&response)?;rm.flush()?;
            println!("BACKEND_CALLBACK xid={xid} branchId={branch_id} kind={} requestId={} actions={actions}",request.kind,request.request_id);
            Ok(actions)
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let observation = (|| -> io::Result<()> {
            loop {
                let observed = crate::transaction::global_end(tm, &mut id, 15, xid)?;
                if observed == 15 {
                    break;
                }
                if !matches!(observed, 2..=5 | 8 | 9 | 11 | 16 | 17)
                    || std::time::Instant::now() >= deadline
                {
                    return Err(crate::invalid("operator retry did not remove old session"));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(())
        })();
        let callback = worker
            .join()
            .unwrap_or_else(|_| Err(crate::invalid("backend callback worker panicked")));
        (observation, callback)
    });
    let actions = callback?;
    observation?;
    if actions != u32::from(branch_id > 0) {
        return Err(crate::invalid(
            "backend simulated callback action count differs",
        ));
    }
    check_lock(tm, rm, &mut id, xid, resource, true)?;
    println!("PASS BACKEND_RETRY xid={xid} branchId={branch_id} initialStatus={status} observedStatus=15 actions={actions} lockReleased=true targetMutationCount=0");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initial;
    use std::io::{self, Cursor, Read, Write};
    struct Channel {
        input: Cursor<Vec<u8>>,
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
    fn response(kind: u16, id: u32, body: &[u8]) -> Vec<u8> {
        let mut f = crate::build_v1_request(kind, id, body).unwrap();
        f[9] = 1;
        f
    }
    fn requests(channel: &Channel) -> Vec<(u16, String)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < channel.written.len() {
            let n =
                u32::from_be_bytes(channel.written[at + 3..at + 7].try_into().unwrap()) as usize;
            let kind = u16::from_be_bytes(channel.written[at + 16..at + 18].try_into().unwrap());
            let mut offset = at + 18;
            let xid = if kind == 1 {
                String::new()
            } else {
                crate::read_string16(&channel.written, &mut offset).unwrap()
            };
            out.push((kind, xid));
            at += n;
        }
        out
    }
    #[test]
    fn timeout_queries_preserve_status_and_never_mutate_the_original_xid() {
        for status in [16, 17] {
            for has_branch in [false, true] {
                let mut begin = vec![1, 0];
                crate::write_string16("contender", &mut begin).unwrap();
                crate::write_string16("", &mut begin).unwrap();
                let input = [
                    response(16, 9000, &[1, 0, status]),
                    response(2, 9001, &begin),
                    response(10, 9003, &[1, 0, 11]),
                ]
                .concat();
                let mut tm = Channel {
                    input: Cursor::new(input),
                    written: vec![],
                };
                let mut rm = Channel {
                    input: Cursor::new(response(22, 9002, &[1, 0, 0, u8::from(!has_branch)])),
                    written: vec![],
                };
                assert_eq!(
                    initial(&mut tm, &mut rm, "original", "resource", status, has_branch).unwrap(),
                    9004
                );
                assert_eq!(
                    requests(&tm),
                    [
                        (15, "original".into()),
                        (1, "".into()),
                        (9, "contender".into())
                    ]
                );
                assert_eq!(requests(&rm), [(21, "contender".into())]);
            }
        }
    }
    #[test]
    fn finished_cannot_masquerade_as_a_retained_retry_timeout() {
        let mut tm = Channel {
            input: Cursor::new(response(16, 9000, &[1, 0, 15])),
            written: vec![],
        };
        let mut rm = Channel {
            input: Cursor::new(vec![]),
            written: vec![],
        };
        assert!(initial(&mut tm, &mut rm, "original", "resource", 16, true).is_err());
        assert_eq!(requests(&tm), [(15, "original".into())]);
        assert!(rm.written.is_empty());
    }
}
