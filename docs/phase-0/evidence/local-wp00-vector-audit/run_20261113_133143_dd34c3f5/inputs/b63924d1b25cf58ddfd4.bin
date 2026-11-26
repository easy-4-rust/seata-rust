use crate::branch_callback_state::BranchCallbackState;
use crate::branch_end_request::BranchEndRequest;
use crate::pending_branch::PendingBranch;
use crate::unknown_outcome::UnknownOutcome;
use std::io::{self, Write};
use std::net::TcpStream;

fn begin(tm: &mut TcpStream, id: &mut u32) -> io::Result<String> {
    let mut body = 60_000_u32.to_be_bytes().to_vec();
    crate::write_string16("seata-rust-it-response-loss", &mut body)?;
    let xid = crate::transaction::parse_begin(&crate::transaction::exchange(tm, *id, 1, &body)?)?;
    *id += 1;
    Ok(xid)
}

fn branch_body(xid: &str, resource: &str, data: &str) -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    crate::write_string16(xid, &mut body)?;
    body.push(0);
    crate::write_string16(resource, &mut body)?;
    crate::write_string32("orders:1", &mut body)?;
    crate::write_string32(data, &mut body)?;
    Ok(body)
}

fn lock(
    rm: &mut TcpStream,
    id: &mut u32,
    target: &str,
    contender: &str,
    resource: &str,
    expected: bool,
) -> io::Result<()> {
    if contender == target {
        return Err(crate::invalid("lock contender must have a different XID"));
    }
    let body = branch_body(contender, resource, "{}")?;
    let actual =
        crate::restart_probe::parse_lockable(&crate::transaction::exchange(rm, *id, 21, &body)?)?;
    *id += 1;
    println!("LOSS_LOCK targetXid={target} contenderXid={contender} lockable={actual} expected={expected}");
    if actual != expected {
        return Err(crate::invalid(
            "response-loss lock occupation/release mismatch",
        ));
    }
    Ok(())
}

/// 在已注册通道验证指定一次响应丢失（11/7/9）；返回实际未知结果、回调和锁清理的验证结果。
/// 对应 Java: AbstractCore#branchRegister、DefaultCore#commit/#rollback；仅模拟分支探针。
pub(super) fn verify(
    tm: &mut TcpStream,
    rm: &mut TcpStream,
    resource: &str,
    operation: u16,
) -> io::Result<()> {
    if !matches!(operation, 7 | 9 | 11) {
        return Err(crate::invalid("unsupported loss scenario"));
    }
    let mut id = 6000;
    let xid = begin(tm, &mut id)?;
    let lost_registration = operation == 11;
    let data = if lost_registration {
        format!("{{\"itRequest\":{id}}}")
    } else {
        "{}".to_owned()
    };
    let body = branch_body(&xid, resource, &data)?;
    let mut uncertain = None;
    let known_branch = if lost_registration {
        uncertain = Some(UnknownOutcome::send_once(rm, 11, id, &xid, &body)?);
        None
    } else {
        Some(crate::branch_probe::parse_branch_id(
            &crate::transaction::exchange(rm, id, 11, &body)?,
        )?)
    };
    id += 1;
    println!("LOSS_TARGET operation={operation} xid={xid} initialBranchId={known_branch:?} applicationData={data}");
    if crate::transaction::global_end(tm, &mut id, 15, &xid)? != 1 {
        return Err(crate::invalid("target must be Begin before end"));
    }
    let contender = begin(tm, &mut id)?;
    lock(rm, &mut id, &xid, &contender, resource, false)?;
    let end = if operation == 7 { 7 } else { 9 };
    let callback = if end == 7 { 3 } else { 5 };
    let (decision, callback_result) = std::thread::scope(|scope| {
        let worker=scope.spawn(||->io::Result<(i64,u32)> {
            let request=BranchEndRequest::read(rm)?;
            let mut actions=0;
            let (frame,branch_id)=if lost_registration {
                let mut pending=PendingBranch::new(&xid,resource,&data);
                if pending.learned_branch_id.is_some() {return Err(crate::invalid("lost response leaked a BranchId before callback"));}
                let frame=pending.reply(&request,||{actions+=1;Ok(())})?;
                (frame,pending.learned_branch_id.ok_or_else(||crate::invalid("validated callback did not identify branch"))?)
            } else {
                let branch_id=known_branch.ok_or_else(||crate::invalid("known branch identity missing"))?;
                let mut state=BranchCallbackState::new(callback,&xid,branch_id,resource);
                (state.reply(&request,||{actions+=1;Ok(())})?,branch_id)
            };
            rm.write_all(&frame)?;rm.flush()?;
            println!("LOSS_CALLBACK xid={xid} branchId={branch_id} type={callback} requestId={} learnedFromCallback={lost_registration} actions={actions}",request.request_id);
            if actions!=1 {return Err(crate::invalid("simulated branch action count is not one"));}
            Ok((branch_id,actions))
        });
        let decision = if lost_registration {
            crate::transaction::global_end(tm, &mut id, end, &xid).and_then(|status| {
                if status == 11 {
                    Ok(())
                } else {
                    Err(crate::invalid(
                        "uncertain-registration cancellation was not acknowledged",
                    ))
                }
            })
        } else {
            let mut body = Vec::new();
            let result = crate::write_string16(&xid, &mut body)
                .and_then(|_| crate::write_string16("", &mut body))
                .and_then(|_| UnknownOutcome::send_once(tm, end, id, &xid, &body));
            match result {
                Ok(value) => {
                    uncertain = Some(value);
                    id += 1;
                    Ok(())
                }
                Err(error) => Err(error),
            }
        };
        let callback_result = worker
            .join()
            .unwrap_or_else(|_| Err(crate::invalid("response-loss callback worker panicked")));
        (decision, callback_result)
    });
    let (branch_id, actions) = callback_result?;
    decision?;
    let mut uncertain = uncertain
        .ok_or_else(|| crate::invalid("response-loss scenario never observed unknown outcome"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let observed = loop {
        let status = crate::transaction::global_end(tm, &mut id, 15, &xid)?;
        let confirmed = uncertain.observe(status)?;
        println!("LOSS_QUERY xid={xid} status={status} confirmedEnd={confirmed}");
        if status == 15 || status == if end == 7 { 9 } else { 11 } {
            break status;
        }
        let pending = if end == 7 {
            matches!(status, 2 | 3 | 8)
        } else {
            matches!(status, 4 | 5)
        };
        if !pending || std::time::Instant::now() >= deadline {
            return Err(crate::invalid(
                "authoritative status did not converge in loss scenario",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    lock(rm, &mut id, &xid, &contender, resource, true)?;
    if crate::transaction::global_end(tm, &mut id, 9, &contender)? != 11 {
        return Err(crate::invalid("lock contender cleanup failed"));
    }
    let confirmed = uncertain.confirmed_status().is_some();
    if !lost_registration && observed == 15 && uncertain.confirmed_status().is_none() {
        println!("UNKNOWN_RETAINED operation={operation} xid={xid} observedStatus=Finished15 reason=no-terminal-proof replayed=false");
    }
    println!("PASS response-loss operation={operation} xid={xid} branchId={branch_id} observedStatus={observed} confirmedEnd={confirmed} registrationReconciled={lost_registration} actions={actions} lockReleased=true replayed=false");
    Ok(())
}
