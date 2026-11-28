//! 两个自有 MySQL 实例共用一个回环 JDBC 地址，受控 TCP 对端核对实际登记。
#![allow(clippy::unwrap_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
#[allow(dead_code)]
mod owned_mysql;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;

use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::BranchRegisterResponse;
use seata_protocol::Message;
use seata_rm::{BranchResource, ResourceCatalog, ResourceManager};
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
#[ignore = "requires two owned real MySQL instances with the same schema"]
async fn different_mysql_servers_cannot_share_a_preparation_or_pool() {
    let primary = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let alternate = std::env::var("SEATA_TEST_SECOND_MYSQL_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let mut primary_admin = owned_mysql::admin_at(primary).await;
    let mut alternate_admin = owned_mysql::admin_at(alternate).await;
    let ident = "SELECT @@server_uuid,DATABASE()";
    let primary_id: (String, String) = sqlx::query_as(ident)
        .fetch_one(&mut primary_admin)
        .await
        .unwrap();
    let alternate_id: (String, String) = sqlx::query_as(ident)
        .fetch_one(&mut alternate_admin)
        .await
        .unwrap();
    assert_eq!(primary_id.1, alternate_id.1);
    assert!(primary_id.1.starts_with("undo_"));
    assert_ne!(primary_id.0, alternate_id.0);

    let router = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = router.local_addr().unwrap().port();
    let selected = Arc::new(AtomicU16::new(primary));
    let relay_route = selected.clone();
    let relay = tokio::spawn(async move {
        let mut sessions = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                incoming = router.accept() => {
                    let (mut client, _) = incoming.unwrap();
                    let backend = relay_route.load(Ordering::SeqCst);
                    sessions.spawn(async move {
                        if let Ok(mut server) = TcpStream::connect(("127.0.0.1", backend)).await {
                            let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                        }
                    });
                }
                _ = sessions.join_next(), if !sessions.is_empty() => {}
            }
        }
    });
    let resource = format!("jdbc:mysql://127.0.0.1:{port}/{}", primary_id.1);
    let schema_path = std::env::var("SEATA_UNDO_SCHEMA_PATH").unwrap();
    assert!(schema_path.starts_with("D:"));
    let schema = std::fs::read_to_string(schema_path).unwrap();
    for admin in [&mut primary_admin, &mut alternate_admin] {
        sqlx::raw_sql(&schema).execute(&mut *admin).await.unwrap();
        for ddl in [
            include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
            include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
            include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
            "CREATE TABLE undo_work(id INT PRIMARY KEY,v VARCHAR(100) NOT NULL) ENGINE=InnoDB",
        ] {
            sqlx::raw_sql(ddl).execute(&mut *admin).await.unwrap();
        }
        sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
            .bind(resource.as_bytes())
            .execute(&mut *admin)
            .await
            .unwrap();
    }

    let options = owned_mysql::options(Some(port))
        .with_java_tc_mysql_db_lock_profile()
        .pool_limits(2, Duration::from_secs(2))
        .unwrap();
    let manager_source = MySqlDataSource::connect(options.clone()).await.unwrap();
    let control = manager_source.begin().await.unwrap();
    selected.store(alternate, Ordering::SeqCst);
    let lease_on_alternate = manager_source.begin().await;
    let rejected_mixed_pool = lease_on_alternate.is_err();
    if let Ok(lease) = lease_on_alternate {
        lease.rollback().await.unwrap();
    }
    let alternate_source = MySqlDataSource::connect(options).await.unwrap();
    assert_eq!(
        manager_source.resource_id().unwrap(),
        alternate_source.resource_id().unwrap()
    );
    let mut tx = alternate_source.begin().await.unwrap();
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'alternate')")
            .unwrap();
    tx.execute(&insert, &[]).await.unwrap();
    selected.store(primary, Ordering::SeqCst);
    control.rollback().await.unwrap();
    manager_source
        .begin()
        .await
        .unwrap()
        .rollback()
        .await
        .unwrap();

    let manager = Arc::new(
        MySqlDataSourceManager::new(manager_source.clone(), MySqlJdbcProfile::Connector8Default)
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(manager.clone()).unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(2))
    .build()
    .unwrap();
    let key = manager.key().clone();
    let peer = async {
        let mut socket = callback_peer::registered(&listener, &key).await;
        let Ok(request) = tokio::time::timeout(
            Duration::from_millis(700),
            rm_fixture::peer::read(&mut socket),
        )
        .await
        else {
            return false;
        };
        assert!(matches!(
            request.body,
            Some(Message::BranchRegisterRequest(_))
        ));
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: rm_fixture::peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        true
    };
    let client = rm.client();
    let (preparation, sent) = tokio::join!(
        manager.prepare_transaction(tx, &client, "mysql:physical-switch", None),
        peer
    );
    let locally_rejected = matches!(
        &preparation,
        Err(MySqlPrepareError::Local {
            source: DbError::Unsupported {
                feature: "preparation requires the original physical MySQL schema"
            }
        })
    );
    if let Ok(prepared) = preparation {
        prepared.rollback().await.unwrap();
    }
    rm.shutdown().await.unwrap();
    manager_source.close().await.unwrap();
    alternate_source.close().await.unwrap();
    relay.abort();
    let _ = relay.await;
    let business = fixture::count(&mut alternate_admin, "undo_work").await;
    let recovery = fixture::count(&mut primary_admin, "seata_rs_at_branch_v1").await;
    let undo = fixture::count(&mut alternate_admin, "undo_log").await;
    fixture::save(
        "physical-identity.json",
        &serde_json::json!({
            "primary":primary_id,"alternate":alternate_id,"resource":resource,"pool_rejected_alternate":rejected_mixed_pool,
            "preparation_rejected_locally":locally_rejected,"branch_register_sent":sent,"business":business,
            "recovery":recovery,"undo":undo
        }),
    );
    assert_eq!(
        (rejected_mixed_pool, locally_rejected, sent),
        (true, true, false)
    );
    assert_eq!((business, recovery, undo), (0, 0, 0));
}
