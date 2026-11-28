//! 真实数据库的 RR、原生列序/类型、只读拒绝、结果预算与取消测试。
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use seata_db_mysql::{DbError, MySqlDataSource};
use seata_orm_seaorm::SeaOrmMySqlConnection;
use std::{path::Path, time::Duration};
fn sql(text: &str) -> Statement {
    Statement::from_string(DbBackend::MySql, text.to_owned())
}
pub async fn run(source: &MySqlDataSource, admin: &mut sqlx::MySqlConnection, out: &Path) {
    sqlx::raw_sql("DROP TABLE IF EXISTS orm_read_types,orm_read_limits; CREATE TABLE orm_read_types(id BIGINT UNSIGNED PRIMARY KEY,v INT NOT NULL,note VARCHAR(16) NOT NULL,d DATE NOT NULL,t TIME(6) NOT NULL,dt DATETIME(6) NOT NULL,ts TIMESTAMP(6) NOT NULL) ENGINE=InnoDB; INSERT INTO orm_read_types VALUES(1,10,'one','2026-11-28','01:02:03.123456','2026-11-28 09:10:11.123456','2026-11-28 09:10:11.123456'); CREATE TABLE orm_read_limits(id INT PRIMARY KEY,body LONGBLOB NULL) ENGINE=InnoDB")
        .execute(&mut *admin).await.unwrap();
    let mut local = source.begin().await.unwrap();
    let mut rejected = 0;
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        for text in [
            "SELECT SLEEP(1) FROM orm_read_types",
            "SELECT GET_LOCK('borrowed-query',1) FROM orm_read_types",
            "SELECT @a:=v FROM orm_read_types",
            "SELECT v INTO OUTFILE '/tmp/borrowed-file' FROM orm_read_types",
            "SELECT v FROM orm_read_types FOR UPDATE",
            "SELECT v FROM orm_read_types; DELETE FROM orm_read_types",
        ] {
            assert!(connection.query_all(sql(text)).await.is_err());
            rejected += 1;
        }
        assert!(connection.execute_unprepared("COMMIT").await.is_err());
        rejected += 1;
        assert!(connection
            .execute_unprepared("DROP TABLE orm_read_types")
            .await
            .is_err());
        rejected += 1;
        assert!(connection
            .query_all(Statement::from_sql_and_values(
                DbBackend::MySql,
                "SELECT id FROM orm_read_types LIMIT ?",
                [1025_u64.into()]
            ))
            .await
            .is_err());
        rejected += 1;
        let row = connection
            .query_one(sql(
                "SELECT note AS z, id AS a, v, d, t, dt, ts FROM orm_read_types WHERE id=1",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get_by_index::<String>(0).unwrap(), "one");
        assert_eq!(row.try_get_by_index::<u64>(1).unwrap(), 1);
        assert_eq!(&row.column_names()[..2], &["z", "a"]);
        assert_eq!(
            row.try_get::<sea_orm::prelude::Date>("", "d")
                .unwrap()
                .to_string(),
            "2026-11-28"
        );
        assert_eq!(
            row.try_get::<sea_orm::prelude::Time>("", "t")
                .unwrap()
                .to_string(),
            "01:02:03.123456"
        );
        assert_eq!(
            row.try_get::<sea_orm::prelude::DateTime>("", "dt")
                .unwrap()
                .to_string(),
            "2026-11-28 09:10:11.123456"
        );
        assert_eq!(
            row.try_get::<sea_orm::prelude::DateTimeUtc>("", "ts")
                .unwrap()
                .to_rfc3339(),
            "2026-11-28T09:10:11.123456+00:00"
        );
        sqlx::query("UPDATE orm_read_types SET v=11 WHERE id=1")
            .execute(&mut *admin)
            .await
            .unwrap();
        let second = connection
            .query_one(sql("SELECT v FROM orm_read_types WHERE id=1"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.try_get_by_index::<i32>(0).unwrap(), 10);
    }
    local.rollback().await.unwrap();
    // 大字段先被长度预检拒绝，未选择该字段时仍允许读取主键。
    sqlx::query("INSERT INTO orm_read_limits VALUES(1,REPEAT(X'61',1048577))")
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut local = source.begin().await.unwrap();
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        assert_eq!(
            connection
                .query_all(sql("SELECT id FROM orm_read_limits"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(connection
            .query_all(sql("SELECT body FROM orm_read_limits"))
            .await
            .is_err());
    }
    assert!(matches!(
        local.table_meta("orm_read_types").await,
        Err(DbError::Poisoned)
    ));
    local.rollback().await.unwrap();
    sqlx::query("TRUNCATE orm_read_limits")
        .execute(&mut *admin)
        .await
        .unwrap();
    let values = (1..=1025)
        .map(|id| format!("({id},NULL)"))
        .collect::<Vec<_>>()
        .join(",");
    sqlx::query(&format!("INSERT INTO orm_read_limits VALUES {values}"))
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut local = source.begin().await.unwrap();
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        assert!(connection
            .query_all(sql("SELECT id FROM orm_read_limits"))
            .await
            .is_err());
    }
    assert!(matches!(
        local.table_meta("orm_read_types").await,
        Err(DbError::Poisoned)
    ));
    local.rollback().await.unwrap();
    // 实际 MDL 阻塞期间取消查询，核心拥有式 Future 必须带走物理事务并留下 poison。
    let mut local = source.begin().await.unwrap();
    sqlx::raw_sql("LOCK TABLES orm_read_types WRITE")
        .execute(&mut *admin)
        .await
        .unwrap();
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            connection.query_all(sql("SELECT id FROM orm_read_types"))
        )
        .await
        .is_err());
    }
    sqlx::raw_sql("UNLOCK TABLES")
        .execute(&mut *admin)
        .await
        .unwrap();
    assert!(matches!(
        local.table_meta("orm_read_types").await,
        Err(DbError::Poisoned)
    ));
    assert!(local.rollback().await.is_err());
    let mut fresh = source.begin().await.unwrap();
    {
        let connection = SeaOrmMySqlConnection::new(&mut fresh);
        assert_eq!(
            connection
                .query_all(sql("SELECT id FROM orm_read_types"))
                .await
                .unwrap()
                .len(),
            1
        );
    }
    fresh.rollback().await.unwrap();
    std::fs::write(out.join("read-checks.json"),serde_json::to_vec_pretty(&serde_json::json!({"rejected":rejected,"native_column_order":["z","a"],"native_date":"2026-11-28","native_time":"01:02:03.123456","native_timestamp_utc":"2026-11-28T09:10:11.123456+00:00","rr_first":10,"rr_after_external_commit":10,"external_committed":11,"large_selected_rejected":true,"unselected_blob_allowed":true,"row_1025_rejected":true,"mdl_cancelled_poisoned":true,"fresh_scope_succeeded":true})).unwrap()).unwrap();
}
