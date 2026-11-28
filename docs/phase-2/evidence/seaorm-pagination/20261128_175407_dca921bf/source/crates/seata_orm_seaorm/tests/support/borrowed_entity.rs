//! 原生现场的真实 SeaORM 实体及 DML 构建，不内插参数。
use sea_orm::entity::prelude::{
    BigDecimal, DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EnumIter, Json,
    PrimaryKeyTrait,
};
use sea_orm::{ActiveModelBehavior, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::{ActiveModelTrait, IntoActiveModel, ModelTrait, QueryOrder};
use seata_orm_seaorm::{SeaOrmMySqlConnection, SeaOrmMySqlStatement};
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orm_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub v: Option<i32>,
    pub note: Option<String>,
    pub amount: BigDecimal,
    pub bytes: Vec<u8>,
    pub payload: Json,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

pub fn plan(id: &str) -> SeaOrmMySqlStatement {
    use sea_orm::sea_query::Expr;
    let amount = BigDecimal::from_str("9007199254740993.1250").unwrap();
    match id {
        "entity_insert" => SeaOrmMySqlStatement::from_query(&Entity::insert(ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            v: Set(Some(25)),
            note: Set(Some("new ?".into())),
            amount: Set(amount),
            bytes: Set(vec![0, 255, 0, 16]),
            payload: Set(serde_json::json!({"n":9007199254740993_i64,"s":"中文"})),
        }))
        .unwrap(),
        "entity_update" => SeaOrmMySqlStatement::from_query(
            &Entity::update_many()
                .col_expr(Column::V, Expr::value(15))
                .col_expr(Column::Note, Expr::value(Option::<String>::None))
                .col_expr(Column::Amount, Expr::value(amount))
                .col_expr(Column::Bytes, Expr::value(vec![0_u8, 255]))
                .col_expr(Column::Payload, Expr::value(serde_json::json!({"m":2})))
                .filter(Column::Id.eq(1)),
        )
        .unwrap(),
        "entity_delete" => {
            SeaOrmMySqlStatement::from_query(&Entity::delete_many().filter(Column::Id.eq(2)))
                .unwrap()
        }
        _ => panic!("unknown fixture case"),
    }
}

pub async fn apply(id: &str, connection: &SeaOrmMySqlConnection<'_>) -> (u64, serde_json::Value) {
    let amount = BigDecimal::from_str("9007199254740993.1250").unwrap();
    let (affected, returned) = match id {
        "entity_insert" => {
            let model = Entity::insert(ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                v: Set(Some(25)),
                note: Set(Some("new ?".into())),
                amount: Set(amount),
                bytes: Set(vec![0, 255, 0, 16]),
                payload: Set(serde_json::json!({"n":9007199254740993_i64,"s":"中文"})),
            })
            .exec_with_returning(connection)
            .await
            .unwrap();
            assert_eq!(model.id, 101);
            assert_eq!(model.amount.as_bigint_and_exponent().1, 4);
            (
                1,
                serde_json::json!({"generated_id":model.id,"note":model.note,"amount":model.amount.to_string(),"payload":model.payload}),
            )
        }
        "entity_update" => {
            let model = Entity::find_by_id(1_u64)
                .one(connection)
                .await
                .unwrap()
                .unwrap();
            let mut active = model.into_active_model();
            active.v = Set(Some(15));
            active.note = Set(None);
            active.amount = Set(amount);
            active.bytes = Set(vec![0, 255]);
            active.payload = Set(serde_json::json!({"m":2}));
            let model = active.update(connection).await.unwrap();
            assert_eq!(model.note, None);
            assert_eq!(model.amount.as_bigint_and_exponent().1, 4);
            (
                1,
                serde_json::json!({"updated_id":model.id,"v":model.v,"note":model.note,"amount":model.amount.to_string()}),
            )
        }
        "entity_delete" => {
            let model = Entity::find_by_id(2_u64)
                .one(connection)
                .await
                .unwrap()
                .unwrap();
            let result = model.delete(connection).await.unwrap();
            assert!(Entity::find_by_id(2_u64)
                .one(connection)
                .await
                .unwrap()
                .is_none());
            (
                result.rows_affected,
                serde_json::json!({"deleted_id":2,"read_after_delete":null}),
            )
        }
        _ => panic!("unknown borrowed entity case"),
    };
    let models = Entity::find()
        .order_by_asc(Column::Id)
        .all(connection)
        .await
        .unwrap();
    assert!(!models.is_empty());
    (
        affected,
        serde_json::json!({"model":returned,"read_own_ids":models.iter().map(|m|m.id).collect::<Vec<_>>()}),
    )
}
