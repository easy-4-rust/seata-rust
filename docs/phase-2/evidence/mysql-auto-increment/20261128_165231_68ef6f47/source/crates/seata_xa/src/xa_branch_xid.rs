use crate::{XaError, XaXid};
/// Seata XID 与原 BranchId 的 XA 字节身份；此值不证明 TC 曾登记分支。
/// 对应 Java: org.apache.seata.rm.datasource.xa.XABranchXid。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct XaBranchXid {
    xid: Option<String>,
    branch_id: i64,
    global_transaction_id: Vec<u8>,
    branch_qualifier: Vec<u8>,
}
impl XaBranchXid {
    /// Seata 原 XA 格式号，对应 Java SEATA_XA_XID_FORMAT_ID。
    pub const FORMAT_ID: i32 = 9752;
    /// 参数为原可空 XID 和有符号 long；返回保留 Java 编码语义的值对象。
    pub fn new(xid: Option<String>, branch_id: i64) -> Self {
        let global_transaction_id = xid.as_deref().unwrap_or("").as_bytes().to_vec();
        let branch_qualifier = if branch_id == 0 {
            Vec::new()
        } else {
            format!("-{branch_id}").into_bytes()
        };
        Self {
            xid,
            branch_id,
            global_transaction_id,
            branch_qualifier,
        }
    }
    /// 参数为恢复格式和两段原字节；返回规范有效 UTF-8 标识，拒绝有损解码和非规范分支文本。
    pub fn from_components(format_id: i32, global: &[u8], branch: &[u8]) -> Result<Self, XaError> {
        if format_id != Self::FORMAT_ID {
            return Err(XaError::Format);
        }
        if global.len() > 64 || branch.len() > 64 {
            return Err(XaError::Components);
        }
        let xid = if global.is_empty() {
            None
        } else {
            Some(
                std::str::from_utf8(global)
                    .map_err(|_| XaError::Components)?
                    .to_owned(),
            )
        };
        let branch_id = if branch.is_empty() {
            0
        } else {
            std::str::from_utf8(branch)
                .map_err(|_| XaError::Components)?
                .strip_prefix('-')
                .ok_or(XaError::Components)?
                .parse::<i64>()
                .map_err(|_| XaError::Components)?
        };
        let value = Self::new(xid, branch_id);
        if value.branch_qualifier != branch {
            return Err(XaError::Components);
        }
        Ok(value)
    }
    /// 无参数；返回严格 MySQL XA 三分量 SQL 字面量，只有十六进制及固定格式号进入命令。
    pub fn mysql_arguments(&self) -> Result<String, XaError> {
        if self.global_transaction_id.is_empty()
            || self.global_transaction_id.len() > 64
            || self.branch_id <= 0
            || self.branch_qualifier.len() > 64
        {
            return Err(XaError::MySqlBranch);
        }
        fn hex(bytes: &[u8]) -> String {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            bytes
                .iter()
                .flat_map(|b| {
                    [
                        HEX[(b >> 4) as usize] as char,
                        HEX[(b & 15) as usize] as char,
                    ]
                })
                .collect()
        }
        Ok(format!(
            "X'{}',X'{}',{}",
            hex(&self.global_transaction_id),
            hex(&self.branch_qualifier),
            Self::FORMAT_ID
        ))
    }
}
impl XaXid for XaBranchXid {
    fn get_global_xid(&self) -> Option<&str> {
        self.xid.as_deref()
    }
    fn get_branch_id(&self) -> i64 {
        self.branch_id
    }
    fn get_format_id(&self) -> i32 {
        Self::FORMAT_ID
    }
    fn get_global_transaction_id(&self) -> &[u8] {
        &self.global_transaction_id
    }
    fn get_branch_qualifier(&self) -> &[u8] {
        &self.branch_qualifier
    }
}
impl std::fmt::Debug for XaBranchXid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XaBranchXid")
            .field("global_bytes", &self.global_transaction_id.len())
            .field("branch_qualifier_bytes", &self.branch_qualifier.len())
            .field("format_id", &Self::FORMAT_ID)
            .finish_non_exhaustive()
    }
}
