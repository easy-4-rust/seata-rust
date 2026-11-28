//! 单文件原字节写入任务；所有关闭等待均保留当前 I/O Future。
use crate::{
    log_file_progress::LogFileProgress, StructuredLogFileConfig, StructuredLogFileError,
    StructuredLogFileReport, StructuredLogFileStage, StructuredLogReceiver,
};
use std::{future::Future, io, io::SeekFrom, sync::Arc};
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::oneshot,
    time::{Instant, MissedTickBehavior},
};

/// 参数为原 I/O Future、停止信号和唯一接收器；停止时关闭接纳，但继续等待同一 I/O。
pub(crate) async fn preserve_io<T: Send>(
    io: impl Future<Output = io::Result<T>> + Send,
    stop: &mut oneshot::Receiver<()>,
    receiver: &mut StructuredLogReceiver,
    stopping: &mut bool,
) -> io::Result<T> {
    tokio::pin!(io);
    if *stopping {
        return io.await;
    }
    tokio::select! {biased;_= &mut *stop=>{receiver.close();*stopping=true;io.await},result=&mut io=>result}
}
async fn synchronize(
    file: &mut File,
    stop: &mut oneshot::Receiver<()>,
    receiver: &mut StructuredLogReceiver,
    stopping: &mut bool,
    report: &mut StructuredLogFileReport,
    progress: &LogFileProgress,
) -> Result<(), StructuredLogFileError> {
    preserve_io(file.flush(), stop, receiver, stopping)
        .await
        .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Flush, *report))?;
    // 只有原系统调用明确成功后才推进同步前缀；失败不授权重放。
    preserve_io(file.sync_all(), stop, receiver, stopping)
        .await
        .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Sync, *report))?;
    report.synchronized_records = report.written_records;
    report.sync_operations = report.sync_operations.saturating_add(1);
    progress.publish(*report);
    Ok(())
}
/// 参数为原消费者、文件能力、配置、停止信号和只读进度载体；返回本任务的确认前缀。
pub(crate) async fn run(
    mut receiver: StructuredLogReceiver,
    file: std::fs::File,
    config: StructuredLogFileConfig,
    mut stop: oneshot::Receiver<()>,
    progress: Arc<LogFileProgress>,
) -> Result<StructuredLogFileReport, StructuredLogFileError> {
    let mut file = File::from_std(file);
    let mut report = StructuredLogFileReport::default();
    let mut stopping = false;
    let metadata = preserve_io(file.metadata(), &mut stop, &mut receiver, &mut stopping)
        .await
        .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Metadata, report))?;
    if !metadata.is_file() || metadata.len() != 0 {
        return Err(StructuredLogFileError::InvalidTarget);
    }
    let position = preserve_io(
        file.seek(SeekFrom::Current(0)),
        &mut stop,
        &mut receiver,
        &mut stopping,
    )
    .await
    .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Position, report))?;
    if position != 0 {
        return Err(StructuredLogFileError::InvalidTarget);
    }
    let mut timer =
        tokio::time::interval_at(Instant::now() + config.sync_interval, config.sync_interval);
    timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let next = if stopping {
            receiver.recv().await
        } else {
            tokio::select! {biased;
                _=&mut stop=>{receiver.close();stopping=true;continue;},
                _=timer.tick()=>{if report.written_records>report.synchronized_records{synchronize(&mut file,&mut stop,&mut receiver,&mut stopping,&mut report,&progress).await?;}continue;},
                next=receiver.recv()=>next,
            }
        };
        let Some(bytes) = next else {
            break;
        };
        report.received_records = report.received_records.saturating_add(1);
        progress.publish(report);
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if length > config.maximum_bytes.saturating_sub(report.written_bytes) {
            if report.written_records > report.synchronized_records {
                synchronize(
                    &mut file,
                    &mut stop,
                    &mut receiver,
                    &mut stopping,
                    &mut report,
                    &progress,
                )
                .await?;
            }
            return Err(StructuredLogFileError::ByteLimit { progress: report });
        }
        preserve_io(
            file.write_all(&bytes),
            &mut stop,
            &mut receiver,
            &mut stopping,
        )
        .await
        .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Write, report))?;
        preserve_io(file.flush(), &mut stop, &mut receiver, &mut stopping)
            .await
            .map_err(|e| StructuredLogFileError::io(e, StructuredLogFileStage::Flush, report))?;
        report.written_records = report.written_records.saturating_add(1);
        report.written_bytes = report.written_bytes.saturating_add(length);
        progress.publish(report);
        if report
            .written_records
            .saturating_sub(report.synchronized_records)
            >= config.records_per_sync
        {
            synchronize(
                &mut file,
                &mut stop,
                &mut receiver,
                &mut stopping,
                &mut report,
                &progress,
            )
            .await?;
        }
    }
    synchronize(
        &mut file,
        &mut stop,
        &mut receiver,
        &mut stopping,
        &mut report,
        &progress,
    )
    .await?;
    Ok(report)
}

#[cfg(test)]
#[path = "log_file_worker_tests.rs"]
mod tests;
