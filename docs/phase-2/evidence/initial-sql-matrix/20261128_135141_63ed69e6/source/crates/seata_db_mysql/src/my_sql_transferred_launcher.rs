use crate::{DbError, MySqlLauncherClaimError, MySqlLauncherSnapshot, MySqlLauncherStore};
use seata_protocol::GlobalStatus;
use seata_tm::{
    DurableTmError, RecoveredEndAuthority, RecoveredEndGate, TmOperation, TransactionManager,
    TransactionOutcome, Xid,
};

/// 已确认转移代际的一次性 TM 结束句柄。对应 Java 无直接类型。
pub struct MySqlTransferredLauncher<'a> {
    pub(crate) snapshot: &'a MySqlLauncherSnapshot,
    pub(crate) manager: &'a TransactionManager,
    pub(crate) epoch: u64,
    pub(crate) operation: TmOperation,
    pub(crate) resource: String,
}

impl MySqlTransferredLauncher<'_> {
    /// 返回已确认的新拥有者代际。
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 消耗句柄，使用同一来源数据库的 TM 写账号确认发送标记，再发送一次原结束操作。
    /// 原 TC 状态或写入确认不满足发送条件时返回原错误，句柄不再使用。
    pub async fn finish(
        self,
        authority: &RecoveredEndAuthority,
        writer: &MySqlLauncherStore,
    ) -> Result<TransactionOutcome, DurableTmError> {
        self.manager
            .send_recovered_end(
                authority,
                MySqlRecoveredEndGate {
                    launcher: self,
                    writer,
                },
            )
            .await
    }
}

struct MySqlRecoveredEndGate<'a, 'writer> {
    launcher: MySqlTransferredLauncher<'a>,
    writer: &'writer MySqlLauncherStore,
}

impl RecoveredEndGate for MySqlRecoveredEndGate<'_, '_> {
    async fn arm(self, manager: &TransactionManager) -> Result<(Xid, TmOperation), DurableTmError> {
        let launcher = self.launcher;
        let snapshot = launcher.snapshot;
        let persist_error = |source: MySqlLauncherClaimError| DurableTmError::Persistence {
            stage: "recovered send marker",
            xid: snapshot.xid().clone(),
            source: Box::new(source),
        };
        if manager.owner_instance_id() != launcher.manager.owner_instance_id()
            || self
                .writer
                .resource_id()
                .map_err(|error| persist_error(error.into()))?
                != launcher.resource
        {
            return Err(persist_error(DbError::RecoveryConflict.into()));
        }
        let state = snapshot
            .query_original_tc(manager, snapshot.business_intent())
            .await?
            .status();
        if state != GlobalStatus::Begin {
            return Err(persist_error(MySqlLauncherClaimError::Observed(state)));
        }
        self.writer
            .mark_transferred_send_started(snapshot, launcher.epoch, manager, launcher.operation)
            .await
            .map_err(|error| persist_error(error.into()))?;
        Ok((snapshot.xid().clone(), launcher.operation))
    }
}
