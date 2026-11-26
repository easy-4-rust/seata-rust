use crate::{MetricsError, MetricsServerConfig, MetricsSources};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};
use tokio::task::{JoinHandle, JoinSet};

const MAX_HEADER_BYTES: usize = 4096;
const MAX_CLIENTS: usize = 16;
const HEADER_TIMEOUT: Duration = Duration::from_secs(2);

/// 拥有本机认证指标监听器和所有采集连接任务；关闭时不影响 TM/RM 池。
/// 对应 Java: 客户端监控端点的生命周期职责。
pub struct MetricsExporter {
    address: SocketAddr,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<Result<(), MetricsError>>>,
}

impl MetricsExporter {
    /// 参数为安全监听配置与只读来源；绑定回环地址并启动有界 HTTP 拉取服务。
    pub async fn start(
        config: MetricsServerConfig,
        sources: MetricsSources,
    ) -> Result<Self, MetricsError> {
        if sources.is_empty() {
            return Err(MetricsError::InvalidConfiguration {
                reason: "at least one metrics source is required",
            });
        }
        let listener = TcpListener::bind(config.address)
            .await
            .map_err(|error| MetricsError::Bind { kind: error.kind() })?;
        let address = listener
            .local_addr()
            .map_err(|error| MetricsError::Bind { kind: error.kind() })?;
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(serve(listener, config.token, sources, receiver));
        Ok(Self {
            address,
            stop,
            task: Some(task),
        })
    }

    /// 返回实际回环监听地址；端口 0 配置时含系统分配端口。
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// 消费并关闭监听器，取消所有在途采集，不关闭被采样的业务连接池。
    pub async fn shutdown(mut self) -> Result<(), MetricsError> {
        self.stop.send_replace(true);
        match self.task.take() {
            Some(task) => task.await.map_err(|_| MetricsError::WorkerFailed)?,
            None => Ok(()),
        }
    }
}

impl Drop for MetricsExporter {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn serve(
    listener: TcpListener,
    token: String,
    sources: MetricsSources,
    mut stop: watch::Receiver<bool>,
) -> Result<(), MetricsError> {
    let token = Arc::new(token);
    let sources = Arc::new(sources);
    let permits = Arc::new(Semaphore::new(MAX_CLIENTS));
    let mut clients = JoinSet::new();
    let mut failure = None;
    loop {
        tokio::select! {biased;
            _ = stop.changed() => break,
            result = listener.accept() => {
                let (socket, _) = match result {
                    Ok(value) => value,
                    Err(error) => {
                        failure = Some(MetricsError::Accept { kind: error.kind() });
                        break;
                    }
                };
                if let Ok(permit) = Arc::clone(&permits).try_acquire_owned() {
                    let token = Arc::clone(&token);
                    let sources = Arc::clone(&sources);
                    clients.spawn(async move {
                        let _permit = permit;
                        handle_client(socket, &token, &sources).await;
                    });
                }
            }
            _ = clients.join_next(), if !clients.is_empty() => {}
        }
    }
    clients.abort_all();
    while clients.join_next().await.is_some() {}
    failure.map_or(Ok(()), Err)
}

async fn handle_client(mut socket: TcpStream, token: &str, sources: &MetricsSources) {
    let request = tokio::time::timeout(HEADER_TIMEOUT, read_header(&mut socket)).await;
    let (status, body) = match request {
        Ok(Ok(Some(header))) => {
            let Some((method, authorized)) = parse_request(&header, token) else {
                return;
            };
            if !authorized {
                ("401 Unauthorized", String::new())
            } else if method != "GET /metrics HTTP/1.1" {
                ("404 Not Found", String::new())
            } else {
                ("200 OK", sources.render())
            }
        }
        Ok(Ok(None)) => ("431 Request Header Fields Too Large", String::new()),
        Ok(Err(_)) | Err(_) => return,
    };
    let authenticate = if status == "401 Unauthorized" {
        "WWW-Authenticate: Bearer realm=\"seata-metrics\"\r\n"
    } else {
        ""
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\n{authenticate}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
}

async fn read_header(socket: &mut TcpStream) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut bytes = Vec::with_capacity(512);
    loop {
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(Some(bytes));
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Ok(None);
        }
        let mut buffer = [0u8; 512];
        let available = (MAX_HEADER_BYTES - bytes.len()).min(buffer.len());
        let count = socket.read(&mut buffer[..available]).await?;
        if count == 0 {
            return Ok(None);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}

fn parse_request<'a>(header: &'a [u8], token: &str) -> Option<(&'a str, bool)> {
    let text = std::str::from_utf8(header).ok()?;
    let mut lines = text.split("\r\n");
    let method = lines.next()?;
    let mut authorization = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("authorization")
            && authorization.replace(value.trim_ascii()).is_some()
        {
            return Some((method, false));
        }
    }
    let valid = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| equal_secret(token.as_bytes(), value.as_bytes()));
    Some((method, valid))
}

fn equal_secret(expected: &[u8], actual: &[u8]) -> bool {
    let mut difference = expected.len() ^ actual.len();
    for index in 0..expected.len().max(actual.len()) {
        difference |= usize::from(
            expected.get(index).copied().unwrap_or(0) ^ actual.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}
