//! 验证托管回滚控制事务的首错保留及 SQLx 连接清理。
use super::MySqlDataSourceManager;
use crate::{
    data_source_proxy::rollback_sqlx_tests::{assert_sqlx_rollback_cleanup, loopback_source},
    mysql_loopback_fixture::{read_command, reject_authentication},
    recovery_action::RecoveryAction,
    DbError, MySqlDataSource, MySqlRecoveryError,
};
use seata_at::MySqlJdbcProfile;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
pub(crate) mod rm_fixture;
use seata_protocol::{
    protocol::RegisterRMResponse, transaction::BranchRollbackRequest, BranchStatus, Message,
    RequestType, ResultCode,
};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    JournalOptions, ResourceCatalog, ResourceManager,
};
use std::{
    error::Error,
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    net::TcpListener,
    time::{timeout, Instant},
};

#[tokio::test]
async fn managed_sqlx_preserves_metadata_failure_after_rollback_error() -> Result<(), Box<dyn Error>>
{
    timeout(
        Duration::from_secs(8),
        assert_sqlx_rollback_cleanup(false, check_managed_failure),
    )
    .await?
}

#[tokio::test]
async fn managed_sqlx_preserves_metadata_failure_after_rollback_timeout(
) -> Result<(), Box<dyn Error>> {
    timeout(
        Duration::from_secs(8),
        assert_sqlx_rollback_cleanup(true, check_managed_failure),
    )
    .await?
}

async fn check_managed_failure(source: &MySqlDataSource) -> Result<(), Box<dyn Error>> {
    let manager = MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)?;
    let identity = BranchIdentity::new("managed-cleanup:42".into(), 42, manager.key.clone(), None)?;
    let failure = manager
        .control(
            &identity,
            RecoveryAction::Rollback,
            Instant::now() + source.operation_timeout(),
        )
        .await
        .err()
        .ok_or("fixture must reject metadata access")?;
    assert!(matches!(
        failure,
        MySqlRecoveryError::BeforeCommit {
            identity: ref original,
            source: DbError::Driver {
                operation: "metadata visibility",
                sql_state: Some(ref state),
                code: Some(1142),
                ..
            },
        } if **original == identity && state == "42000"
    ));
    Ok(())
}

#[tokio::test]
async fn managed_acquisition_timeout_preserves_phase_before_begin() -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(8), async {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let source = loopback_source(listener.local_addr()?.port(), Duration::from_millis(250));
        let (release, wait_release) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await?;
            crate::mysql_loopback_fixture::authenticate(&mut socket, 1).await?;
            tokio::select! {
                biased;
                command = read_command(&mut socket) => return Err(io::Error::other(format!("unexpected managed SQL: {:?}", command?))),
                accepted = listener.accept() => return Err(io::Error::other(format!("unexpected second connection: {}", accepted?.1))),
                ready = wait_release => ready.map_err(io::Error::other)?,
            }
            crate::data_source_proxy::rollback_sqlx_tests::serve_idle_connection(&mut socket).await
        });
        let lease = source.control_pool().acquire().await?;
        let manager = MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)?;
        let identity = BranchIdentity::new("xid".into(), 42, manager.key().clone(), None)?;
        let error = manager.control(&identity, RecoveryAction::Rollback, Instant::now() + Duration::from_secs(2))
            .await.err().ok_or("managed control unexpectedly acquired a connection")?;
        assert!(matches!(
            error,
            MySqlRecoveryError::BeforeCommit {
                identity: ref original,
                source: DbError::Deadline { operation: "RM recovery acquire" },
            } if **original == identity
        ), "lost managed acquisition stage: {error:?}");
        release.send(()).map_err(|()| "pool fixture ended early")?;
        drop(lease);
        source.close().await?;
        peer.await??;
        Ok(())
    }).await?
}

#[tokio::test]
async fn managed_acquisition_failure_replies_retryable_and_keeps_durable_pending(
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(10), async {
        let mysql = TcpListener::bind("127.0.0.1:0").await?;
        let tc = TcpListener::bind("127.0.0.1:0").await?;
        let source = loopback_source(mysql.local_addr()?.port(), Duration::from_secs(3));
        let mysql_peer = tokio::spawn(async move {
            for handshake in [1, 2] {
                let (mut socket, _) = mysql.accept().await?;
                reject_authentication(&mut socket, handshake).await?;
                assert!(read_command(&mut socket).await?.is_none(), "SQL after failed authentication");
            }
            assert!(timeout(Duration::from_millis(75), mysql.accept()).await.is_err());
            Ok::<(), io::Error>(())
        });
        let manager = Arc::new(MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)?);
        let key = manager.key().clone();
        let catalog = ResourceCatalog::new(4, 4096)?;
        catalog.register(manager)?;
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
        let dir = PathBuf::from(format!("D:/codex-builds/seata-managed-acquisition-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let journal = dir.join("branches.log");
        let options = JournalOptions::new(16, 1048576, 65536, 16)?;
        let store = FileBranchStore::open(journal.clone(), options).await?;
        let rm = ResourceManager::builder(
            rm_fixture::settings(tc.local_addr()?),
            rm_fixture::registration(),
            catalog,
            store.clone(),
        ).build()?;
        let (mut socket, _) = tc.accept().await?;
        let register = rm_fixture::peer::read(&mut socket).await;
        assert!(matches!(register.body, Some(Message::RegisterRMRequest(request)) if request.resource_ids.as_deref() == Some(key.resource_id())));
        rm_fixture::peer::write(&mut socket, rm_fixture::peer::reply(
            register.id,
            Message::RegisterRMResponse(RegisterRMResponse { identified: true, wire_version: None }),
        )).await;

        let identity = BranchIdentity::new("xid".into(), 42, key.clone(), None)?;
        for wire_id in [-101, -102] {
            let mut request = rm_fixture::peer::reply(wire_id, Message::BranchRollbackRequest(BranchRollbackRequest {
                xid: identity.xid().into(),
                branch_id: identity.branch_id(),
                branch_type: key.branch_type(),
                resource_id: key.resource_id().into(),
                application_data: None,
            }));
            request.message_type = RequestType::RequestSync;
            request.head_map = vec![("nullable".into(), None)];
            rm_fixture::peer::write(&mut socket, request.clone()).await;
            let response = rm_fixture::peer::read(&mut socket).await;
            assert_eq!(response.id, wire_id);
            assert_eq!(response.head_map, request.head_map);
            assert!(matches!(response.body, Some(Message::BranchRollbackResponse(value))
                if value.xid.as_deref() == Some(identity.xid())
                    && value.branch_id == identity.branch_id()
                    && value.response.result_code == ResultCode::Failed
                    && value.branch_status == BranchStatus::PhaseTwoRollbackFailedRetryable
                    && value.branch_status.to_java_v1() == 9));
            assert_eq!(store.lease(identity.clone(), BranchPhase::Rollback).await?.state(), Some(BranchRecordState::Pending));
        }
        rm.shutdown().await?;
        rm_fixture::peer::eof(&mut socket).await;
        drop(store);
        let reopened = FileBranchStore::open(journal, options).await?;
        assert_eq!(reopened.lease(identity, BranchPhase::Rollback).await?.state(), Some(BranchRecordState::Pending));
        source.close().await?;
        mysql_peer.await??;
        Ok(())
    }).await?
}
