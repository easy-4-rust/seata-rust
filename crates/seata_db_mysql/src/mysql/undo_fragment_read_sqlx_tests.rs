use super::MySqlUndoLogManager;
use crate::{
    mysql_loopback_fixture::{authenticate, read_command, write_error, write_packet},
    DbError,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use std::{
    error::Error,
    io,
    num::{IntErrorKind, ParseIntError},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::timeout,
};

const ROOT_QUERY: &[u8] = b"SELECT xid,branch_id,context,log_status,CAST(OCTET_LENGTH(rollback_info) AS SIGNED) AS bytes FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE";
const MEMBERS_QUERY: &[u8] =
    b"SELECT branch_id FROM undo_log WHERE xid=? AND context=? LIMIT 65 FOR UPDATE";
const BODY_QUERY: &[u8] =
    b"SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE";
const CHILD_BODY_QUERY: &[u8] =
    b"SELECT branch_id,rollback_info FROM undo_log WHERE branch_id IN (?, ?) AND xid=? FOR UPDATE";
const ROOT_CONTEXT: &[u8] = b"serializer=jackson&compressorType=NONE&map=1024&subId=102,101,";
const BLANK_SUBID: &[u8] = b"serializer=jackson&compressorType=NONE&subId= ";
const FOREIGN_SPACE_SUBID: &[u8] =
    "serializer=jackson&compressorType=NONE&subId=\u{00a0}".as_bytes();
const XID: &str = "xid";

#[derive(Debug)]
enum ReadFailure {
    Unsupported(&'static str),
    InvalidNumber(IntErrorKind),
    ChildPrepareRejected,
}
const HEADER: &[(&[u8], u8)] = &[
    (b"xid", 0xfd),
    (b"branch_id", 8),
    (b"context", 0xfd),
    (b"log_status", 3),
    (b"bytes", 8),
];
const MEMBERS: &[(&[u8], u8)] = &[(b"branch_id", 8)];
const BODY: &[(&[u8], u8)] = &[(b"rollback_info", 0xfc)];
const CHILD_BODY: &[(&[u8], u8)] = &[(b"branch_id", 8), (b"rollback_info", 0xfc)];
const ROOT_BYTES: &[u8] = &[1, 2];

fn child_bytes(id: i64) -> &'static [u8] {
    match id {
        101 => &[1, 2],
        102 => &[3],
        _ => unreachable!("unexpected fixture child id"),
    }
}

#[tokio::test]
async fn fragment_membership_row_order_does_not_change_root_declared_body_order(
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), verify_fragments(&[101, 102], true)).await?
}

#[tokio::test]
async fn duplicate_child_rows_cannot_hide_a_missing_child_before_body_reads(
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), verify_fragments(&[102, 102], false)).await?
}

#[tokio::test]
async fn orphan_or_missing_children_are_rejected_before_body_reads() -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), async {
        verify_fragments(&[102, 103], false).await?;
        verify_fragments(&[102], false).await
    })
    .await?
}

#[tokio::test]
async fn blank_subid_accepts_root_only_and_still_rejects_orphan() -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), async {
        for context in [
            BLANK_SUBID,
            b"serializer=jackson&compressorType=NONE&subId=".as_slice(),
            b"serializer=jackson&compressorType=NONE&subId=\x1c\t".as_slice(),
        ] {
            verify_read(context, &[], Some(&[]), Ok(&[1, 2])).await?;
        }
        verify_read(
            BLANK_SUBID,
            &[],
            Some(&[103]),
            Err(ReadFailure::Unsupported(
                "Undo orphan/missing child membership",
            )),
        )
        .await
    })
    .await?
}

#[tokio::test]
async fn non_java_whitespace_subid_stops_before_membership_or_body() -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        verify_read(
            FOREIGN_SPACE_SUBID,
            &[],
            None,
            Err(ReadFailure::InvalidNumber(IntErrorKind::InvalidDigit)),
        ),
    )
    .await?
}

