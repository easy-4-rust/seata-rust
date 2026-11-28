use super::MySqlUndoLogManager;
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

const ROOT_SQL: &[u8] = b"\x16DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0 AND CAST(xid AS BINARY)=CAST(? AS BINARY)";
const CHILD_SQL: &[u8] = b"\x16DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0 AND CAST(xid AS BINARY)=CAST(? AS BINARY) AND CAST(context AS BINARY)=CAST(? AS BINARY)";
const XID: &str = "xid-Exact";

#[derive(Clone, Copy)]
enum SecondChildReply {
    Deleted,
    NoMatch,
    DriverFailure,
}

#[tokio::test]
async fn deletes_children_then_root_with_exact_xid_and_parent() -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        verify_cleanup(SecondChildReply::Deleted),
    )
    .await?
}

#[tokio::test]
async fn zero_row_or_driver_error_on_child_stops_before_root_delete() -> Result<(), Box<dyn Error>>
{
    timeout(Duration::from_secs(8), async {
        verify_cleanup(SecondChildReply::NoMatch).await?;
        verify_cleanup(SecondChildReply::DriverFailure).await
    })
    .await?
}

async fn verify_cleanup(reply: SecondChildReply) -> Result<(), Box<dyn Error>> {
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
    let cleanup = MySqlUndoLogManager::delete_normal(&mut tx, XID, 42, &[102, 101]).await;
    match reply {
        SecondChildReply::Deleted => cleanup?,
        SecondChildReply::NoMatch => assert!(matches!(
            cleanup,
            Err(DbError::Unsupported {
                feature: "Undo exact deleted row count"
            })
        )),
        SecondChildReply::DriverFailure => assert!(matches!(cleanup, Err(DbError::Driver {
            operation: "Undo normal delete", code: Some(1213), sql_state: Some(state), ..
        }) if state == "40001")),
    }
    tx.rollback().await?;
    pool.close().await;
    peer.await??;
    Ok(())
}

async fn serve(listener: TcpListener, reply: SecondChildReply) -> io::Result<()> {
    let (mut socket, _) = listener.accept().await?;
    authenticate(&mut socket, 1).await?;
    expect(&mut socket, b"\x03BEGIN").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 3, 0, 0, 0]).await?;

    expect(&mut socket, CHILD_SQL).await?;
    prepare_reply(&mut socket, 1, 4).await?;
    expect(&mut socket, &execute(1, 102, Some("branchId=42"))).await?;
    write_packet(&mut socket, 1, &[0, 1, 0, 3, 0, 0, 0]).await?;
    expect(&mut socket, &execute(1, 101, Some("branchId=42"))).await?;

    match reply {
        SecondChildReply::Deleted => {
            write_packet(&mut socket, 1, &[0, 1, 0, 3, 0, 0, 0]).await?;
            expect(&mut socket, ROOT_SQL).await?;
            prepare_reply(&mut socket, 2, 3).await?;
            expect(&mut socket, &execute(2, 42, None)).await?;
            write_packet(&mut socket, 1, &[0, 1, 0, 3, 0, 0, 0]).await?;
        }
        SecondChildReply::NoMatch => write_packet(&mut socket, 1, &[0, 0, 0, 3, 0, 0, 0]).await?,
        SecondChildReply::DriverFailure => write_error(&mut socket, 1213, b"40001").await?,
    }
    expect(&mut socket, b"\x03ROLLBACK").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 2, 0, 0, 0]).await?;
    expect(&mut socket, b"\x01").await
}

fn execute(statement: u32, branch: i64, context: Option<&str>) -> Vec<u8> {
    let mut bytes = vec![0x17];
    bytes.extend_from_slice(&statement.to_le_bytes());
    bytes.extend_from_slice(&[0, 1, 0, 0, 0, 0, 1, 0xfd, 0, 8, 0, 0xfd, 0]);
    if context.is_some() {
        bytes.extend_from_slice(&[0xfd, 0]);
    }
    bytes.push(XID.len() as u8);
    bytes.extend_from_slice(XID.as_bytes());
    bytes.extend_from_slice(&branch.to_le_bytes());
    bytes.push(XID.len() as u8);
    bytes.extend_from_slice(XID.as_bytes());
    if let Some(context) = context {
        bytes.push(context.len() as u8);
        bytes.extend_from_slice(context.as_bytes());
    }
    bytes
}

async fn prepare_reply(socket: &mut TcpStream, id: u32, count: u8) -> io::Result<()> {
    let mut prepared = vec![0];
    prepared.extend_from_slice(&id.to_le_bytes());
    prepared.extend_from_slice(&[0, 0, count, 0, 0, 0, 0]);
    write_packet(socket, 1, &prepared).await?;
    let parameter = [
        3, b'd', b'e', b'f', 0, 0, 0, 1, b'?', 1, b'?', 12, 45, 0, 0, 0, 0, 0, 0xfd, 0, 0, 0, 0, 0,
    ];
    for sequence in 2..count + 2 {
        write_packet(socket, sequence, &parameter).await?;
    }
    write_packet(socket, count + 2, &[0xfe, 0, 0, 3, 0]).await
}

async fn expect(socket: &mut TcpStream, command: &[u8]) -> io::Result<()> {
    assert_eq!(read_command(socket).await?.as_deref(), Some(command));
    Ok(())
}
