//! 原事务读取、拒绝、取消、丢弃和原生 RBS 值的真实数据库边界。
use rbatis::executor::{Executor, RBatisRef};
use rbs::Value;
use seata_db_mysql::{DbError, MySqlDataSource};
use seata_orm_rbatis::RbatisMySqlExecutor;
use sqlx::MySqlConnection;
use std::{path::Path, sync::Arc, time::Duration};
fn first(value: Value) -> rbs::value::map::ValueMap {
    let Value::Array(mut rows) = value else {
        panic!("native rows not array")
    };
    assert_eq!(rows.len(), 1);
    let Value::Map(row) = rows.remove(0) else {
        panic!("native row not map")
    };
    row
}
pub async fn run(source: &MySqlDataSource, admin: &mut MySqlConnection, out: &Path) {
    sqlx::raw_sql("DROP TABLE IF EXISTS orm_read_types,orm_read_limits; CREATE TABLE orm_read_types(id BIGINT UNSIGNED PRIMARY KEY,v INT NOT NULL,note VARCHAR(16) NOT NULL,d DATE NOT NULL,t TIME(6) NOT NULL,dt DATETIME(6) NOT NULL,ts TIMESTAMP(6) NOT NULL) ENGINE=InnoDB; INSERT INTO orm_read_types VALUES(1,10,'one','2026-11-28','01:02:03.123456','2026-11-28 09:10:11.123456','2026-11-28 09:10:11.123456'); CREATE TABLE orm_read_limits(id INT PRIMARY KEY,body LONGBLOB NULL) ENGINE=InnoDB").execute(&mut *admin).await.unwrap();
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    assert!(executor.rb_ref().get_pool().is_err());
    assert!(executor.rb_ref().acquire_begin().await.is_err());
    let mut rejected = 0;
    for text in [
        "SELECT SLEEP(1) FROM orm_read_types",
        "SELECT GET_LOCK('rbatis-query',1) FROM orm_read_types",
        "SELECT @a:=v FROM orm_read_types",
        "SELECT v INTO OUTFILE '/tmp/rbatis-file' FROM orm_read_types",
        "SELECT v FROM orm_read_types FOR UPDATE",
        "SELECT v FROM orm_read_types; DELETE FROM orm_read_types",
    ] {
        assert!(executor.query(text, vec![]).await.is_err());
        rejected += 1;
    }
    for text in ["COMMIT", "DROP TABLE orm_read_types"] {
        assert!(executor.exec(text, vec![]).await.is_err());
        rejected += 1;
    }
    assert!(executor
        .query(
            "SELECT id FROM orm_read_types LIMIT ?",
            vec![Value::U64(1025)]
        )
        .await
        .is_err());
    rejected += 1;
    let row = first(
        executor
            .query(
                "SELECT note AS z,id AS a,v,d,t,dt,ts FROM orm_read_types WHERE id=1",
                vec![],
            )
            .await
            .unwrap(),
    );
    assert_eq!(row.0.get_index(0).unwrap().0, &Value::String("z".into()));
    assert_eq!(row.0.get_index(1).unwrap().0, &Value::String("a".into()));
    assert_eq!(row["a"], Value::U64(1));
    assert_eq!(row["z"], Value::String("one".into()));
    assert_eq!(row["d"], Value::String("2026-11-28".into()));
    assert_eq!(row["t"], Value::String("01:02:03.123456".into()));
    assert_eq!(
        row["dt"],
        Value::String("2026-11-28 09:10:11.123456".into())
    );
    assert_eq!(
        row["ts"],
        Value::String("2026-11-28T09:10:11.123456+00:00".into())
    );
    assert_eq!(row["v"].as_i64(), Some(10));
    sqlx::query("UPDATE orm_read_types SET v=11 WHERE id=1")
        .execute(&mut *admin)
        .await
        .unwrap();
    assert_eq!(
        first(
            executor
                .query("SELECT v FROM orm_read_types WHERE id=1", vec![])
                .await
                .unwrap()
        )["v"]
            .as_i64(),
        Some(10)
    );
    executor.into_transaction().rollback().await.unwrap();
    // 配置不被静默忽略；拒绝必须发生在原事务执行前。
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    executor.rb_ref().intercepts.push(Arc::new(
        rbatis::plugin::intercept_page::PageIntercept::new(),
    ));
    assert!(executor
        .exec("UPDATE orm_read_types SET v=22 WHERE id=1", vec![])
        .await
        .is_err());
    assert!(executor
        .query("SELECT id FROM orm_read_types", vec![])
        .await
        .is_err());
    executor.rb_ref().intercepts.clear();
    for query in [false, true] {
        let mut deep = Value::Null;
        for _ in 0..40_000 {
            deep = Value::Ext("bad", Box::new(deep));
        }
        if query {
            drop(executor.query("COMMIT", vec![deep]));
        } else {
            drop(executor.exec("COMMIT", vec![deep]));
        }
    }
    assert!(executor.into_transaction().undo_logs().is_empty());
    // 不提交而丢弃执行器，原 SQLx 事务的回滚必须释放真实 MDL。
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    executor
        .exec("UPDATE orm_read_types SET v=22 WHERE id=1", vec![])
        .await
        .unwrap();
    drop(executor);
    tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query("ALTER TABLE orm_read_types ADD COLUMN extra INT NULL").execute(&mut *admin),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT v FROM orm_read_types WHERE id=1")
            .fetch_one(&mut *admin)
            .await
            .unwrap(),
        11
    );
    sqlx::query("INSERT INTO orm_read_limits VALUES(1,REPEAT(X'61',1048577))")
        .execute(&mut *admin)
        .await
        .unwrap();
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    assert!(executor
        .query("SELECT id FROM orm_read_limits", vec![])
        .await
        .is_ok());
    assert!(executor
        .query("SELECT body FROM orm_read_limits", vec![])
        .await
        .is_err());
    let mut local = executor.into_transaction();
    assert!(matches!(
        local.table_meta("orm_read_types").await,
        Err(DbError::Poisoned)
    ));
    local.rollback().await.unwrap();
    sqlx::query("TRUNCATE orm_read_limits")
        .execute(&mut *admin)
        .await
        .unwrap();
    let rows = (1..=1025)
        .map(|id| format!("({id},NULL)"))
        .collect::<Vec<_>>()
        .join(",");
    sqlx::query(&format!("INSERT INTO orm_read_limits VALUES {rows}"))
        .execute(&mut *admin)
        .await
        .unwrap();
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    assert!(executor
        .query("SELECT id FROM orm_read_limits", vec![])
        .await
        .is_err());
    executor.into_transaction().rollback().await.unwrap();
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    sqlx::raw_sql("LOCK TABLES orm_read_types WRITE")
        .execute(&mut *admin)
        .await
        .unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(100),
        executor.query("SELECT id FROM orm_read_types", vec![])
    )
    .await
    .is_err());
    sqlx::raw_sql("UNLOCK TABLES")
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut local = executor.into_transaction();
    assert!(matches!(
        local.table_meta("orm_read_types").await,
        Err(DbError::Poisoned)
    ));
    assert!(local.rollback().await.is_err());
    let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
    assert!(executor
        .query("SELECT id FROM orm_read_types", vec![])
        .await
        .is_ok());
    executor.into_transaction().rollback().await.unwrap();
    std::fs::write(out.join("read-checks.json"),serde_json::to_vec_pretty(&serde_json::json!({"rejected":rejected,"native_column_order":["z","a"],"native_date":row["d"].as_str().unwrap(),"native_time":row["t"].as_str().unwrap(),"native_timestamp_utc":row["ts"].as_str().unwrap(),"rr_first":10,"rr_after_external_commit":10,"external_committed":11,"large_selected_rejected":true,"unselected_blob_allowed":true,"row_1025_rejected":true,"mdl_cancelled_poisoned":true,"fresh_scope_succeeded":true,"configured_interceptors_rejected":2,"unpolled_deep_values_discarded":2,"dropped_executor_rolled_back_and_released_mdl":true})).unwrap()).unwrap();
}
