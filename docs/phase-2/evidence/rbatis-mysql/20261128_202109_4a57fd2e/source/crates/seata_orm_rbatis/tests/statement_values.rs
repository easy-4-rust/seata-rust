//! 原生 RBS 参数和正式 DML 准入的精确边界。
#![allow(clippy::unwrap_used)]
use rbs::Value;
use seata_datasource::FieldValue;
use seata_orm_rbatis::RbatisMySqlStatement;
#[test]
fn exact_values_are_bound_without_sql_interpolation() {
    let decimal: rbatis::rbdc::Decimal = "9007199254740993.1250".parse().unwrap();
    let plan = RbatisMySqlStatement::from_sql(
        "INSERT INTO orm_items(id,amount,note,bytes,payload) VALUES(?,?,?,?,?)",
        vec![
            Value::U64(u64::MAX),
            rbs::to_value(decimal).unwrap(),
            Value::Null,
            Value::Binary(vec![0, 255]),
            rbs::to_value(rbatis::rbdc::Json(
                "{\"n\":9007199254740993,\"s\":\"中文\"}".into(),
            ))
            .unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(plan.parameters()[0], FieldValue::Unsigned(u64::MAX));
    assert_eq!(plan.parameters()[2], FieldValue::Null);
    assert_eq!(plan.parameters()[3], FieldValue::Binary(vec![0, 255]));
    assert!(!plan.recognizer().sql().contains("9007199254740993"));
    assert!(!format!("{plan:?}").contains("9007199254740993"));
}
#[test]
fn control_sql_floats_arity_extensions_and_budget_are_rejected() {
    for sql in [
        "COMMIT",
        "ROLLBACK",
        "BEGIN",
        "SET @x=1",
        "DELETE FROM orm_items; COMMIT",
    ] {
        assert!(RbatisMySqlStatement::from_sql(sql, vec![]).is_err());
    }
    for value in [
        Value::F64(0.1),
        Value::Array(vec![]),
        Value::Ext("Unknown", Box::new(Value::String("secret".into()))),
        Value::String("x".repeat(1_048_577)),
    ] {
        assert!(
            RbatisMySqlStatement::from_sql("UPDATE orm_items SET note=?", vec![value]).is_err()
        );
    }
    assert!(RbatisMySqlStatement::from_sql("DELETE FROM orm_items WHERE id=?", vec![]).is_err());
}
#[test]
fn deep_container_rejection_is_iterative_on_all_input_paths() {
    for sql in ["COMMIT", "UPDATE orm_items SET note=?"] {
        let mut value = Value::Null;
        for _ in 0..40_000 {
            value = Value::Ext("bad", Box::new(value));
        }
        assert!(RbatisMySqlStatement::from_sql(sql, vec![value]).is_err());
    }
}
#[test]
fn temporal_extensions_keep_original_units_and_microseconds() {
    for (name, text) in [
        ("Date", "2024-02-29"),
        ("Time", "-12:34:56.123456"),
        ("DateTime", "2024-02-29 12:34:56.123456"),
    ] {
        let plan = RbatisMySqlStatement::from_sql(
            "UPDATE orm_items SET note=?",
            vec![Value::Ext(name, Box::new(Value::String(text.into())))],
        )
        .unwrap();
        assert_eq!(plan.parameters().len(), 1);
    }
    let plan = RbatisMySqlStatement::from_sql(
        "UPDATE orm_items SET note=?",
        vec![Value::Ext(
            "Timestamp",
            Box::new(Value::I64(1_700_000_000_123)),
        )],
    )
    .unwrap();
    match plan.parameters()[0] {
        FieldValue::Timestamp(value) => {
            assert_eq!(value.epoch_seconds(), 1_700_000_000);
            assert_eq!(value.nanosecond(), 123_000_000);
        }
        _ => panic!("wrong type"),
    }
}

#[test]
fn native_rbdc_temporal_and_binary_serialization_is_admitted() {
    let date: rbatis::rbdc::Date = "2024-02-29".parse().unwrap();
    let time: rbatis::rbdc::Time = "12:34:56.123456".parse().unwrap();
    let datetime: rbatis::rbdc::DateTime = "2024-02-29 12:34:56.123456".parse().unwrap();
    let values = vec![
        rbs::to_value(date).unwrap(),
        rbs::to_value(time).unwrap(),
        rbs::to_value(datetime).unwrap(),
        rbs::to_value(rbatis::rbdc::Bytes::from(vec![0, 255])).unwrap(),
    ];
    let plan =
        RbatisMySqlStatement::from_sql("INSERT INTO orm_items(a,b,c,d) VALUES(?,?,?,?)", values)
            .unwrap();
    assert!(matches!(plan.parameters()[0], FieldValue::LocalDate(_)));
    assert!(matches!(plan.parameters()[1], FieldValue::InternalTime(_)));
    assert!(matches!(plan.parameters()[2], FieldValue::LocalDateTime(_)));
    assert_eq!(plan.parameters()[3], FieldValue::Binary(vec![0, 255]));
}
