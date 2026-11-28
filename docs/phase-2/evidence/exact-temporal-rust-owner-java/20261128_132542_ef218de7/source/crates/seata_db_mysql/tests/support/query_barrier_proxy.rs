//! 自有loopback非TLS测试：扣住指定客户端原始MySQL帧，不改写任何协议内容。
use std::{
    io,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, Notify},
    task::{JoinHandle, JoinSet},
};

/// 完整值查询尚未送往真实MySQL时的测试屏障，不注入生产钩子。
pub struct QueryBarrierProxy {
    pub port: u16,
    captured_packet: Arc<Mutex<Option<Vec<u8>>>>,
    release: Arc<Notify>,
    stop: Option<oneshot::Sender<()>>,
    owner: JoinHandle<()>,
}
impl QueryBarrierProxy {
    pub async fn start(upstream_port: u16, command_prefix: Vec<u8>) -> Self {
        assert!(command_prefix.starts_with(b"\x16SELECT "));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("owned query barrier bind");
        let port = listener.local_addr().expect("query barrier address").port();
        let captured_packet = Arc::new(Mutex::new(None));
        let release = Arc::new(Notify::new());
        let (stop, mut stopping) = oneshot::channel();
        let record = captured_packet.clone();
        let gate = release.clone();
        let owner = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            let mut connections = 0;
            loop {
                tokio::select! {
                    _=&mut stopping=>break,
                    Some(_)=workers.join_next(),if !workers.is_empty()=>{},
                    incoming=listener.accept()=>{
                        let Ok((client,_))=incoming else{break};connections+=1;if connections>8{drop(client);continue;}
                        let record=record.clone();let gate=gate.clone();let prefix=command_prefix.clone();
                        workers.spawn(async move {
                            let server=TcpStream::connect(("127.0.0.1",upstream_port)).await?;
                            let (mut client_read,mut client_write)=client.into_split();let(mut server_read,mut server_write)=server.into_split();
                            let forward=async {
                                loop {
                                    let mut header=[0u8;4];
                                    if let Err(error)=client_read.read_exact(&mut header).await{
                                        return if error.kind()==io::ErrorKind::UnexpectedEof{Ok(())}else{Err(error)};
                                    }
                                    let length=u32::from_le_bytes([header[0],header[1],header[2],0]) as usize;
                                    if length>1_048_576{return Err(io::Error::new(io::ErrorKind::InvalidData,"test client frame exceeds budget"));}
                                    let mut payload=vec![0;length];client_read.read_exact(&mut payload).await?;
                                    // 原包已完整收到但未转发：此时完整值查询尚未PREPARE，更不可能EXECUTE。
                                    let pause=if payload.starts_with(&prefix){
                                        let mut captured=record.lock().expect("test capture record");
                                        if captured.is_none(){let mut raw=header.to_vec();raw.extend_from_slice(&payload);*captured=Some(raw);true}else{false}
                                    }else{false};
                                    if pause{
                                        gate.notified().await;
                                    }
                                    server_write.write_all(&header).await?;server_write.write_all(&payload).await?;
                                }
                            };
                            let backward=async {tokio::io::copy(&mut server_read,&mut client_write).await.map(|_|())};
                            // 任一方向EOF立即释放两端；不能让测试代理自己延长已取消的数据库会话。
                            tokio::select! {result=forward=>result,result=backward=>result}
                        });
                    }
                }
            }
            workers.abort_all();
            while workers.join_next().await.is_some() {}
        });
        Self {
            port,
            captured_packet,
            release,
            stop: Some(stop),
            owner,
        }
    }
    pub fn captured(&self) -> bool {
        self.captured_packet
            .lock()
            .expect("test capture record")
            .is_some()
    }
    pub fn captured_packet(&self) -> Option<Vec<u8>> {
        self.captured_packet
            .lock()
            .expect("test capture record")
            .clone()
    }
    pub fn release(&self) {
        self.release.notify_one();
    }
    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = (&mut self.owner).await;
    }
}
impl Drop for QueryBarrierProxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
