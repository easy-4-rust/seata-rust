//! 真正 SeaORM 实体构建与精确绑定边界。
#![allow(clippy::unwrap_used)]
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DbBackend, EntityTrait, QueryFilter, Statement, Value,
};
use seata_datasource::FieldValue;
use seata_orm_seaorm::SeaOrmMySqlStatement;

mod item {
    use sea_orm::entity::prelude::{
        DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EnumIter, PrimaryKeyTrait,
    };
    use sea_orm::ActiveModelBehavior;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "orm_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub note: Option<String>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[test]
fn entity_insert_and_delete_keep_original_bindings_without_interpolation() {
    let query = item::Entity::insert(item::ActiveModel {
        id: Set(i64::MAX),
        note: Set(Some("private-bound-value".into())),
    });
    let plan = SeaOrmMySqlStatement::from_query(&query).unwrap();
    assert_eq!(plan.recognizer().table_name(), "orm_items");
    assert_eq!(
        plan.parameters(),
        &[
            FieldValue::Signed(i64::MAX),
            FieldValue::Text("private-bound-value".into())
        ]
    );
    assert!(!plan.recognizer().sql().contains("private-bound-value"));
    assert!(!format!("{plan:?}").contains("private-bound-value"));
    let deletion = item::Entity::delete_many().filter(item::Column::Id.eq(3_i64));
    let plan = SeaOrmMySqlStatement::from_query(&deletion).unwrap();
    assert_eq!(plan.parameters(), &[FieldValue::Signed(3)]);
}

#[test]
fn null_json_null_unsigned_and_binary_are_distinct() {
    let plan = SeaOrmMySqlStatement::from_statement(Statement::from_sql_and_values(
        DbBackend::MySql,
        "INSERT INTO orm_items(id,note,a,b) VALUES(?,?,?,?)",
        [
            Value::BigUnsigned(Some(u64::MAX)),
            Value::Json(Some(Box::new(serde_json::Value::Null))),
            Value::String(None),
            Value::Bytes(Some(Box::new(vec![0, 255, 0]))),
        ],
    ))
    .unwrap();
    assert_eq!(
        plan.parameters(),
        &[
            FieldValue::Unsigned(u64::MAX),
            FieldValue::Text("null".into()),
            FieldValue::Null,
            FieldValue::Binary(vec![0, 255, 0])
        ]
    );
}

#[test]
fn unsupported_dialect_shape_type_arity_and_budget_reject_before_execution() {
    for statement in [
        Statement::from_string(DbBackend::Postgres, "DELETE FROM orm_items"),
        Statement::from_string(DbBackend::MySql, "COMMIT"),
        Statement::from_sql_and_values(DbBackend::MySql, "DELETE FROM orm_items WHERE id=?", []),
        Statement::from_sql_and_values(
            DbBackend::MySql,
            "UPDATE orm_items SET note=?",
            [Value::Double(Some(0.1))],
        ),
        Statement::from_sql_and_values(
            DbBackend::MySql,
            "UPDATE orm_items SET note=?",
            [Value::String(Some(Box::new("x".repeat(1_048_577))))],
        ),
    ] {
        assert!(SeaOrmMySqlStatement::from_statement(statement).is_err());
    }
}

#[test]
fn decimals_preserve_scale_without_floating_or_unbounded_exponent_expansion() {
    use sea_orm::prelude::BigDecimal;
    use std::str::FromStr;
    let statement = |s: &str| {
        Statement::from_sql_and_values(
            DbBackend::MySql,
            "UPDATE orm_items SET note=?",
            [Value::BigDecimal(Some(Box::new(
                BigDecimal::from_str(s).unwrap(),
            )))],
        )
    };
    let value = SeaOrmMySqlStatement::from_statement(statement("9007199254740993.1250")).unwrap();
    assert!(
        matches!(&value.parameters()[0],FieldValue::Decimal(v) if v.as_str()=="9007199254740993.1250")
    );
    for s in [
        "1e1000000000",
        "1e-1000000000",
        "1.0000000000000000000000000000001",
    ] {
        assert!(SeaOrmMySqlStatement::from_statement(statement(s)).is_err());
    }
}

#[test]
fn temporal_bindings_keep_calendar_instant_and_reject_submicrosecond_loss() {
    use sea_orm::prelude::{ChronoDateTime, DateTimeWithTimeZone};
    let local =
        ChronoDateTime::parse_from_str("2026-11-28 01:02:03.123456", "%Y-%m-%d %H:%M:%S%.f")
            .unwrap();
    let instant =
        DateTimeWithTimeZone::parse_from_rfc3339("2026-11-28T09:02:03.123456+08:00").unwrap();
    let plan = SeaOrmMySqlStatement::from_statement(Statement::from_sql_and_values(
        DbBackend::MySql,
        "UPDATE orm_items SET a=?,b=?",
        [
            Value::ChronoDateTime(Some(Box::new(local))),
            Value::ChronoDateTimeWithTimeZone(Some(Box::new(instant))),
        ],
    ))
    .unwrap();
    assert!(
        matches!(&plan.parameters()[0],FieldValue::LocalDateTime(v) if v.to_sql_string()=="2026-11-28 01:02:03.123456")
    );
    assert!(
        matches!(&plan.parameters()[1],FieldValue::Timestamp(v) if v.to_sql_utc_string()=="2026-11-28 01:02:03.123456")
    );
    let raw =
        ChronoDateTime::parse_from_str("2026-11-28 01:02:03.123456789", "%Y-%m-%d %H:%M:%S%.f")
            .unwrap();
    assert!(
        SeaOrmMySqlStatement::from_statement(Statement::from_sql_and_values(
            DbBackend::MySql,
            "UPDATE orm_items SET a=?",
            [Value::ChronoDateTime(Some(Box::new(raw)))]
        ))
        .is_err()
    );
}

#[test]
fn oversized_and_deep_json_are_rejected_without_recursive_stack_failure() {
    let mut value = serde_json::Value::Null;
    for _ in 0..10000 {
        value = serde_json::Value::Array(vec![value]);
    }
    for value in [
        value,
        serde_json::Value::Array(vec![serde_json::Value::Null; 8193]),
        serde_json::Value::String("x".repeat(1_048_577)),
    ] {
        assert!(
            SeaOrmMySqlStatement::from_statement(Statement::from_sql_and_values(
                DbBackend::MySql,
                "UPDATE orm_items SET a=?",
                [Value::Json(Some(Box::new(value)))]
            ))
            .is_err()
        );
    }
}
