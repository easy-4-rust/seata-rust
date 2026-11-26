/// 单写入者的有界任务，普通请求必须通过等待表写入标记，控制回复不占用普通ID完成槽。
/// 对应 Java: Channel#writeAndFlush 的排队写入职责；Rust区分请求与心跳控制。
pub(crate) enum OutboundCommand {
    Request { id: i32, bytes: Vec<u8> },
    Control { bytes: Vec<u8> },
    Probe { bytes: Vec<u8> },
    Response(super::response_write::ResponseWrite),
}
