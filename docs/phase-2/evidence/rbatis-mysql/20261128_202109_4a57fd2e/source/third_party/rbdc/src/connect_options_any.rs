use std::any::Any;
/// 为 Rust 1.85 保留对象安全的原连接选项类型查询，不改变驱动实现。
/// 对应 Java: 无直接对象；rbdc ConnectOptions 的 Rust 兼容扩展。
pub trait ConnectOptionsAny: Any {
    /// 无参数；返回原具体连接选项的共享 Any 引用，不复制对象。
    fn options_as_any(&self) -> &dyn Any;
    /// 无参数；返回原具体连接选项的可变 Any 引用，不替换对象。
    fn options_as_any_mut(&mut self) -> &mut dyn Any;
}
impl<T: Any> ConnectOptionsAny for T {
    fn options_as_any(&self) -> &dyn Any {
        self
    }
    fn options_as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
