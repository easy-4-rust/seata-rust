//! 原包受控 TCP 与自有 MySQL 的准备夹具，不替代原 Java TC。
use super::{callback_peer, fixture, owned_mysql, recovery, rm_fixture};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlLocalTransaction};
use seata_protocol::transaction::BranchRegisterResponse;
use seata_protocol::Message;
use seata_rm::{BranchResource, ResourceCatalog, ResourceManager};
use std::{sync::Arc, time::Duration};
use tokio::net::{TcpListener, TcpStream};
#[path = "phase_one_metrics_snapshot.rs"]
pub(crate) mod phase_one_metrics_snapshot;

/// 单个测试的自有作用域；只在运行器创建的随机库使用。
pub struct Harness {
    pub admin: sqlx::MySqlConnection,
    pub source: MySqlDataSource,
    pub adapter: Arc<MySqlDataSourceManager>,
    pub rm: ResourceManager,
    listener: TcpListener,
}
impl Harness {
    /// 参数为阶段、结果和次数；核对本夹具原数据源的已结算计数。
    #[allow(dead_code)]
    pub fn assert_phase(
        &self,
        operation: seata_db_mysql::MySqlPhaseOneOperation,
        outcome: seata_db_mysql::MySqlPhaseOneOutcome,
        expected: u64,
    ) {
        assert_eq!(
            self.source
                .phase_one_metrics_sampler()
                .snapshot(operation)
                .count(outcome),
            expected
        );
    }

    pub async fn start(budget: Duration, request_timeout: Duration) -> Self {
        Self::start_with_store(budget, request_timeout, rm_fixture::journal().await).await
    }
    pub async fn start_with_store(
        budget: Duration,
        request_timeout: Duration,
        store: seata_rm::FileBranchStore,
    ) -> Self {
        Self::start_with_store_and_port(budget, request_timeout, store, None).await
    }
    #[cfg(feature = "experimental_changed_commit")]
    #[allow(dead_code)] // 共享夹具在其他集成测试目标中同样编译，只有提交丢包目标使用此入口。
    pub async fn start_with_source_port(
        budget: Duration,
        request_timeout: Duration,
        source_port: u16,
    ) -> Self {
        Self::start_with_store_and_port(
            budget,
            request_timeout,
            rm_fixture::journal().await,
            Some(source_port),
        )
        .await
    }
    async fn start_with_store_and_port(
        budget: Duration,
        request_timeout: Duration,
        store: seata_rm::FileBranchStore,
        source_port: Option<u16>,
    ) -> Self {
        let (mut admin, _) = recovery::empty().await;
        sqlx::query("DROP TABLE IF EXISTS seata_rs_at_commit_attempt")
            .execute(&mut admin)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/002_seata_rs_at_registration_origin.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        let source = MySqlDataSource::connect(
            owned_mysql::options(source_port)
                .operation_timeout(budget)
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(
            MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)
                .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let catalog = ResourceCatalog::new(4, 32767).unwrap();
        catalog.register(adapter.clone()).unwrap();
        let rm = ResourceManager::builder(
            rm_fixture::settings(listener.local_addr().unwrap()),
            rm_fixture::registration(),
            catalog,
            store.clone(),
        )
        .request_timeout(request_timeout)
        .callback_timeout(Duration::from_secs(5))
        .build()
        .unwrap();
        Self {
            admin,
            source,
            adapter,
            rm,
            listener,
        }
    }
    /// 封存原数据源的实际计数快照，不把业务身份放入产品指标。
    #[allow(dead_code)]
    pub fn save_phase_metrics(&self, name: &str) {
        fixture::save(
            name,
            &phase_one_metrics_snapshot::value(&self.source.phase_one_metrics_sampler()),
        );
    }
    pub async fn accept(&self) -> TcpStream {
        callback_peer::registered(&self.listener, self.adapter.key()).await
    }
    pub async fn transaction(&self) -> MySqlLocalTransaction {
        let mut tx = self.source.begin().await.unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'prepared_fixture')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
        tx
    }
    pub async fn closed_counts(mut self, socket: &mut TcpStream) -> (i64, i64, (i64, i64, i64)) {
        self.rm.shutdown().await.unwrap();
        rm_fixture::peer::eof(socket).await;
        self.source.close().await.unwrap();
        phase_one_metrics_snapshot::save(&self.source.phase_one_metrics_sampler());
        (
            fixture::count(&mut self.admin, "undo_work").await,
            fixture::count(&mut self.admin, "undo_log").await,
            recovery::state(&mut self.admin).await,
        )
    }
}
pub async fn request(socket: &mut TcpStream, xid: &str) -> i32 {
    let request = rm_fixture::peer::read(socket).await;
    match request.body {
        Some(Message::BranchRegisterRequest(v)) => {
            assert_eq!(v.xid.as_deref(), Some(xid));
            assert_eq!(v.lock_key.as_deref(), Some("undo_work:1"));
            assert_eq!(v.application_data.as_deref(), Some("prepare-fixture-app"));
        }
        other => panic!("{other:?}"),
    };
    request.id
}
pub async fn positive(socket: &mut TcpStream, id: i32) {
    rm_fixture::peer::write(
        socket,
        rm_fixture::peer::reply(
            id,
            Message::BranchRegisterResponse(BranchRegisterResponse {
                response: rm_fixture::peer::success(),
                branch_id: 42,
            }),
        ),
    )
    .await;
}
