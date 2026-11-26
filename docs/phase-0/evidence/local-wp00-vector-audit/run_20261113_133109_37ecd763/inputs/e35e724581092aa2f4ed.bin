use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};

const APP: &str = "phase0-auth";
const GROUP: &str = "default_tx_group";
const TOKEN: &str = "phase0-local-only";
const RESOURCES: &str = "mysql://auth-a,mysql://auth-b";

fn body(
    tm: bool,
    app: &str,
    group: &str,
    credential: &str,
    resources: &str,
) -> io::Result<Vec<u8>> {
    let mut value = Vec::new();
    for item in [crate::CLIENT_VERSION, app, group, credential] {
        crate::write_string16(item, &mut value)?;
    }
    if !tm {
        crate::write_string32(resources, &mut value)?;
    }
    Ok(value)
}

fn verify_unbound<S: Read + Write>(stream: &mut S) -> io::Result<()> {
    let mut request = 60_000u32.to_be_bytes().to_vec();
    crate::write_string16("denied-must-not-begin", &mut request)?;
    stream.write_all(&crate::build_v1_request(1, 80, &request)?)?;
    stream.flush()?;
    let mut first = [0];
    match stream.read(&mut first) {
        Ok(0) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::NotConnected
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
        Ok(_) => Err(crate::invalid(
            "denied channel returned transaction data instead of closing",
        )),
    }
}

fn register(
    endpoint: (&str, u16),
    tm: bool,
    app: &str,
    group: &str,
    credential: &str,
    resources: &str,
    expected: bool,
) -> io::Result<TcpStream> {
    let kind = if tm { 101 } else { 103 };
    let id = if tm { 1 } else { 2 };
    let frame = crate::build_v1_request(kind, id, &body(tm, app, group, credential, resources)?)?;
    let mut stream = crate::connect(endpoint.0, endpoint.1)?;
    stream.write_all(&frame)?;
    stream.flush()?;
    let response = crate::read_identify_response(&mut stream, id, kind + 1)?;
    println!(
        "AUTH_RESPONSE role={} identified={} serverVersion={}",
        if tm { "TM" } else { "RM" },
        response.identified,
        response.version
    );
    if response.identified != expected {
        return Err(crate::invalid(
            "auth identification flag differs from expected policy",
        ));
    }
    Ok(stream)
}

fn denied(host: &str, port: u16) -> io::Result<()> {
    let mut count = 0;
    for tm in [true, false] {
        let mut cases = vec![
            ("missing", APP, GROUP, "", RESOURCES),
            ("wrong", APP, GROUP, "wrong", RESOURCES),
            ("prefix", APP, GROUP, "phase0-local-only-extra", RESOURCES),
            ("throw", APP, GROUP, "phase0-throw", RESOURCES),
            ("app", "another-app", GROUP, TOKEN, RESOURCES),
            ("group", APP, "another-group", TOKEN, RESOURCES),
        ];
        if !tm {
            cases.extend([
                ("resource-empty", APP, GROUP, TOKEN, ""),
                ("resource-other", APP, GROUP, TOKEN, "mysql://other"),
                (
                    "resource-mixed",
                    APP,
                    GROUP,
                    TOKEN,
                    "mysql://auth-a,mysql://other",
                ),
                ("resource-trailing", APP, GROUP, TOKEN, "mysql://auth-a,"),
            ]);
        }
        for (name, app, group, credential, resources) in cases {
            let mut stream = register((host, port), tm, app, group, credential, resources, false)?;
            verify_unbound(&mut stream)?;
            count += 1;
            println!(
                "AUTH_DENIED role={} case={name} channelClosed=true transactionRequestCount=1",
                if tm { "TM" } else { "RM" }
            );
        }
    }
    if count != 16 {
        return Err(crate::invalid("incomplete live auth denial matrix"));
    }
    println!("PASS AUTH_DENIED cases={count} noIdentifiedChannel=true");
    Ok(())
}

fn begin(tm: &mut TcpStream, id: &mut u32) -> io::Result<String> {
    let mut value = 60_000u32.to_be_bytes().to_vec();
    crate::write_string16("auth-reconnect", &mut value)?;
    let xid = crate::transaction::parse_begin(&crate::transaction::exchange(tm, *id, 1, &value)?)?;
    *id += 1;
    Ok(xid)
}

