//! 固定 Java TC 的 FILE 重启验证辅助函数；不实现 Rust TC 恢复。
use crate::restart_record::RestartRecord;
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

/// 输入锁查询响应体，返回明确成功的锁布尔值；失败、截断或非法值不折叠为 false。
/// 对应 Java: GlobalLockQueryResponseCodec#decode。
pub(super) fn parse_lockable(body: &[u8]) -> io::Result<bool> {
    let mut at = 0;
    let (success, code, message) = crate::transaction::prefix(body, &mut at)?;
    if !success || body.len().saturating_sub(at) != 2 {
        return Err(crate::invalid(format!(
            "lock query failed or malformed: code={code} message={message:?}"
        )));
    }
    match u16::from_be_bytes(body[at..].try_into().unwrap()) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::invalid("invalid lockable boolean")),
    }
}

fn branch_body(xid: &str, resource: &str) -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    crate::write_string16(xid, &mut body)?;
    body.push(0);
    crate::write_string16(resource, &mut body)?;
    crate::write_string32("orders:1", &mut body)?;
    crate::write_string32("{}", &mut body)?;
    Ok(body)
}

fn begin<S: Read + Write>(tm: &mut S, id: &mut u32) -> io::Result<String> {
    let mut body = 600_000u32.to_be_bytes().to_vec();
    crate::write_string16("seata-rust-it-file-restart", &mut body)?;
    let xid = crate::transaction::parse_begin(&crate::transaction::exchange(tm, *id, 1, &body)?)?;
    *id += 1;
    Ok(xid)
}

fn check_lock(
    tm: &mut TcpStream,
    rm: &mut TcpStream,
    id: &mut u32,
    record: &RestartRecord,
    expected: bool,
) -> io::Result<()> {
    // 使用独立 XID，避免同一事务可重入锁掩盖丢失的恢复锁。
    let contender = begin(tm, id)?;
    if contender == record.xid {
        return Err(crate::invalid("TC reused recovered XID for lock contender"));
    }
    let body = branch_body(&contender, &record.resource)?;
    let observed = parse_lockable(&crate::transaction::exchange(rm, *id, 21, &body)?)?;
    *id += 1;
    if crate::transaction::global_end(tm, id, 9, &contender)? != 11 {
        return Err(crate::invalid("lock contender cleanup failed"));
    }
    println!("LockQuery recoveredXid={} contenderXid={contender} lockable={observed} expected={expected}",record.xid);
    if observed != expected {
        return Err(crate::invalid("recovered lock state mismatch"));
    }
    Ok(())
}

/// 在已注册通道创建一次 Begin/可选 AT 分支，写入给定 create-only 记录，返回准备结果。
/// 对应 Java: DefaultCore#begin / AbstractCore#branchRegister；不结束事务以供外部强制重启。
pub(super) fn prepare(
    tm: &mut TcpStream,
    rm: &mut TcpStream,
    resource: &str,
    with_branch: bool,
    path: &Path,
) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "restart record already exists; refusing new Begin",
        ));
    }
    let mut id = 4000;
    let xid = begin(tm, &mut id)?;
    // 提前输出 TC 已分配身份，文件写入或后续注册失败时也便于人工定位；不自动重发。
    println!("PREPARE TC-issued xid={xid} withBranch={with_branch}");
    let branch_id = if with_branch {
        let body = branch_body(&xid, resource)?;
        let branch_id = crate::branch_probe::parse_branch_id(&crate::transaction::exchange(
            rm, id, 11, &body,
        )?)?;
        id += 1;
        println!("PREPARE TC-issued xid={xid} branchId={branch_id}");
        branch_id
    } else {
        0
    };
    let record = RestartRecord {
        xid,
        branch_id,
        resource: resource.to_owned(),
    };
    record.save(path)?;
    if crate::transaction::global_end(tm, &mut id, 15, &record.xid)? != 1 {
        return Err(crate::invalid("prepared transaction is not Begin"));
    }
    if with_branch {
        check_lock(tm, rm, &mut id, &record, false)?;
    }
    println!(
        "PASS prepared-restart xid={} branchId={} status=Begin record={}",
        record.xid,
        record.branch_id,
        path.display()
    );
    Ok(())
}

/// 用新进程重新注册后的通道验证原身份恢复并回滚；返回会话、分支及锁观测的验证结果。
/// 对应 Java: SessionHolder#reload / DefaultCore#rollback；不重新创建目标全局事务或分支。
pub(super) fn resume(
    tm: &mut TcpStream,
    rm: &mut TcpStream,
    resource: &str,
    path: &Path,
) -> io::Result<()> {
    let record = RestartRecord::load(path)?;
    if record.resource != resource {
        return Err(crate::invalid(
            "recorded resource differs from registered resource",
        ));
    }
    let mut id = 5000;
    let status = crate::transaction::global_end(tm, &mut id, 15, &record.xid)?;
    println!(
        "RELOAD originalXid={} originalBranchId={} observedStatus={status}",
        record.xid, record.branch_id
    );
    if status != 1 {
        return Err(crate::invalid("original session did not recover as Begin"));
    }
    let decision = if record.branch_id > 0 {
        check_lock(tm, rm, &mut id, &record, false)?;
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                crate::branch_probe::acknowledge_callback(
                    rm,
                    5,
                    &record.xid,
                    record.branch_id,
                    resource,
                )
            });
            let decision = crate::transaction::global_end(tm, &mut id, 9, &record.xid);
            worker
                .join()
                .map_err(|_| crate::invalid("recovery RM worker panicked"))??;
            decision
        })?
    } else {
        crate::transaction::global_end(tm, &mut id, 9, &record.xid)?
    };
    if decision != 11 {
        return Err(crate::invalid(format!(
            "recovered rollback status={decision}, expected 11"
        )));
    }
    if crate::transaction::global_end(tm, &mut id, 15, &record.xid)? != 15 {
        return Err(crate::invalid("recovered session cleanup not observed"));
    }
    if record.branch_id > 0 {
        check_lock(tm, rm, &mut id, &record, true)?;
    }
    println!(
        "PASS recovered-restart xid={} branchId={} rollbackStatus=11 finalStatus=15",
        record.xid, record.branch_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_lockable;
    #[test]
    fn lock_query_boolean_uses_java_short_and_preserves_false() {
        assert!(parse_lockable(&[1, 0, 0, 1]).unwrap());
        assert!(!parse_lockable(&[1, 0, 0, 0]).unwrap());
    }
    #[test]
    fn failed_or_malformed_lock_query_is_not_lockable_false() {
        for body in [
            &[1, 0, 0][..],
            &[1, 0, 0, 2],
            &[1, 0, 0, 1, 0],
            &[0, 0, 0, 0, 0, 0],
        ] {
            assert!(parse_lockable(body).is_err());
        }
    }
}
