//! 原生分页统计必须越过行探测上限，并保持 LIKE/NULL/原事务快照语义。
use futures_util::TryStreamExt;
use sea_orm::entity::prelude::{
    DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EnumIter, PrimaryKeyTrait,
};
use sea_orm::{
    ActiveModelBehavior, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Statement,
};
use seata_db_mysql::MySqlDataSource;
use seata_orm_seaorm::SeaOrmMySqlConnection;
use std::path::Path;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "paging_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: u64,
    pub name: String,
    pub category: i32,
    pub note: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
fn raw(sql: &str) -> Statement {
    Statement::from_string(DbBackend::MySql, sql.to_owned())
}
/// 参数为正式源、独立观察者及证据目录；实际运行原生分页，不建立 ORM 业务池。
pub async fn run(source: &MySqlDataSource, admin: &mut sqlx::MySqlConnection, out: &Path) {
    sqlx::raw_sql("DROP TABLE IF EXISTS paging_items; CREATE TABLE paging_items(id BIGINT UNSIGNED PRIMARY KEY,name VARCHAR(64) COLLATE utf8mb4_bin NOT NULL,category INT NOT NULL,note VARCHAR(16) NULL) ENGINE=InnoDB; INSERT INTO paging_items VALUES(1,'alpha',1,'first'),(2,'alphabet',1,NULL),(3,'beta',2,'third'),(4,'中文甲',2,NULL),(5,'literal%value',3,NULL),(6,'literal_value',3,NULL)").execute(&mut *admin).await.unwrap();
    let values = (7..=3006)
        .map(|id| format!("({id},'item-{id}',4,NULL)"))
        .collect::<Vec<_>>()
        .join(",");
    sqlx::query(&format!("INSERT INTO paging_items VALUES {values}"))
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut local = source.begin().await.unwrap();
    let mut page_edges = Vec::new();
    let total;
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        let mut pages = Entity::find()
            .order_by_asc(Column::Id)
            .paginate(&connection, 1007);
        total = pages.num_items().await.unwrap();
        assert_eq!(total, 3006);
        assert_eq!(pages.num_pages().await.unwrap(), 3);
        let numbers = pages.num_items_and_pages().await.unwrap();
        assert_eq!(
            (numbers.number_of_items, numbers.number_of_pages),
            (3006, 3)
        );
        for expected in [(1, 1007), (1008, 2014), (2015, 3006)] {
            let page = pages.fetch_and_next().await.unwrap().unwrap();
            assert_eq!(
                (page.first().unwrap().id, page.last().unwrap().id),
                expected
            );
            page_edges.push(serde_json::json!([page.len(), expected.0, expected.1]));
        }
        assert!(pages.fetch_and_next().await.unwrap().is_none());
        assert_eq!(pages.cur_page(), 4);
        let first = pages.fetch_page(0).await.unwrap();
        assert_eq!(first.len(), 1007);
        assert!(pages.fetch_page(3).await.unwrap().is_empty());
        let mut stream = Entity::find()
            .order_by_asc(Column::Id)
            .paginate(&connection, 1007)
            .into_stream();
        let mut streamed = 0;
        while let Some(page) = stream.try_next().await.unwrap() {
            assert!(page.len() <= 1007);
            streamed += page.len();
        }
        assert_eq!(streamed, 3006);
        assert!(Entity::find()
            .paginate(&connection, 1025)
            .fetch_page(0)
            .await
            .is_err());
        assert!(Entity::find()
            .paginate(&connection, 1)
            .fetch_page(1_000_001)
            .await
            .is_err());
        assert_eq!(
            Entity::find()
                .filter(Column::Name.contains("alpha"))
                .count(&connection)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            Entity::find()
                .filter(Column::Name.starts_with("alpha"))
                .count(&connection)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            Entity::find()
                .filter(Column::Name.ends_with("bet"))
                .count(&connection)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            Entity::find()
                .filter(Column::Name.not_like("%alpha%"))
                .count(&connection)
                .await
                .unwrap(),
            3004
        );
        assert_eq!(
            Entity::find()
                .filter(Column::Name.like("中文_"))
                .count(&connection)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            Entity::find()
                .filter(Column::Note.not_like("%first%"))
                .count(&connection)
                .await
                .unwrap(),
            1
        );
        let empty = Entity::find()
            .filter(Column::Id.eq(0_u64))
            .paginate(&connection, 5);
        assert_eq!(empty.num_items().await.unwrap(), 0);
        assert_eq!(empty.num_pages().await.unwrap(), 0);
        assert!(empty.fetch().await.unwrap().is_empty());
        for (pattern, id) in [("literal!%value", 5), ("literal!_value", 6)] {
            let row = connection
                .query_one(Statement::from_sql_and_values(
                    DbBackend::MySql,
                    "SELECT id FROM paging_items WHERE name LIKE ? ESCAPE '!'",
                    [pattern.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get_by_index::<u64>(0).unwrap(), id);
        }
        let count=connection.query_one(Statement::from_sql_and_values(DbBackend::MySql,"SELECT COUNT(*) AS total FROM (SELECT id FROM paging_items WHERE name NOT LIKE ? LIMIT ? OFFSET ?) AS sub_query",["alpha%".into(),2_u64.into(),1_u64.into()])).await.unwrap().unwrap();
        assert_eq!(count.try_get::<i64>("", "total").unwrap(), 2);
        for text in [
            "SELECT COUNT(*) FROM (SELECT SLEEP(1) FROM paging_items) AS x",
            "SELECT COUNT(1) FROM paging_items",
            "SELECT SUM(id) FROM paging_items",
            "SELECT COUNT(*) OVER() FROM paging_items",
            "SELECT COUNT(*) FROM (SELECT id FROM paging_items) AS x WHERE x.id>1",
        ] {
            assert!(connection.query_one(raw(text)).await.is_err());
        }
        sqlx::query("INSERT INTO paging_items VALUES(4000,'external',5,NULL)")
            .execute(&mut *admin)
            .await
            .unwrap();
        assert_eq!(pages.num_items().await.unwrap(), 3006);
    }
    local.rollback().await.unwrap();
    let mut fresh = source.begin().await.unwrap();
    let fresh_total;
    {
        let connection = SeaOrmMySqlConnection::new(&mut fresh);
        fresh_total = Entity::find().count(&connection).await.unwrap();
        assert_eq!(fresh_total, 3007);
    }
    fresh.rollback().await.unwrap();
    sqlx::query("DELETE FROM paging_items WHERE id=4000")
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut local = source.begin().await.unwrap();
    {
        let connection = SeaOrmMySqlConnection::new(&mut local);
        Entity::insert(ActiveModel {
            id: Set(5000),
            name: Set("owned".into()),
            category: Set(5),
            note: Set(None),
        })
        .exec(&connection)
        .await
        .unwrap();
        assert_eq!(Entity::find().count(&connection).await.unwrap(), 3007);
        let external: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM paging_items")
            .fetch_one(&mut *admin)
            .await
            .unwrap();
        assert_eq!(external, 3006);
    }
    local.rollback().await.unwrap();
    std::fs::write(out.join("pagination-checks.json"),serde_json::to_vec_pretty(&serde_json::json!({"total":total,"pages":3,"streamed_items":3006,"page_edges":page_edges,"contains":2,"starts_with":2,"ends_with":1,"not_like":3004,"unicode_like":1,"null_not_like":1,"escaped_percent_id":5,"escaped_underscore_id":6,"empty_total":0,"derived_paged_count":2,"rr_after_external_insert":3006,"fresh_total":fresh_total,"own_uncommitted_count":3007,"outside_uncommitted_count":3006,"large_page_rejected":true,"large_offset_rejected":true,"unsafe_count_shapes_rejected":5})).unwrap()).unwrap();
}
