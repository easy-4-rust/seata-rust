use std::any::Any;
/// 为 Rust 1.85 保留对象安全的原拦截器类型查询，不改变用户拦截器实现。
/// 对应 Java: 无直接对象；rbatis Intercept 的 Rust 兼容扩展。
pub trait InterceptAny: Any {
    /// 无参数；返回原具体拦截器的共享 Any 引用，不复制或重建对象。
    fn intercept_as_any(&self) -> &dyn Any;
}
impl<T: Any> InterceptAny for T {
    fn intercept_as_any(&self) -> &dyn Any {
        self
    }
}
