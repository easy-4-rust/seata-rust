//! 自有明文 MySQL 夹具：只截留首个已武装 COMMIT 的原服务端 OK，不伪造或重放字节。
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
pub struct ControlCommitProxy {
    pub port: u16,
    armed: Arc<AtomicBool>,
    // 原请求和原服务端包作为一个关联对保留，仅供自有故障夹具核验。
    #[allow(clippy::type_complexity)]
    captured: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>>,
    stop: Option<oneshot::Sender<()>>,
    owner: JoinHandle<()>,
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
impl ControlCommitProxy {
    pub async fn start(upstream: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let armed = Arc::new(AtomicBool::new(false));
        let captured = Arc::new(Mutex::new(None));
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
                        let (client,_)=incoming.unwrap();accepted+=1;if accepted>32 {drop(client);continue;}
                        let flag=flag.clone();let record=record.clone();
                        workers.spawn(async move {
                            let server=TcpStream::connect(("127.0.0.1",upstream)).await?;
                            let (mut cr,mut cw)=client.into_split();let (mut sr,mut sw)=server.into_split();
                            let request=Arc::new(Mutex::new(None));let pending=request.clone();
                            let forward=async {
                                loop {let frame=packet(&mut cr).await?;
                                    if frame[4..]==*b"\x03COMMIT" && flag.swap(false,Ordering::AcqRel) {*request.lock().unwrap()=Some(frame.clone());}
                                    sw.write_all(&frame).await?;
                                }
                                #[allow(unreachable_code)] Ok::<(),io::Error>(())
                            };
                            let backward=async {
                                loop {let frame=packet(&mut sr).await?;
                                    let commit=pending.lock().unwrap().take();
                                    if let Some(commit)=commit {
                                        assert_eq!(frame[3],1);assert_eq!(frame[4],0,"original control COMMIT must return actual OK");
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
            captured,
            stop: Some(stop),
            owner,
        }
    }
    pub fn arm(&self) {
        self.armed.store(true, Ordering::Release);
    }
    pub fn captured(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.captured.lock().unwrap().clone()
    }
    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        (&mut self.owner).await.unwrap();
    }
}
impl Drop for ControlCommitProxy {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