#[tokio::test]
async fn invalid_subid_preserves_numeric_cause_before_sql_for_children(
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), async {
        for (context, kind) in [
            (
                b"serializer=jackson&compressorType=NONE&subId=invalid".as_slice(),
                IntErrorKind::InvalidDigit,
            ),
            (
                b"serializer=jackson&compressorType=NONE&subId=9223372036854775808,".as_slice(),
                IntErrorKind::PosOverflow,
            ),
        ] {
            verify_read(context, &[], None, Err(ReadFailure::InvalidNumber(kind))).await?;
        }
        verify_read(
            b"serializer=jackson&compressorType=NONE&subId=101,102,,",
            &[],
            None,
            Err(ReadFailure::InvalidNumber(IntErrorKind::Empty)),
        )
        .await
    })
    .await?
}

#[tokio::test]
async fn reads_original_child_ids_payloads_and_closes_the_batch_statement(
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), async {
        for context in [
            b"serializer=jackson&compressorType=NONE&subId=101,102".as_slice(),
            b"serializer=jackson&compressorType=NONE&subId=101,102,".as_slice(),
        ] {
            verify_read(
                context,
                &[101, 102],
                Some(&[101, 102]),
                Ok(&[1, 2, 1, 2, 3]),
            )
            .await?;
        }
        verify_read(
            b"serializer=jackson&compressorType=NONE&subId=101,102",
            &[101, 102],
            Some(&[101, 103]),
            Err(ReadFailure::Unsupported(
                "Undo orphan/missing child membership",
            )),
        )
        .await
    })
    .await?
}

#[tokio::test]
async fn child_batch_prepare_rejection_preserves_server_diagnostics() -> Result<(), Box<dyn Error>>
{
    timeout(
        Duration::from_secs(8),
        verify_read(
            b"serializer=jackson&compressorType=NONE&subId=101",
            &[101],
            Some(&[101]),
            Err(ReadFailure::ChildPrepareRejected),
        ),
    )
    .await?
}

async fn verify_fragments(members: &'static [i64], accept: bool) -> Result<(), Box<dyn Error>> {
    let expected: Result<&[u8], ReadFailure> = if accept {
        Ok(&[1, 2, 3, 1, 2])
    } else {
        Err(ReadFailure::Unsupported(
            "Undo orphan/missing child membership",
        ))
    };
    verify_read(ROOT_CONTEXT, &[102, 101], Some(members), expected).await
}

async fn verify_read(
    root_context: &'static [u8],
    declared: &'static [i64],
    members: Option<&'static [i64]>,
    expected: Result<&[u8], ReadFailure>,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (last_row_sent, wait_for_last_row) = oneshot::channel();
    let (allow_eof, wait_to_send_eof) = oneshot::channel();
    let peer = tokio::spawn(serve(
        listener,
        root_context,
        declared,
        members,
        matches!(&expected, Err(ReadFailure::ChildPrepareRejected)),
        expected
            .is_ok()
            .then_some((last_row_sent, wait_to_send_eof)),
    ));
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
    let result = {
        let mut reading = std::pin::pin!(MySqlUndoLogManager::read_normal(&mut tx, XID, 42));
        if expected.is_ok() {
            tokio::select! {
                _ = &mut reading => return Err("body read finished before the final row".into()),
                ready = wait_for_last_row => ready?,
            }
            assert!(
                timeout(Duration::from_millis(100), &mut reading)
                    .await
                    .is_err(),
                "body read finished before the final result-set terminator"
            );
            allow_eof
                .send(())
                .map_err(|()| "fixture ended before final EOF")?;
        }
        reading.await
    };
    match expected {
        Ok(bytes) => {
            let (actual, ids) = result?;
            assert_eq!(actual, bytes);
            assert_eq!(ids, declared);
        }
        Err(ReadFailure::Unsupported(expected)) => assert!(
            matches!(result, Err(DbError::Unsupported { feature }) if feature == expected),
            "unexpected read result: {result:?}"
        ),
        Err(ReadFailure::ChildPrepareRejected) => assert!(matches!(
            result,
            Err(DbError::Driver { operation: "Undo child body", sql_state: Some(state), code: Some(1142), .. })
                if state == "42000"
        )),
        Err(ReadFailure::InvalidNumber(expected)) => {
            let failure = result.err().ok_or("invalid subId was accepted")?;
            let kind = failure
                .source()
                .and_then(|source| source.downcast_ref::<ParseIntError>())
                .map(ParseIntError::kind);
            assert_eq!(kind, Some(&expected), "missing parse cause: {failure:?}");
        }
    }
    tx.rollback().await?;
    pool.close().await;
    peer.await??;
    Ok(())
}

