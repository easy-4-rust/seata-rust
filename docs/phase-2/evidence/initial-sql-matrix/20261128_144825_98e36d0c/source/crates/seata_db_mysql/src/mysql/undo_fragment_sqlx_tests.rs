use super::{undo_fragment_layout::UndoFragmentLayout, MySqlUndoLogManager};
use crate::{
    mysql_loopback_fixture::{authenticate, read_command, write_error, write_packet},
    DbError,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use std::{error::Error, io, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};

#[derive(Clone, Copy)]
enum InsertReply {
    Success,
    RejectSecondExecute,
    RejectPrepare,
    DisconnectDuringPrepare,
}

const SQL: &[u8] = b"\x16INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,0,now(6),now(6))";

#[tokio::test]
async fn original_fragment_insert_binds_and_releases_each_statement() -> Result<(), Box<dyn Error>>
{
    timeout(Duration::from_secs(8), verify_inserts(InsertReply::Success)).await?
}

#[tokio::test]
async fn second_fragment_error_stops_root_insert_and_releases_statement(
) -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        verify_inserts(InsertReply::RejectSecondExecute),
    )
    .await?
}

#[tokio::test]
async fn insert_prepare_rejection_preserves_server_diagnostics() -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        verify_inserts(InsertReply::RejectPrepare),
    )
    .await?
}

#[tokio::test]
async fn insert_prepare_disconnect_retains_io_cause() -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        verify_inserts(InsertReply::DisconnectDuringPrepare),
    )
    .await?
}

#[test]
fn driver_error_preserves_os_code_and_excludes_custom_message() -> Result<(), Box<dyn Error>> {
    let error = DbError::driver("fixture", sqlx::Error::Io(io::Error::from_raw_os_error(2)));
    let cause = error
        .source()
        .and_then(|source| source.downcast_ref::<io::Error>());
    assert_eq!(cause.and_then(io::Error::raw_os_error), Some(2));

    let error = DbError::driver(
        "fixture",
        sqlx::Error::Io(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "hidden peer detail",
        )),
    );
    let cause = error
        .source()
        .and_then(|source| source.downcast_ref::<io::Error>())
        .ok_or("expected sanitized I/O cause")?;
    assert_eq!(cause.kind(), io::ErrorKind::ConnectionReset);
    assert!(!format!("{error:?} {error} {cause:?} {cause}").contains("hidden peer detail"));
    Ok(())
}

async fn verify_inserts(reply: InsertReply) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let peer = tokio::spawn(serve(listener, reply));
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect_lazy_with(
            MySqlConnectOptions::new()
                .host("127.0.0.1")
                .port(port)
                .username("fixture")
                .ssl_mode(MySqlSslMode::Disabled),
        );
    let mut tx = pool.begin().await?;
    let mut layout = UndoFragmentLayout::new(42, 5, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])?;
    layout.add_child(101)?;
    layout.add_child(102)?;
    let result = MySqlUndoLogManager::write_fragments(&mut tx, "xid", &layout).await;
    match reply {
        InsertReply::Success => result?,
        InsertReply::RejectSecondExecute => assert!(matches!(
            result,
            Err(DbError::Driver { operation: "Undo insert", code: Some(1062), sql_state: Some(state), .. })
                if state == "23000"
        )),
        InsertReply::RejectPrepare => assert!(matches!(
            result,
            Err(DbError::Driver { operation: "Undo insert", code: Some(1142), sql_state: Some(state), .. })
                if state == "42000"
        )),
        InsertReply::DisconnectDuringPrepare => {
            let failure = result.err().ok_or("disconnected fixture accepted INSERT")?;
            assert!(matches!(
                failure,
                DbError::Driver {
                    operation: "Undo insert",
                    ..
                }
            ));
            let original_io = failure
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>());
            assert_eq!(
                original_io.map(io::Error::kind),
                Some(io::ErrorKind::UnexpectedEof),
                "lost INSERT PREPARE I/O cause: {failure:?}",
            );
        }
    }

    // 响应完整时由原调用者回滚。断连无法收到清理确认。
    let rollback = tx.rollback().await;
    if matches!(reply, InsertReply::DisconnectDuringPrepare) {
        assert!(rollback.is_err());
    } else {
        rollback?;
    }
    pool.close().await;
    peer.await??;
    Ok(())
}

