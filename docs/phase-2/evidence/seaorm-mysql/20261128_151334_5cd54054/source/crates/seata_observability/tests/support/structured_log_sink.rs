//! 测试消费者明确写入自有文件并 sync_all；不将入队数冒充持久化数。
use seata_observability::{StructuredLogReceiver, StructuredLogSampler};
use std::io::Write;
/// 参数为唯一消费者、计数、固定文件名及实际对端事实；返回已写入并回读的原 JSON 记录。
pub async fn finish(
    mut receiver: StructuredLogReceiver,
    sampler: &StructuredLogSampler,
    name: &str,
    proof: serde_json::Value,
) -> Vec<serde_json::Value> {
    receiver.close();
    let mut bytes = Vec::new();
    let mut values = Vec::new();
    while let Some(line) = receiver.recv().await {
        assert_eq!(line.iter().filter(|b| **b == b'\n').count(), 1);
        values.push(serde_json::from_slice(&line).unwrap());
        bytes.extend(line);
    }
    let folder = std::env::var_os("SEATA_STRUCTURED_LOG_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(format!(
                "D:/codex-builds/structured-log-sink-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
        });
    assert!(
        folder.is_absolute()
            && (folder.to_string_lossy().starts_with("D:")
                || folder.to_string_lossy().starts_with("E:"))
    );
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{name}.ndjson"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let s = sampler.snapshot();
    assert_eq!(s.accepted, s.received);
    assert_eq!(
        s.dropped_invalid + s.dropped_full + s.dropped_closed + s.dropped_oversize,
        0
    );
    let stats = serde_json::json!({"accepted":s.accepted,"received":s.received,"dropped_full":s.dropped_full,"dropped_closed":s.dropped_closed,"dropped_oversize":s.dropped_oversize,"dropped_invalid":s.dropped_invalid});
    std::fs::write(folder.join(format!("{name}-proof.json")),serde_json::to_vec_pretty(&serde_json::json!({"source":proof,"stats":stats,"file_write_all_sync_all_and_readback_confirmed":true})).unwrap()).unwrap();
    values
}
