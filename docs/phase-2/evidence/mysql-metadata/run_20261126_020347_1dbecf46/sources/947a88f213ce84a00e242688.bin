use thiserror::Error;

/// 解析器名称与 SPI 加载错误，类别与固定 Java 路径的异常对应。
/// 对应 Java: UndoLogParserFactory 和 EnhancedServiceLoader 的选择错误。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum UndoLogParserSelectionError {
    /// 工厂接收了空引用，对应 NullPointerException。
    #[error("Undo parser factory name is null")]
    NullFactoryName,
    /// SPI 接收了空名称，对应 IllegalArgumentException。
    #[error("Undo parser provider name is empty")]
    EmptyName,
    /// FST 已移除，对应工厂的 IllegalArgumentException。
    #[error("FST Undo parser was removed in 2.0")]
    RemovedFst,
    /// 名称缺少已登记实现，对应 EnhancedServiceNotFoundException。
    #[error("no Undo parser provider for the requested name")]
    ProviderNotFound,
}