async fn serve(
    listener: TcpListener,
    root_context: &[u8],
    declared: &[i64],
    members: Option<&[i64]>,
    reject_child_prepare: bool,
    mut final_result: Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>,
) -> io::Result<()> {
    let (mut socket, _) = listener.accept().await?;
    authenticate(&mut socket, 1).await?;
    expect(&mut socket, b"\x03BEGIN").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 3, 0, 0, 0]).await?;

    prepare(&mut socket, 1, ROOT_QUERY, HEADER, 2).await?;
    let root = (42, root_context, ROOT_BYTES.len() as i64);
    let children = declared
        .iter()
        .map(|&id| (id, b"branchId=42".as_slice(), child_bytes(id).len() as i64));
    for (id, context, size) in std::iter::once(root).chain(children) {
        expect(
            &mut socket,
            &execute(
                1,
                &[0xfd, 8],
                &[&binary_string(XID.as_bytes()), &id.to_le_bytes()],
            ),
        )
        .await?;
        let header = [
            binary_string(XID.as_bytes()),
            id.to_le_bytes().to_vec(),
            binary_string(context),
            0i32.to_le_bytes().to_vec(),
            size.to_le_bytes().to_vec(),
        ]
        .concat();
        result_set(&mut socket, HEADER, &[header]).await?;
    }

    if let Some(members) = members {
        send_members(&mut socket, members).await?;
    }
    if reject_child_prepare || final_result.is_some() {
        let final_result = final_result.take();
        prepare(&mut socket, 3, BODY_QUERY, BODY, 2).await?;
        expect(
            &mut socket,
            &execute(
                3,
                &[0xfd, 8],
                &[&binary_string(XID.as_bytes()), &42i64.to_le_bytes()],
            ),
        )
        .await?;
        let root_only = declared.is_empty();
        let (root_eof, children_eof) = if root_only {
            (final_result, None)
        } else {
            (None, final_result)
        };
        send_body(&mut socket, 3, BODY, &[binary_string(ROOT_BYTES)], root_eof).await?;

        if reject_child_prepare {
            expect(
                &mut socket,
                b"\x16SELECT branch_id,rollback_info FROM undo_log WHERE branch_id IN (?) AND xid=? FOR UPDATE",
            ).await?;
            write_error(&mut socket, 1142, b"42000").await?;
        } else if let Some(final_result) = children_eof {
            assert_eq!(declared.len(), 2);
            prepare(&mut socket, 4, CHILD_BODY_QUERY, CHILD_BODY, 3).await?;
            expect(
                &mut socket,
                &execute(
                    4,
                    &[8, 8, 0xfd],
                    &[
                        &declared[0].to_le_bytes(),
                        &declared[1].to_le_bytes(),
                        &binary_string(XID.as_bytes()),
                    ],
                ),
            )
            .await?;
            let rows = members
                .ok_or_else(|| io::Error::other("missing fixture members"))?
                .iter()
                .map(|&id| [id.to_le_bytes().to_vec(), binary_string(child_bytes(id))].concat())
                .collect::<Vec<_>>();
            send_body(&mut socket, 4, CHILD_BODY, &rows, Some(final_result)).await?;
        }
    }
    expect(&mut socket, b"\x03ROLLBACK").await?;
    write_packet(&mut socket, 1, &[0, 0, 0, 2, 0, 0, 0]).await?;
    expect(&mut socket, b"\x01").await
}

async fn send_body(
    socket: &mut TcpStream,
    statement: u32,
    definitions: &[(&[u8], u8)],
    rows: &[Vec<u8>],
    final_result: Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>,
) -> io::Result<()> {
    let sequence = result_rows(socket, definitions, rows).await?;
    let close = [vec![0x19], statement.to_le_bytes().to_vec()].concat();
    expect(socket, &close).await?;
    if let Some((sent, wait)) = final_result {
        sent.send(())
            .map_err(|()| io::Error::other("reader finished before final row"))?;
        wait.await.map_err(io::Error::other)?;
    }
    write_packet(socket, sequence, &[0xfe, 0, 0, 3, 0]).await
}

