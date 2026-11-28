//! 回环测试用的最小 MySQL 帧与固定握手。
use std::io;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

async fn receive_handshake_response(socket: &mut TcpStream, id: u32) -> io::Result<()> {
    const CAPABILITIES: u32 = 0x0008_a209; // protocol 4.1、认证插件、事务和数据库。
    let mut hello = b"\x0a8.0.36\0".to_vec();
    hello.extend_from_slice(&id.to_le_bytes());
    hello.extend_from_slice(b"fixture0\0");
    hello.extend_from_slice(&(CAPABILITIES as u16).to_le_bytes());
    hello.extend_from_slice(&[45, 2, 0]);
    hello.extend_from_slice(&((CAPABILITIES >> 16) as u16).to_le_bytes());
    hello.push(21);
    hello.extend_from_slice(&[0; 10]);
    hello.extend_from_slice(b"fixture12345\0mysql_native_password\0");
    write_packet(socket, 0, &hello).await?;
    assert!(read_packet(socket)
        .await?
        .is_some_and(|(sequence, _)| sequence == 1));
    Ok(())
}

pub(crate) async fn authenticate(socket: &mut TcpStream, id: u32) -> io::Result<()> {
    receive_handshake_response(socket, id).await?;
    write_packet(socket, 2, &[0, 0, 0, 2, 0, 0, 0]).await?;

    let command = read_command(socket)
        .await?
        .ok_or_else(|| io::Error::other("missing SQLx setup"))?;
    assert!(command.starts_with(b"\x03SET "));
    write_packet(socket, 1, &[0, 0, 0, 2, 0, 0, 0]).await
}

pub(crate) async fn write_error(
    socket: &mut TcpStream,
    number: u16,
    state: &[u8; 5],
) -> io::Result<()> {
    write_error_at(socket, 1, number, state).await
}

pub(crate) async fn reject_authentication(socket: &mut TcpStream, id: u32) -> io::Result<()> {
    receive_handshake_response(socket, id).await?;
    write_error_at(socket, 2, 1045, b"28000").await
}

async fn write_error_at(
    socket: &mut TcpStream,
    sequence: u8,
    number: u16,
    state: &[u8; 5],
) -> io::Result<()> {
    let mut error = vec![0xff];
    error.extend_from_slice(&number.to_le_bytes());
    error.push(b'#');
    error.extend_from_slice(state);
    error.extend_from_slice(b"fixed fixture error");
    write_packet(socket, sequence, &error).await
}

pub(crate) async fn read_command(socket: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    Ok(read_packet(socket).await?.map(|(sequence, payload)| {
        assert_eq!(sequence, 0);
        payload
    }))
}

async fn read_packet(socket: &mut TcpStream) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut header = [0; 4];
    if let Err(error) = socket.read_exact(&mut header[..1]).await {
        return match error.kind() {
            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset => Ok(None),
            _ => Err(error),
        };
    }
    socket.read_exact(&mut header[1..]).await?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
    if length > 4096 {
        return Err(io::Error::other("fixture packet exceeds 4 KiB"));
    }
    let mut payload = vec![0; length];
    socket.read_exact(&mut payload).await?;
    Ok(Some((header[3], payload)))
}

pub(crate) async fn write_packet(
    socket: &mut TcpStream,
    sequence: u8,
    payload: &[u8],
) -> io::Result<()> {
    let mut header = (payload.len() as u32).to_le_bytes();
    header[3] = sequence;
    socket.write_all(&header).await?;
    socket.write_all(payload).await
}
