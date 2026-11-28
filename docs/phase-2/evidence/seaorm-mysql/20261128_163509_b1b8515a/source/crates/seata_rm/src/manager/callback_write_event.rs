use seata_transport::CallError;

/// 原回调信封的本地写入事件作用域，取消时不推断是否已发送。
/// 对应 Java: ChannelFuture 回包写入观察职责；不代表 TC 已接收。
pub(crate) struct CallbackWriteEvent {
    request_id: i32,
    connection_generation: u64,
    response_count: usize,
    settled: bool,
}

impl CallbackWriteEvent {
    /// 参数为原信封编号、原连接代际及响应条数；返回单次结算作用域。
    pub(crate) fn new(request_id: i32, connection_generation: u64, response_count: usize) -> Self {
        Self {
            request_id,
            connection_generation,
            response_count,
            settled: false,
        }
    }

    /// 参数为传输层原结果引用；无返回值，既不改写错误也不重试回包。
    pub(crate) fn finish(mut self, result: &Result<(), CallError>) {
        let outcome = match result {
            Ok(()) => "local_written",
            Err(error) if error.is_unknown_outcome() => "unknown",
            Err(_) => "not_sent",
        };
        self.emit(outcome);
        self.settled = true;
    }

    fn emit(&self, outcome: &'static str) {
        tracing::info!(
            target: "seata_rm::callback_write",
            request_id = self.request_id,
            connection_generation = self.connection_generation,
            response_count = self.response_count,
            outcome,
            "rm_callback_response_write"
        );
    }
}

impl Drop for CallbackWriteEvent {
    fn drop(&mut self) {
        if !self.settled {
            self.emit("cancelled");
        }
    }
}

#[cfg(test)]
#[path = "callback_write_event_tests.rs"]
mod tests;
