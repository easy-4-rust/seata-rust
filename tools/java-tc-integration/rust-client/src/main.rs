mod auth_probe;
mod backend_timeout_probe;
mod branch_callback_state;
mod branch_end_request;
mod branch_probe;
mod file_timeout_probe;
mod pending_branch;
mod raft_timeout_probe;
mod response_loss_probe;
mod restart_probe;
mod restart_record;
mod transaction;
mod unknown_outcome;

#[cfg(test)]
mod payload;

use std::env;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

const MAGIC: [u8; 2] = [0xda, 0xda];
const PROTOCOL_V1: u8 = 1;
const REQUEST_SYNC: u8 = 0;
const RESPONSE: u8 = 1;
const SERIALIZER_SEATA: u8 = 1;
const COMPRESSOR_NONE: u8 = 0;
const HEADER_LEN: usize = 16;
const MAX_FRAME_LEN: usize = 8 * 1024 * 1024;
const TYPE_REGISTER_TM: u16 = 101;
const TYPE_REGISTER_TM_RESULT: u16 = 102;
const TYPE_REGISTER_RM: u16 = 103;
const TYPE_REGISTER_RM_RESULT: u16 = 104;
const CLIENT_VERSION: &str = "1.1.0";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn write_string16(value: &str, out: &mut Vec<u8>) -> io::Result<()> {
    let bytes = value.as_bytes();
    let len = u16::try_from(bytes.len()).map_err(|_| invalid("string16 exceeds 65535 bytes"))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn write_string32(value: &str, out: &mut Vec<u8>) -> io::Result<()> {
    let bytes = value.as_bytes();
    let len = u32::try_from(bytes.len()).map_err(|_| invalid("string32 exceeds u32"))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn build_v1_request(message_type: u16, request_id: u32, body: &[u8]) -> io::Result<Vec<u8>> {
    let mut payload = Vec::with_capacity(2 + body.len());
    payload.extend_from_slice(&message_type.to_be_bytes());
    payload.extend_from_slice(body);

    let full_len = HEADER_LEN + payload.len();
    if full_len > MAX_FRAME_LEN {
        return Err(invalid("registration frame exceeds Seata max frame length"));
    }

    let full_len = u32::try_from(full_len).map_err(|_| invalid("frame length exceeds u32"))?;
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(PROTOCOL_V1);
    frame.extend_from_slice(&full_len.to_be_bytes());
    frame.extend_from_slice(&(HEADER_LEN as u16).to_be_bytes());
    frame.push(REQUEST_SYNC);
    frame.push(SERIALIZER_SEATA);
    frame.push(COMPRESSOR_NONE);
    frame.extend_from_slice(&request_id.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn read_u16(input: &[u8], offset: &mut usize) -> io::Result<u16> {
    if input.len().saturating_sub(*offset) < 2 {
        return Err(invalid("truncated u16"));
    }
    let value = u16::from_be_bytes([input[*offset], input[*offset + 1]]);
    *offset += 2;
    Ok(value)
}

fn read_string16(input: &[u8], offset: &mut usize) -> io::Result<String> {
    let len = read_u16(input, offset)? as usize;
    if input.len().saturating_sub(*offset) < len {
        return Err(invalid("truncated string16"));
    }
    let value = std::str::from_utf8(&input[*offset..*offset + len])
        .map_err(|e| invalid(format!("invalid UTF-8 in string16: {e}")))?
        .to_owned();
    *offset += len;
    Ok(value)
}

#[derive(Debug)]
struct IdentifyResponse {
    identified: bool,
    version: String,
}

fn read_identify_response(
    stream: &mut TcpStream,
    expected_request_id: u32,
    expected_message_type: u16,
) -> io::Result<IdentifyResponse> {
    let mut header = [0u8; HEADER_LEN];
    stream.read_exact(&mut header)?;

    if header[0..2] != MAGIC {
        return Err(invalid(format!(
            "bad response magic: {:02x?}",
            &header[0..2]
        )));
    }
    if header[2] != PROTOCOL_V1 {
        return Err(invalid(format!(
            "TC selected protocol version {}, expected V1",
            header[2]
        )));
    }

    let full_len = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    let head_len = u16::from_be_bytes([header[7], header[8]]) as usize;
    let message_type = header[9];
    let serializer = header[10];
    let compressor = header[11];
    let request_id = u32::from_be_bytes(header[12..16].try_into().unwrap());

    if !(HEADER_LEN..=MAX_FRAME_LEN).contains(&full_len) {
        return Err(invalid(format!("invalid response fullLength: {full_len}")));
    }
    if head_len < HEADER_LEN || head_len > full_len {
        return Err(invalid(format!("invalid response headLength: {head_len}")));
    }
    if message_type != RESPONSE {
        return Err(invalid(format!(
            "unexpected RPC message type: {message_type}"
        )));
    }
    if serializer != SERIALIZER_SEATA || compressor != COMPRESSOR_NONE {
        return Err(invalid(format!(
            "unexpected serializer/compressor: {serializer}/{compressor}"
        )));
    }
    if request_id != expected_request_id {
        return Err(invalid(format!(
            "request ID mismatch: received {request_id}, expected {expected_request_id}"
        )));
    }

    let mut remainder = vec![0u8; full_len - HEADER_LEN];
    stream.read_exact(&mut remainder)?;

    // Registration response fixtures currently have no HeadMap. Keep parsing
    // strict here so an unexpected server header extension is not ignored.
    if head_len != HEADER_LEN {
        return Err(invalid(format!(
            "registration response unexpectedly contains {} HeadMap bytes",
            head_len - HEADER_LEN
        )));
    }

    let mut offset = 0usize;
    let actual_message_type = read_u16(&remainder, &mut offset)?;
    if actual_message_type != expected_message_type {
        return Err(invalid(format!(
            "registration response message type {actual_message_type}, expected {expected_message_type}"
        )));
    }
    let identified = match remainder.get(offset).copied() {
        Some(0) => false,
        Some(1) => true,
        Some(value) => return Err(invalid(format!("invalid identified flag: {value}"))),
        None => return Err(invalid("missing identified flag")),
    };
    offset += 1;
    let version = read_string16(&remainder, &mut offset)?;
    if offset != remainder.len() {
        return Err(invalid(format!(
            "unexpected {} trailing bytes in IdentifyResponse",
            remainder.len() - offset
        )));
    }

    Ok(IdentifyResponse {
        identified,
        version,
    })
}

fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    let address = format!("{host}:{port}");
    let socket = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| invalid(format!("could not resolve {address}")))?;
    let stream = TcpStream::connect_timeout(&socket, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

fn register_tm(host: &str, port: u16, app_id: &str, tx_group: &str) -> io::Result<TcpStream> {
    let mut body = Vec::new();
    write_string16(CLIENT_VERSION, &mut body)?;
    write_string16(app_id, &mut body)?;
    write_string16(tx_group, &mut body)?;
    write_string16("", &mut body)?;

    let request_id = 1;
    let frame = build_v1_request(TYPE_REGISTER_TM, request_id, &body)?;
    let mut stream = connect(host, port)?;
    stream.write_all(&frame)?;
    let response = read_identify_response(&mut stream, request_id, TYPE_REGISTER_TM_RESULT)?;
    if !response.identified {
        return Err(invalid(format!(
            "Java TC rejected RegisterTM (version={})",
            response.version
        )));
    }
    println!(
        "PASS RegisterTM identified=true protocol=V1 serverVersion={}",
        response.version
    );
    Ok(stream)
}

fn register_rm(
    host: &str,
    port: u16,
    app_id: &str,
    tx_group: &str,
    resource_id: &str,
) -> io::Result<TcpStream> {
    let mut body = Vec::new();
    write_string16(CLIENT_VERSION, &mut body)?;
    write_string16(app_id, &mut body)?;
    write_string16(tx_group, &mut body)?;
    write_string16("", &mut body)?;
    write_string32(resource_id, &mut body)?;

    let request_id = 2;
    let frame = build_v1_request(TYPE_REGISTER_RM, request_id, &body)?;
    let mut stream = connect(host, port)?;
    stream.write_all(&frame)?;
    let response = read_identify_response(&mut stream, request_id, TYPE_REGISTER_RM_RESULT)?;
    if !response.identified {
        return Err(invalid(format!(
            "Java TC rejected RegisterRM (version={})",
            response.version
        )));
    }
    println!(
        "PASS RegisterRM identified=true resourceId={} protocol=V1 serverVersion={}",
        resource_id, response.version
    );
    Ok(stream)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [] => "registration",
        [flag]
            if matches!(
                flag.as_str(),
                "--lifecycle" | "--branches" | "--callback-loss" | "--prepare-restart-begin" | "--prepare-restart-branch" | "--resume-restart" | "--lost-branch-register-response" | "--lost-commit-response" | "--lost-rollback-response" | "--query-file-timeout" | "--backend-retry-timeout" | "--auth-default" | "--auth-denied" | "--auth-accepted" | "--auth-bypass" | "--query-raft-timeout"
            ) =>
        {
            flag.as_str()
        }
        _ => {
            return Err(
                invalid("usage: seata-java-tc-it [--lifecycle|--branches|--callback-loss|--prepare-restart-begin|--prepare-restart-branch|--resume-restart|--lost-branch-register-response|--lost-commit-response|--lost-rollback-response|--query-file-timeout|--backend-retry-timeout|--auth-default|--auth-denied|--auth-accepted|--auth-bypass]").into(),
            )
        }
    };
    let host = env::var("SEATA_TC_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("SEATA_TC_PORT")
        .unwrap_or_else(|_| "18091".to_owned())
        .parse::<u16>()?;
    if mode.starts_with("--auth-") {
        return auth_probe::run(&host, port, mode).map_err(Into::into);
    }
    let app_id = env::var("SEATA_IT_APP_ID").unwrap_or_else(|_| "orders-rust".to_owned());
    let tx_group = env::var("SEATA_IT_TX_GROUP").unwrap_or_else(|_| "default_tx_group".to_owned());
    let resource_id =
        env::var("SEATA_IT_RESOURCE_ID").unwrap_or_else(|_| "mysql://orders".to_owned());
    let hold_ms = env::var("SEATA_IT_HOLD_MS")
        .unwrap_or_else(|_| "1000".to_owned())
        .parse::<u64>()?;

    let mut tm_channel = register_tm(&host, port, &app_id, &tx_group)?;
    if mode == "--query-raft-timeout" {
        let xids = env::var("SEATA_IT_RAFT_XIDS")?;
        let statuses = env::var("SEATA_IT_RAFT_STATUSES")?
            .split(',')
            .map(str::parse::<u8>)
            .collect::<Result<Vec<_>, _>>()?;
        return raft_timeout_probe::verify(
            &mut tm_channel,
            &xids.split(',').collect::<Vec<_>>(),
            &statuses,
        )
        .map_err(Into::into);
    }
    let mut rm_channel = register_rm(&host, port, &app_id, &tx_group, &resource_id)?;

    println!("PASS live Java TC registration smoke: TM + RM");
    if mode == "--backend-retry-timeout" {
        backend_timeout_probe::run(
            &mut tm_channel,
            &mut rm_channel,
            &env::var("SEATA_IT_RETRY_XID")?,
            &resource_id,
            env::var("SEATA_IT_RETRY_STATUS")?.parse()?,
            env::var("SEATA_IT_RETRY_BRANCH_ID")?.parse()?,
            std::path::Path::new(&env::var("SEATA_IT_RETRY_READY")?),
        )?;
    }
    if mode == "--query-file-timeout" {
        file_timeout_probe::verify(
            &mut tm_channel,
            &mut rm_channel,
            &env::var("SEATA_IT_TIMEOUT_XID")?,
            &resource_id,
            env::var("SEATA_IT_TIMEOUT_EXPECT_STATUS")?.parse()?,
        )?;
    }
    if mode.starts_with("--lost-") {
        let operation = match mode {
            "--lost-branch-register-response" => 11,
            "--lost-commit-response" => 7,
            _ => 9,
        };
        response_loss_probe::verify(&mut tm_channel, &mut rm_channel, &resource_id, operation)?;
    }
    if mode == "--lifecycle" {
        transaction::verify_empty_transactions(&mut tm_channel)?;
        println!("PASS no-branch global lifecycle smoke; AT and RM callbacks NOT tested");
    }
    if mode == "--branches" {
        branch_probe::verify_branch_transactions(&mut tm_channel, &mut rm_channel, &resource_id)?;
        println!(
            "PASS simulated branch lifecycle; database/Undo Log, retry and restart NOT tested"
        );
    }
    if mode == "--callback-loss" {
        branch_probe::verify_callback_loss(&mut tm_channel, &mut rm_channel, &resource_id)?;
        println!("PASS callback response-loss probe; process-local simulated actions only; durable/database idempotency NOT tested");
    }
    if mode.starts_with("--prepare-restart-") || mode == "--resume-restart" {
        let path = std::path::PathBuf::from(env::var("SEATA_IT_RESTART_RECORD")?);
        if mode == "--resume-restart" {
            restart_probe::resume(&mut tm_channel, &mut rm_channel, &resource_id, &path)?;
        } else {
            restart_probe::prepare(
                &mut tm_channel,
                &mut rm_channel,
                &resource_id,
                mode == "--prepare-restart-branch",
                &path,
            )?;
        }
    }
    // Retain the existing registration hold period after any selected probe.
    // The branch probe services one expected callback per transaction, not a long-lived RM.
    thread::sleep(Duration::from_millis(hold_ms));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_registration_roles_use_the_same_numeric_client_version() {
        for rm in [false, true] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut header = [0; HEADER_LEN];
                socket.read_exact(&mut header).unwrap();
                let length = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
                let id = u32::from_be_bytes(header[12..16].try_into().unwrap());
                let mut body = vec![0; length - HEADER_LEN];
                socket.read_exact(&mut body).unwrap();
                let mut at = 2;
                let version = read_string16(&body, &mut at).unwrap();
                let mut response = vec![1];
                write_string16("2.8.0", &mut response).unwrap();
                let mut frame =
                    build_v1_request(if rm { 104 } else { 102 }, id, &response).unwrap();
                frame[9] = RESPONSE;
                socket.write_all(&frame).unwrap();
                version
            });
            let channel = if rm {
                register_rm("127.0.0.1", port, "it-app", "it-group", "mysql://it")
            } else {
                register_tm("127.0.0.1", port, "it-app", "it-group")
            };
            channel.unwrap();
            assert_eq!(server.join().unwrap(), CLIENT_VERSION, "RM={rm}");
        }
    }

    #[test]
    fn tm_frame_has_v1_header_and_type_101() {
        let mut body = Vec::new();
        write_string16(CLIENT_VERSION, &mut body).unwrap();
        write_string16("orders-rust", &mut body).unwrap();
        write_string16("default_tx_group", &mut body).unwrap();
        write_string16("", &mut body).unwrap();
        let frame = build_v1_request(TYPE_REGISTER_TM, 1, &body).unwrap();
        assert_eq!(&frame[0..3], &[0xda, 0xda, 1]);
        assert_eq!(
            u32::from_be_bytes(frame[3..7].try_into().unwrap()) as usize,
            frame.len()
        );
        assert_eq!(u16::from_be_bytes([frame[7], frame[8]]), 16);
        assert_eq!(frame[9], REQUEST_SYNC);
        assert_eq!(u16::from_be_bytes([frame[16], frame[17]]), TYPE_REGISTER_TM);
    }

    #[test]
    fn rm_frame_uses_string32_for_resource_ids() {
        let mut body = Vec::new();
        write_string16("V1,0", &mut body).unwrap();
        write_string16("orders-rust", &mut body).unwrap();
        write_string16("default_tx_group", &mut body).unwrap();
        write_string16("", &mut body).unwrap();
        write_string32("mysql://orders", &mut body).unwrap();
        let frame = build_v1_request(TYPE_REGISTER_RM, 2, &body).unwrap();
        assert_eq!(u16::from_be_bytes([frame[16], frame[17]]), TYPE_REGISTER_RM);
        assert_eq!(
            &frame[frame.len() - 18..frame.len() - 14],
            &14u32.to_be_bytes()
        );
    }
}