async fn send_members(socket: &mut TcpStream, members: &[i64]) -> io::Result<()> {
    prepare(socket, 2, MEMBERS_QUERY, MEMBERS, 2).await?;
    expect(
        socket,
        &execute(
            2,
            &[0xfd, 0xfd],
            &[
                &binary_string(XID.as_bytes()),
                &binary_string(b"branchId=42"),
            ],
        ),
    )
    .await?;
    let rows = members
        .iter()
        .map(|id| id.to_le_bytes().to_vec())
        .collect::<Vec<_>>();
    result_set(socket, MEMBERS, &rows).await
}

async fn expect(socket: &mut TcpStream, command: &[u8]) -> io::Result<()> {
    assert_eq!(read_command(socket).await?.as_deref(), Some(command));
    Ok(())
}

fn binary_string(value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![value.len() as u8];
    bytes.extend_from_slice(value);
    bytes
}

fn execute(id: u32, parameter_types: &[u8], parameters: &[&[u8]]) -> Vec<u8> {
    let mut message = vec![0x17];
    message.extend_from_slice(&id.to_le_bytes());
    message.extend_from_slice(&[0, 1, 0, 0, 0, 0, 1]);
    for ty in parameter_types {
        message.extend_from_slice(&[*ty, 0]);
    }
    for parameter in parameters {
        message.extend_from_slice(parameter);
    }
    message
}

fn column(name: &[u8], ty: u8) -> Vec<u8> {
    let mut field = vec![3, b'd', b'e', b'f', 0, 0, 0];
    field.extend_from_slice(&binary_string(name));
    field.extend_from_slice(&binary_string(name));
    let (collation, flags) = if ty == 0xfc { (63, 128) } else { (45, 0) };
    field.extend_from_slice(&[12, collation, 0, 0, 0, 0, 0, ty, flags, 0, 0, 0, 0]);
    field
}

async fn columns(
    socket: &mut TcpStream,
    mut sequence: u8,
    definitions: &[(&[u8], u8)],
) -> io::Result<u8> {
    for &(name, ty) in definitions {
        write_packet(socket, sequence, &column(name, ty)).await?;
        sequence += 1;
    }
    write_packet(socket, sequence, &[0xfe, 0, 0, 3, 0]).await?;
    Ok(sequence + 1)
}

async fn prepare(
    socket: &mut TcpStream,
    id: u32,
    query: &[u8],
    definitions: &[(&[u8], u8)],
    parameters: u8,
) -> io::Result<()> {
    let mut command = vec![0x16];
    command.extend_from_slice(query);
    expect(socket, &command).await?;
    let mut response = vec![0];
    response.extend_from_slice(&id.to_le_bytes());
    response.extend_from_slice(&[definitions.len() as u8, 0, parameters, 0, 0, 0, 0]);
    write_packet(socket, 1, &response).await?;
    let sequence = columns(
        socket,
        2,
        &vec![(b"?".as_slice(), 0xfd); parameters as usize],
    )
    .await?;
    columns(socket, sequence, definitions).await?;
    Ok(())
}

async fn result_set(
    socket: &mut TcpStream,
    definitions: &[(&[u8], u8)],
    rows: &[Vec<u8>],
) -> io::Result<()> {
    let sequence = result_rows(socket, definitions, rows).await?;
    write_packet(socket, sequence, &[0xfe, 0, 0, 3, 0]).await
}

async fn result_rows(
    socket: &mut TcpStream,
    definitions: &[(&[u8], u8)],
    rows: &[Vec<u8>],
) -> io::Result<u8> {
    write_packet(socket, 1, &[definitions.len() as u8]).await?;
    let mut sequence = columns(socket, 2, definitions).await?;
    for row in rows {
        // BinaryRow 以 0x00 开头，本夹具最多五列，后接一个全零 NULL 位图。
        let mut bytes = vec![0, 0];
        bytes.extend_from_slice(row);
        write_packet(socket, sequence, &bytes).await?;
        sequence += 1;
    }
    Ok(sequence)
}