async fn serve(listener: TcpListener, reply: InsertReply) -> io::Result<()> {
    let (mut socket, _) = listener.accept().await?;
    authenticate(&mut socket, 1).await?;
    expect_command(&mut socket, b"\x03BEGIN").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 3, 0, 0, 0]).await?;

    let rows: [(i64, &[u8], &[u8]); 3] = [
        (101, b"branchId=42", &[4, 5, 6, 7]),
        (102, b"branchId=42", &[8, 9]),
        (
            42,
            b"serializer=jackson&compressorType=NONE&map=5&subId=101,102,",
            &[0, 1, 2, 3],
        ),
    ];
    for (index, (branch_id, context, bytes)) in rows.into_iter().enumerate() {
        let id = index as u32 + 1;
        expect_command(&mut socket, SQL).await?;
        match reply {
            InsertReply::RejectPrepare => {
                write_error(&mut socket, 1142, b"42000").await?;
                break;
            }
            InsertReply::DisconnectDuringPrepare => return Ok(()),
            _ => {}
        }
        prepare_reply(&mut socket, id).await?;
        let mut execute = statement_command(0x17, id);
        // 无游标、一次执行、无空参数，并附带四个参数的 MySQL 类型。
        execute.extend_from_slice(&[0, 1, 0, 0, 0, 0, 1, 0x08, 0, 0xfd, 0, 0xfd, 0, 0xfc, 0]);
        execute.extend_from_slice(&branch_id.to_le_bytes());
        for payload in [b"xid".as_slice(), context, bytes] {
            execute.push(u8::try_from(payload.len()).map_err(io::Error::other)?);
            execute.extend_from_slice(payload);
        }
        expect_command(&mut socket, &execute).await?;
        // SQLx 为一次性预编译语句在 EXECUTE 后关闭，服务器不回复 CLOSE。
        if matches!(reply, InsertReply::RejectSecondExecute) && id == 2 {
            write_error(&mut socket, 1062, b"23000").await?;
            expect_command(&mut socket, &statement_command(0x19, id)).await?;
            break;
        }
        write_packet(&mut socket, 1, &[0, 1, 0, 3, 0, 0, 0]).await?;
        expect_command(&mut socket, &statement_command(0x19, id)).await?;
    }

    expect_command(&mut socket, b"\x03ROLLBACK").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 2, 0, 0, 0]).await?;
    expect_command(&mut socket, b"\x01").await
}

async fn expect_command(socket: &mut TcpStream, expected: &[u8]) -> io::Result<()> {
    let actual = read_command(socket).await?;
    assert_eq!(actual.as_deref(), Some(expected));
    Ok(())
}

fn statement_command(opcode: u8, id: u32) -> Vec<u8> {
    let mut packet = vec![opcode];
    packet.extend_from_slice(&id.to_le_bytes());
    packet
}

async fn prepare_reply(socket: &mut TcpStream, id: u32) -> io::Result<()> {
    let mut prepared = statement_command(0, id);
    prepared.extend_from_slice(&[0, 0, 4, 0, 0, 0, 0]);
    write_packet(socket, 1, &prepared).await?;
    let parameter = [
        3, b'd', b'e', b'f', 0, 0, 0, 1, b'?', 1, b'?', 12, 45, 0, 0, 0, 0, 0, 0xfd, 0, 0, 0, 0, 0,
    ];
    for sequence in 2..=5 {
        write_packet(socket, sequence, &parameter).await?;
    }
    write_packet(socket, 6, &[0xfe, 0, 0, 3, 0]).await
}
