/// XA 原两分量及 Seata 逻辑身份的只读契约，不授予登记、恢复或全局结束权。
/// 对应 Java: org.apache.seata.rm.datasource.xa.XAXid。
pub trait XaXid {
    /// 无参数；返回原逻辑 XID，null 保留为 None。
    fn get_global_xid(&self) -> Option<&str>;
    /// 无参数；返回原有符号 long 分支号，零保留为零。
    fn get_branch_id(&self) -> i64;
    /// 无参数；返回原 XA 格式编号。
    fn get_format_id(&self) -> i32;
    /// 无参数；返回原 XID 的 UTF-8 字节，null/空串均为空字节。
    fn get_global_transaction_id(&self) -> &[u8];
    /// 无参数；返回原分支分量字节，零为空。
    fn get_branch_qualifier(&self) -> &[u8];
}