fn wait_finished<S: Read + Write>(
    tm: &mut S,
    id: &mut u32,
    xid: &str,
    expected: u8,
) -> io::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    // 已收到匹配的成功结束应答与原分支回调；只查询收敛，不重发结束操作。
    loop {
        let observed = crate::transaction::global_end(tm, id, 15, xid)?;
        println!("AUTH_STATUS xid={xid} expectedTerminal={expected} observedStatus={observed}");
        if observed == 15 {
            println!("AUTH_FINISHED xid={xid} observedStatus=15");
            return Ok(());
        }
        let pending = if expected == 9 {
            matches!(observed, 2 | 3 | 8 | 9)
        } else {
            matches!(observed, 4 | 5 | 11)
        };
        if !pending || std::time::Instant::now() >= deadline {
            return Err(crate::invalid(format!(
                "authenticated target did not finish: {observed}"
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn accepted(host: &str, port: u16) -> io::Result<()> {
    let ready = std::env::var("SEATA_IT_AUTH_RECONNECT_READY")
        .map_err(|_| crate::invalid("auth reconnect requires the owned runner readiness path"))?;
    let proceed = std::env::var("SEATA_IT_AUTH_RECONNECT_CONTINUE").map_err(|_| {
        crate::invalid("auth reconnect requires the owned runner continuation path")
    })?;
    if std::path::Path::new(&ready).exists() || std::path::Path::new(&proceed).exists() {
        return Err(crate::invalid("auth reconnect control files already exist"));
    }
    let mut tm = register((host, port), true, APP, GROUP, TOKEN, RESOURCES, true)?;
    let mut old = register((host, port), false, APP, GROUP, TOKEN, RESOURCES, true)?;
    let mut id = 10000;
    let mut branches = Vec::new();
    // 两个资源的分支都在旧连接注册；新连接必须重新登记整个缓存资源列表，不能重新注册分支。
    for resource in ["mysql://auth-a", "mysql://auth-b"] {
        let xid = begin(&mut tm, &mut id)?;
        let mut request = Vec::new();
        crate::write_string16(&xid, &mut request)?;
        request.push(0);
        crate::write_string16(resource, &mut request)?;
        crate::write_string32("orders:1", &mut request)?;
        crate::write_string32("{}", &mut request)?;
        let branch = crate::branch_probe::parse_branch_id(&crate::transaction::exchange(
            &mut old, id, 11, &request,
        )?)?;
        id += 1;
        println!("AUTH_PREPARED xid={xid} branchId={branch} resource={resource}");
        branches.push((xid, branch, resource));
    }
    let old_port = old.local_addr()?.port();
    old.shutdown(Shutdown::Both)?;
    drop(old);
    let mut rm = register((host, port), false, APP, GROUP, TOKEN, RESOURCES, true)?;
    let new_port = rm.local_addr()?.port();
    if new_port == old_port {
        return Err(crate::invalid(
            "RM reconnect did not demonstrate a fresh TCP endpoint",
        ));
    }
    println!("AUTH_RECONNECTED oldPort={old_port} newPort={new_port} resources={RESOURCES}");
    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ready)?;
    writeln!(marker, "{}\t{old_port}\t{new_port}", std::process::id())?;
    marker.sync_all()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match std::fs::read_to_string(&proceed) {
            Ok(value) if value.trim() == std::process::id().to_string() => break,
            Ok(_) => {
                return Err(crate::invalid(
                    "auth reconnect continuation belongs to another process",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if std::time::Instant::now() >= deadline {
            return Err(crate::invalid(
                "old RM disconnect was not confirmed by the owned runner",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for (index, (xid, branch, resource)) in branches.into_iter().enumerate() {
        let end = if index == 0 { 7 } else { 9 };
        let callback = if end == 7 { 3 } else { 5 };
        let (decision, reply) = std::thread::scope(|scope| {
            let worker=scope.spawn(||->io::Result<()>{
                let request=crate::branch_end_request::BranchEndRequest::read(&mut rm)?;
                let mut state=crate::branch_callback_state::BranchCallbackState::new(callback,&xid,branch,resource);
                let mut actions=0;let frame=state.reply(&request,||{actions+=1;Ok(())})?;
                rm.write_all(&frame)?;rm.flush()?;
                if actions!=1{return Err(crate::invalid("auth reconnect action count differs"));}
                println!("AUTH_CALLBACK xid={xid} branchId={branch} resource={resource} kind={callback} requestId={} actions={actions}",request.request_id);Ok(())
            });
            let decision = crate::transaction::global_end(&mut tm, &mut id, end, &xid);
            let reply = worker
                .join()
                .unwrap_or_else(|_| Err(crate::invalid("auth reconnect worker panicked")));
            (decision, reply)
        });
        reply?;
        if decision? != if end == 7 { 9 } else { 11 } {
            return Err(crate::invalid("authenticated end was not acknowledged"));
        }
        wait_finished(&mut tm, &mut id, &xid, if end == 7 { 9 } else { 11 })?;
    }
    println!("PASS AUTH_ACCEPTED reconnect=true cachedResources=2 originalBranchCallbacks=2 branchReregistrations=0");
    Ok(())
}

/// 运行认证开关／扩展拒绝／认证后重连探针，输入独立端点及模式，返回现场验证结果。
/// 对应 Java: RegTmProcessor、RegRmProcessor、RegisterCheckAuthHandler；不是生产凭据 API。
pub(super) fn run(host: &str, port: u16, mode: &str) -> io::Result<()> {
    if host != "127.0.0.1" || port != 28091 {
        return Err(crate::invalid(
            "auth fault probes require isolated loopback port28091",
        ));
    }
    match mode {
        "--auth-denied" => denied(host, port),
        "--auth-accepted" => accepted(host, port),
        "--auth-default" | "--auth-bypass" => {
            for tm in [true, false] {
                let _channel = register(
                    (host, port),
                    tm,
                    APP,
                    GROUP,
                    if mode == "--auth-default" {
                        ""
                    } else {
                        "wrong"
                    },
                    RESOURCES,
                    true,
                )?;
            }
            println!("PASS AUTH_CONTROL mode={mode} roles=2 identified=true");
            Ok(())
        }
        _ => Err(crate::invalid("unsupported auth probe mode")),
    }
}

#[cfg(test)]
mod tests {
    use super::{body, verify_unbound, wait_finished, APP, GROUP, RESOURCES, TOKEN};
    use std::io::{self, Read, Write};
    struct ClosedProbe {
        result: Option<io::ErrorKind>,
        data: bool,
        written: Vec<u8>,
    }
    impl Read for ClosedProbe {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            if let Some(kind) = self.result {
                return Err(io::Error::new(kind, "test"));
            }
            if self.data {
                b[0] = 0xda;
                Ok(1)
            } else {
                Ok(0)
            }
        }
    }
    impl Write for ClosedProbe {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    struct StatusChannel {
        input: io::Cursor<Vec<u8>>,
        written: Vec<u8>,
    }
    impl Read for StatusChannel {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.input.read(b)
        }
    }
    impl Write for StatusChannel {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn status_channel(statuses: &[u8]) -> StatusChannel {
        let input = statuses
            .iter()
            .enumerate()
            .flat_map(|(index, status)| {
                let mut frame =
                    crate::build_v1_request(16, 100 + index as u32, &[1, 0, *status]).unwrap();
                frame[9] = 1;
                frame
            })
            .collect();
        StatusChannel {
            input: io::Cursor::new(input),
            written: vec![],
        }
    }
    #[test]
    fn asynchronous_end_is_observed_until_finished_with_only_status_queries() {
        for (statuses, expected) in [(&[8, 9, 15][..], 9), (&[4, 11, 15][..], 11)] {
            let mut stream = status_channel(statuses);
            let mut id = 100;
            wait_finished(&mut stream, &mut id, "original", expected).unwrap();
            assert_eq!(id, 103);
            let mut at = 0;
            while at < stream.written.len() {
                let size =
                    u32::from_be_bytes(stream.written[at + 3..at + 7].try_into().unwrap()) as usize;
                assert_eq!(&stream.written[at + 16..at + 18], &[0, 15]);
                at += size;
            }
        }
    }
    #[test]
    fn opposite_or_begin_status_is_not_successful_completion() {
        for (status, expected) in [(11, 9), (9, 11), (1, 9)] {
            let mut stream = status_channel(&[status]);
            let mut id = 100;
            assert!(wait_finished(&mut stream, &mut id, "original", expected).is_err());
            assert_eq!(id, 101);
        }
    }
    #[test]
    fn credential_is_encoded_in_the_java_extra_data_slot_for_both_roles() {
        let expected = b"\0\x051.1.0\0\x0bphase0-auth\0\x10default_tx_group\0\x11phase0-local-only";
        assert_eq!(body(true, APP, GROUP, TOKEN, RESOURCES).unwrap(), expected);
        let mut rm = expected.to_vec();
        rm.extend_from_slice(&[0, 0, 0, 29]);
        rm.extend_from_slice(RESOURCES.as_bytes());
        assert_eq!(body(false, APP, GROUP, TOKEN, RESOURCES).unwrap(), rm);
    }
    #[test]
    fn denied_registration_must_not_treat_timeout_or_data_as_unbound_proof() {
        for (result, data) in [(Some(io::ErrorKind::TimedOut), false), (None, true)] {
            let mut stream = ClosedProbe {
                result,
                data,
                written: vec![],
            };
            assert!(verify_unbound(&mut stream).is_err());
        }
    }
    #[test]
    fn unbound_probe_sends_one_begin_and_requires_actual_connection_closure() {
        let mut stream = ClosedProbe {
            result: None,
            data: false,
            written: vec![],
        };
        verify_unbound(&mut stream).unwrap();
        assert!(
            stream.written.len() >= 18,
            "missing single transaction stimulus"
        );
        assert_eq!(&stream.written[16..18], &[0, 1]);
        assert_eq!(
            stream.written.len(),
            u32::from_be_bytes(stream.written[3..7].try_into().unwrap()) as usize
        );
    }
}
