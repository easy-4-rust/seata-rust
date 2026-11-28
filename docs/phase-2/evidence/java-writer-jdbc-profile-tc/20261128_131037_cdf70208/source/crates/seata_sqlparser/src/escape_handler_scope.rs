/// 转义处理器在加载器中的实例范围。
/// 对应 Java: org.apache.seata.common.loader.Scope。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EscapeHandlerScope {
    /// 同一服务定义共用一个处理器。
    Singleton,
    /// 每个工厂缓存未命中的原始名称构造一个处理器。
    Prototype,
}
