use std::io::{self, Read};

/// RM 收到的二阶段请求，仅接受本探针验证的 V1/SEATA/无压缩信封。
/// 对应 Java: org.apache.seata.core.protocol.transaction.AbstractBranchEndRequest，附带 RpcMessage 的关联字段。
pub(super) struct BranchEndRequest {
    pub(super) request_id: u32,
    pub(super) kind: u16,
    pub(super) xid: String,
    pub(super) branch_id: i64,
    pub(super) branch_type: u8,
    pub(super) resource: String,
    pub(super) application_data: String,
}

impl BranchEndRequest {
    /// 从读取器解码完整请求；返回请求身份信息，遇到截断或不支持的信封时返回错误。
    /// 对应 Java: ProtocolDecoderV1#decodeFrame / AbstractBranchEndRequestCodec#decode。
    pub(super) fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut header = [0; crate::HEADER_LEN];
        reader.read_exact(&mut header)?;
        let full = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
        if header[..3] != [0xda, 0xda, 1]
            || !(crate::HEADER_LEN + 2..=crate::MAX_FRAME_LEN).contains(&full)
            || header[7..12]
                != [
                    0,
                    16,
                    crate::REQUEST_SYNC,
                    crate::SERIALIZER_SEATA,
                    crate::COMPRESSOR_NONE,
                ]
        {
            return Err(crate::invalid(format!(
                "unsupported callback envelope: {header:02x?}"
            )));
        }
        let mut body = vec![0; full - crate::HEADER_LEN];
        reader.read_exact(&mut body)?;
        let mut at = 0;
        let kind = crate::read_u16(&body, &mut at)?;
        if !matches!(kind, 3 | 5) {
            return Err(crate::invalid(format!("unexpected RM request type {kind}")));
        }
        let xid = crate::read_string16(&body, &mut at)?;
        let branch_id = i64::from_be_bytes(take(&body, &mut at, 8)?.try_into().unwrap());
        let branch_type = take(&body, &mut at, 1)?[0];
        let resource = crate::read_string16(&body, &mut at)?;
        let data_len = i32::from_be_bytes(take(&body, &mut at, 4)?.try_into().unwrap());
        let data_len = usize::try_from(data_len)
            .map_err(|_| crate::invalid("negative applicationData length"))?;
        let application_data = std::str::from_utf8(take(&body, &mut at, data_len)?)
            .map_err(|_| crate::invalid("non-UTF8 applicationData"))?
            .to_owned();
        if at != body.len() || xid.is_empty() || resource.is_empty() || branch_id <= 0 {
            return Err(crate::invalid(
                "invalid callback identity or trailing bytes",
            ));
        }
        Ok(Self {
            request_id: u32::from_be_bytes(header[12..16].try_into().unwrap()),
            kind,
            xid,
            branch_id,
            branch_type,
            resource,
            application_data,
        })
    }
}

fn take<'a>(body: &'a [u8], at: &mut usize, len: usize) -> io::Result<&'a [u8]> {
    if body.len().saturating_sub(*at) < len {
        return Err(crate::invalid("truncated branch callback"));
    }
    let bytes = &body[*at..*at + len];
    *at += len;
    Ok(bytes)
}
