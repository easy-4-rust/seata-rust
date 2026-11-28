//! 原生现场的真实 SeaORM 实体及 DML 构建，不内插参数。
use sea_orm::entity::prelude::{
    BigDecimal, DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EnumIter, Json,
    PrimaryKeyTrait,
};
use sea_orm::{ActiveModelBehavior, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use seata_orm_seaorm::SeaOrmMySqlStatement;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "orm_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
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
            id: Set(11),
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
