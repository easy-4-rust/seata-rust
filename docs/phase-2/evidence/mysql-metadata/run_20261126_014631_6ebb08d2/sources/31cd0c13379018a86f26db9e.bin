use crate::codec::{WireReader, WireWriter};
use crate::{ProtocolError, ResultCode, TransactionExceptionCode};
use std::fmt;
/// 事务响应公共前缀；失败消息及异常码不能由后续状态覆盖。
/// 对应 Java: org.apache.seata.core.protocol.transaction.AbstractTransactionResponse
#[derive(Clone, PartialEq, Eq)]
pub struct AbstractTransactionResponse {
    /// 调用结果；Failed为明确失败，不能解释为成功。
    pub result_code: ResultCode,
    /// 失败消息，Java零长度解码为None；成功响应不能携带该字段。
    pub msg: Option<String>,
    /// Java异常域语义，使用显式码转换。
    pub transaction_exception_code: TransactionExceptionCode,
}
impl fmt::Debug for AbstractTransactionResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AbstractTransactionResponse")
            .field("result_code", &self.result_code)
            .field("msg_present", &self.msg.is_some())
            .field(
                "transaction_exception_code",
                &self.transaction_exception_code,
            )
            .finish_non_exhaustive()
    }
}
impl AbstractTransactionResponse {
    /// 从参数游标读取响应前缀；所有失败消息采用signed-short长度，非法数据返回错误。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        let result_code = ResultCode::from_java_v1(reader.u8()?)?;
        let msg = if result_code == ResultCode::Failed {
            reader.string16("msg")?
        } else {
            None
        };
        let transaction_exception_code = TransactionExceptionCode::from_java_v1(reader.u8()?)?;
        Ok(Self {
            result_code,
            msg,
            transaction_exception_code,
        })
    }
    /// 写入完整前缀；成功携带msg无法无损表达，返回错误而不静默丢弃。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        if self.result_code == ResultCode::Success && self.msg.is_some() {
            return Err(ProtocolError::InvalidValue {
                field: "msg",
                reason: "success response cannot carry error text",
            });
        }
        writer.u8(self.result_code.to_java_v1())?;
        if self.result_code == ResultCode::Failed {
            writer.string16(self.msg.as_deref(), "msg")?;
        }
        writer.u8(self.transaction_exception_code.to_java_v1())
    }
}
