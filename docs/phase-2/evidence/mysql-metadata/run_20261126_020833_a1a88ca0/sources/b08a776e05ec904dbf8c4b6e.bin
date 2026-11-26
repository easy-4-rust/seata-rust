//! 仅限自有MySQL测试：正常透传后明确扣住服务端字节，不重写或伪造协议结果。
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

pub struct StallProxy {
    pub port: u16,
    stalled: Arc<AtomicBool>,
    held: Arc<AtomicUsize>,
    stop: Option<oneshot::Sender<()>>,
    owner: JoinHandle<()>,
}
impl StallProxy {
    pub async fn start(upstream_port: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("owned proxy bind");
        let port = listener.local_addr().expect("proxy address").port();
        let stalled = Arc::new(AtomicBool::new(false));
        let held = Arc::new(AtomicUsize::new(0));
        let (stop, mut stopping) = oneshot::channel();
        let flag = stalled.clone();
        let count = held.clone();
        let owner = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            let mut accepted = 0;
            loop {
                tokio::select! {
                    _=&mut stopping=>break,
                    Some(_)=workers.join_next(), if !workers.is_empty()=>{},
                    incoming=listener.accept()=>{
                        let Ok((client,_))=incoming else {break};
                        accepted+=1;if accepted>8 {drop(client);continue;}
                        let flag=flag.clone();let count=count.clone();
                        workers.spawn(async move {
                            let server=TcpStream::connect(("127.0.0.1",upstream_port)).await?;
                            let (mut client_read,mut client_write)=client.into_split();
                            let (mut server_read,mut server_write)=server.into_split();
                            let forward=async {tokio::io::copy(&mut client_read,&mut server_write).await.map(|_|())};
                            let backward=async {
                                let mut buffer=[0u8;4096];
                                loop {
                                    let length=server_read.read(&mut buffer).await?;
                                    if length==0 {return Ok::<(),std::io::Error>(());}
                                    if flag.load(Ordering::Acquire) {
                                        count.fetch_add(length,Ordering::AcqRel);
                                        std::future::pending::<()>().await;
                                    }
                                    client_write.write_all(&buffer[..length]).await?;
                                }
                            };
                            tokio::try_join!(forward,backward).map(|_|())
                        });
                    }
                }
            }
            workers.abort_all();
            while workers.join_next().await.is_some() {}
        });
        Self {
            port,
            stalled,
            held,
            stop: Some(stop),
            owner,
        }
    }
    pub fn stall(&self) {
        self.stalled.store(true, Ordering::Release);
    }
    pub fn held_bytes(&self) -> usize {
        self.held.load(Ordering::Acquire)
    }
    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = (&mut self.owner).await;
    }
}
impl Drop for StallProxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
