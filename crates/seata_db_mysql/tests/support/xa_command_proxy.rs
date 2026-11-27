//! 自有明文 MySQL 夹具：只截留首个已武装的精确 XA 命令 的原服务端 OK，不伪造或重放字节。
use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
/// 只在自有明文 MySQL 夹具中截留原成功响应并统计实际请求，不改写或重放字节。
/// 对应 Java: 无直接对象；Rust XA 故障测试代理。
pub struct XaCommandProxy {
    /// 当前夹具的回环监听端口。
    pub port: u16,
    armed: Arc<AtomicBool>,
    attempts: Arc<AtomicUsize>,
    skip: Arc<AtomicUsize>,
    // 原请求和原服务端包作为一个关联对保留，仅供自有故障夹具核验。
    #[allow(clippy::type_complexity)]
    captured: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>>,
    stop: Option<oneshot::Sender<()>>,
    owner: JoinHandle<()>,
}
// 用两个目标工具链均支持的 CAS，避免较新工具链已弃用的 fetch_update。
fn should_capture(flag: &AtomicBool, skip: &AtomicUsize) -> bool {
    if !flag.load(Ordering::Acquire) {
        return false;
    }
    loop {
        let remaining = skip.load(Ordering::Acquire);
        if remaining == 0 {
            return flag.swap(false, Ordering::AcqRel);
        }
        if skip
            .compare_exchange(
                remaining,
                remaining - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            return false;
        }
    }
}
async fn packet(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
    let mut header = [0; 4];
    reader.read_exact(&mut header).await?;
    let size = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
    if size > 1_048_576 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "owned MySQL packet budget",
        ));
    }
    let mut value = vec![0; size + 4];
    value[..4].copy_from_slice(&header);
    reader.read_exact(&mut value[4..]).await?;
    Ok(value)
}
impl XaCommandProxy {
    /// 参数为自有上游端口及精确 COM_QUERY 命令字节；返回有界连接的已启动代理。
    pub async fn start(upstream: u16, command: Vec<u8>) -> Self {
        assert!(command.starts_with(b"\x03XA ") || command == b"\x03COMMIT");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let armed = Arc::new(AtomicBool::new(false));
        let captured = Arc::new(Mutex::new(None));
        let skip = Arc::new(AtomicUsize::new(0));
        let skipping = skip.clone();
        let attempts = Arc::new(AtomicUsize::new(0));
        let counted = attempts.clone();
        let (stop, mut stopping) = oneshot::channel();
        let flag = armed.clone();
        let record = captured.clone();
        let owner = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            let mut accepted = 0;
            loop {
                tokio::select! {
                    _=&mut stopping=>break,
                    Some(_)=workers.join_next(),if !workers.is_empty()=>{},
                    incoming=listener.accept()=>{
                        let (client,_)=incoming.unwrap();accepted+=1;if accepted>256 {drop(client);continue;}
                        let flag=flag.clone();let record=record.clone();let command=command.clone();let counted=counted.clone();let skip=skipping.clone();
                        workers.spawn(async move {
                            let server=TcpStream::connect(("127.0.0.1",upstream)).await?;
                            let (mut cr,mut cw)=client.into_split();let (mut sr,mut sw)=server.into_split();
                            let request=Arc::new(Mutex::new(None));let pending=request.clone();
                            let forward=async {
                                loop {let frame=packet(&mut cr).await?;
                                    if frame[4..]==command {counted.fetch_add(1,Ordering::AcqRel);if should_capture(&flag, &skip) {*request.lock().unwrap()=Some(frame.clone());}}
                                    sw.write_all(&frame).await?;
                                }
                                #[allow(unreachable_code)] Ok::<(),io::Error>(())
                            };
                            let backward=async {
                                loop {let frame=packet(&mut sr).await?;
                                    let commit=pending.lock().unwrap().take();
                                    if let Some(commit)=commit {
                                        assert_eq!(frame[3],1);assert_eq!(frame[4],0,"original XA command must return actual OK");
                                        *record.lock().unwrap()=Some((commit,frame));std::future::pending::<()>().await;
                                    } else {cw.write_all(&frame).await?;}
                                }
                                #[allow(unreachable_code)] Ok::<(),io::Error>(())
                            };
                            tokio::select!{value=forward=>value,value=backward=>value}
                        });
                    }
                }
            }
            workers.abort_all();
            while workers.join_next().await.is_some() {}
        });
        Self {
            port,
            armed,
            attempts,
            skip,
            captured,
            stop: Some(stop),
            owner,
        }
    }
    /// 无参数；截留下一次精确匹配命令的原成功响应，不影响其他命令。
    pub fn arm(&self) {
        self.arm_after(0);
    }
    /// 参数为武装后允许正常转发的精确匹配次数；随后仅截留一次原服务端成功响应。
    pub fn arm_after(&self, skip: usize) {
        self.skip.store(skip, Ordering::Release);
        self.armed.store(true, Ordering::Release);
    }
    /// 无参数；返回实际原请求和被扣留的原响应快照，不伪造成功。
    pub fn captured(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.captured.lock().unwrap().clone()
    }
    /// 无参数；等待代理及所有连接结束，返回完整观察期间匹配的原命令次数。
    pub async fn shutdown(mut self) -> usize {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        (&mut self.owner).await.unwrap();
        self.attempts.load(Ordering::Acquire)
    }
}
impl Drop for XaCommandProxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
