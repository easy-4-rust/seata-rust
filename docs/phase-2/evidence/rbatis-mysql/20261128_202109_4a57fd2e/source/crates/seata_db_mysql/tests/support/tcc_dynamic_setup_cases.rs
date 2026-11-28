//! 显式启用、最小连接预算及缺失迁移的真实恢复边界。
#![allow(clippy::unwrap_used)]
use super::{fixture, manager, peer};
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager, MySqlTccTryError};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and explicit optional-report setup rejection"]
async fn disabled_reporting_small_pool_and_missing_migration_never_start_business() {
    let mut cases = Vec::new();
    for (index, mode) in ["missing_migration", "small_pool", "disabled"]
        .into_iter()
        .enumerate()
    {
        let (mut admin, seed, _) = fixture::seed().await;
        seed.close().await.unwrap();
        sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
            .execute(&mut admin)
            .await
            .unwrap();
        if mode != "missing_migration" {
            sqlx::raw_sql(include_str!(
                "../../migrations/020_seata_rs_tcc_context_v1.sql"
            ))
            .execute(&mut admin)
            .await
            .unwrap();
        }
        let options = fixture::owned_mysql::options(None);
        let source = MySqlDataSource::connect(if mode == "small_pool" {
            options.pool_limits(1, Duration::from_secs(1)).unwrap()
        } else {
            options
        })
        .await
        .unwrap();
        let adapter = MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap();
        let adapter = Arc::new(if mode == "disabled" {
            adapter
        } else {
            adapter.with_context_reporting()
        });
        let id = BranchIdentity::new(
            format!("tcc:dynamic:setup:{mode}"),
            9800 + index as i64,
            adapter.key().clone(),
            Some(fixture::app(1)),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, store) = manager(adapter.clone(), &listener, mode).await;
        let original = id.clone();
        let (bootstrapped, ready) = tokio::sync::oneshot::channel();
        let (resume, resumed) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
            bootstrapped.send(()).unwrap();
            if mode != "small_pool" {
                peer::registered(
                    &mut socket,
                    original.resource().resource_id(),
                    original.xid(),
                    original.application_data(),
                    original.branch_id(),
                )
                .await;
            }
            resumed.await.unwrap();
            if mode == "small_pool" {
                peer::await_eof(&mut socket).await;
            } else {
                for request in [-701, -702] {
                    assert_eq!(
                        peer::callback(&mut socket, request, &original, BranchPhase::Rollback)
                            .await,
                        (ResultCode::Success, 8)
                    );
                }
            }
        });
        tokio::time::timeout(Duration::from_secs(5), ready)
            .await
            .unwrap()
            .unwrap();
        let error = if mode == "disabled" {
            let receipt = rm
                .client()
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(id.xid().into()),
                    branch_type: BranchType::Tcc,
                    resource_id: Some(id.resource().resource_id().into()),
                    lock_key: None,
                    application_data: id.application_data().map(str::to_owned),
                })
                .await
                .unwrap();
            adapter
                .try_registered_with_context(&rm.client(), receipt)
                .await
                .unwrap_err()
        } else {
            adapter
                .register_and_try(
                    &rm.client(),
                    id.xid(),
                    id.application_data().map(str::to_owned),
                )
                .await
                .unwrap_err()
        };
        if mode == "small_pool" {
            assert!(matches!(&error, MySqlTccTryError::Local { .. }));
            assert!(error.registered_branch().is_none());
        } else {
            assert!(matches!(&error, MySqlTccTryError::Refused { .. }));
            assert_eq!(error.registered_branch().unwrap().identity(), &id);
        }
        assert!(fixture::business(&mut admin).await.is_empty());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(count, 0);
        if mode == "missing_migration" {
            sqlx::raw_sql(include_str!(
                "../../migrations/020_seata_rs_tcc_context_v1.sql"
            ))
            .execute(&mut admin)
            .await
            .unwrap();
        }
        resume.send(()).unwrap();
        if mode == "small_pool" {
            rm.shutdown().await.unwrap();
            server.await.unwrap();
        } else {
            server.await.unwrap();
            assert_eq!(
                store
                    .lease(id.clone(), BranchPhase::Rollback)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            rm.shutdown().await.unwrap();
        }
        let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_context_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(versions, 0);
        source.close().await.unwrap();
        cases.push(serde_json::json!({"mode":mode,"xid":id.xid(),"branch_id":id.branch_id(),"original_register_requests":if mode=="small_pool"{0}else{1},"reports":0,"try_business_rows":0,"versions":0,"journal":if mode=="small_pool"{"None"}else{"Completed"},"migration_installed_explicitly_for_recovery":mode=="missing_migration"}));
    }
    fixture::record(
        "dynamic-report-setup",
        serde_json::json!({"cases":cases,"whole_plan_complete":false}),
    );
}
