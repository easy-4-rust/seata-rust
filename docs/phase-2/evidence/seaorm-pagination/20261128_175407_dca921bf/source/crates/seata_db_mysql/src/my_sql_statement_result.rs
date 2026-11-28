/// 原业务语句的 MySQL OK 回执；仅在镜像与 Undo 准备成功后由事务发布。
/// 对应 Java: 无直接对象；StatementCallback 原执行结果的 Rust 类型化扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MySqlStatementResult {
    rows_affected: u64,
    last_insert_id: u64,
}
impl MySqlStatementResult {
    /// 参数为同一原语句的服务端计数与 ID；返回不可变原始回执，不推算生成主键。
    pub(crate) fn new(rows_affected: u64, last_insert_id: u64) -> Self {
        Self {
            rows_affected,
            last_insert_id,
        }
    }
    /// 无参数；返回服务端原始影响行数，不表示本地或全局事务已提交。
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }
    /// 无参数；返回服务端原始 last_insert_id，可为零；不推算多行插入的其他 ID。
    pub fn last_insert_id(&self) -> u64 {
        self.last_insert_id
    }
}
