#[cfg(test)]
mod test {
    use rbs::value::map::ValueMap;
    use rbs::Value;
    use serde::{Deserialize, Serialize};
    use std::str::FromStr;

    fn make_map(k: &str, v: Value) -> Value {
        let mut map = ValueMap::new();
        map.insert(Value::String(k.to_string()), v);
        Value::Map(map)
    }

    fn make_array_map(values: Vec<(&str, Value)>) -> Value {
        let mut map = ValueMap::new();
        for (k, v) in values {
            map.insert(Value::String(k.to_string()), v);
        }
        Value::Map(map)
    }

    #[test]
    fn test_decode_value() {
        // [{k:value}] format
        let m = Value::Array(vec![make_map("1", Value::I64(1))]);
        let v: Value = rbatis::decode(m.clone()).unwrap();
        assert_eq!(v, m);
    }

    #[test]
    fn test_decode_value_one() {
        // [{k:value}] format
        let m = Value::Array(vec![make_map("1", Value::I64(1))]);
        let v: i64 = rbatis::decode(m).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_decode_one() {
        let date = rbdc::types::datetime::DateTime::now();
        // [{k:value}] format
        let date_value: Value = date.clone().into();
        let m = Value::Array(vec![make_map("1", date_value)]);
        let v: rbdc::types::datetime::DateTime = rbatis::decode(m).unwrap();
        assert_eq!(v.to_string(), date.to_string());
        println!("{}", v.offset());
    }

    #[test]
    fn test_decode_i32() {
        // [{k:value}] format
        let v: i32 = rbatis::decode(Value::Array(vec![make_map("a", Value::I64(1))])).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_decode_i64() {
        // [{k:value}] format
        let v: i64 = rbatis::decode(Value::Array(vec![make_map("a", Value::I64(1))])).unwrap();
        assert_eq!(v, 1i64);
    }

    #[test]
    fn test_decode_string() {
        // [{k:value}] format
        let v: String = rbatis::decode(Value::Array(vec![make_map(
            "a",
            Value::String("a".to_string()),
        )]))
        .unwrap();
        assert_eq!(v, "a");
    }

    #[test]
    fn test_decode_json_array() {
        // [{k:value}] format for multiple rows
        let v: serde_json::Value = rbatis::decode(Value::Array(vec![
            make_array_map(vec![("1", Value::I64(1)), ("2", Value::I64(2))]),
            make_array_map(vec![("1", Value::I64(1)), ("2", Value::I64(2))]),
        ]))
        .unwrap();
        assert_eq!(
            v,
            serde_json::from_str::<serde_json::Value>(r#"[{"1":1,"2":2},{"1":1,"2":2}]"#).unwrap()
        );
    }

    #[test]
    fn test_decode_rbdc_types() {
        use rbdc::types::*;
        let date = date::Date::from_str("2023-12-12").unwrap();
        let date_new: date::Date = rbs::from_value(rbs::value!(date.clone())).unwrap();
        assert_eq!(date, date_new);

        let datetime = datetime::DateTime::from_str("2023-12-12 12:12:12").unwrap();
        let datetime_new: datetime::DateTime =
            rbs::from_value(rbs::value!(datetime.clone())).unwrap();
        assert_eq!(datetime, datetime_new);
    }

    #[test]
    fn test_decode_empty_array() {
        // Empty array decode in [{k:value}] format.
        let empty_array = Value::Array(vec![]);
        let result: Result<Option<i32>, _> = rbatis::decode(empty_array);
        assert!(result.is_err() || result.unwrap().is_none());
    }

    #[test]
    fn test_decode_multiple_rows_to_single_type() {
        // Multiple rows can unwrap to i32; returns the first successful value.
        let data = Value::Array(vec![
            make_map("a", Value::I64(1)),
            make_map("a", Value::I64(2)),
        ]);
        let result: i32 = rbatis::decode(data).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_decode_f32() {
        // [{k:value}] format
        let v: f32 = rbatis::decode(Value::Array(vec![make_map("a", Value::F64(1.0))])).unwrap();
        assert_eq!(v, 1.0);
    }

    //https://github.com/rbatis/rbatis/issues/498
    #[test]
    fn test_decode_type_fail_498() {
        // CSV format with float that can't be parsed as i64
        let m = Value::Array(vec![
            Value::Array(vec![Value::String("aa".to_string())]),
            Value::Array(vec![Value::F64(0.0)]),
        ]);
        let v = rbatis::decode::<i64>(m).err().unwrap();
        assert_eq!(
            v.to_string(),
            "invalid type: floating point `0.0`, expected i64"
        );
    }

    #[test]
    fn test_decode_f64() {
        // [{k:value}] format
        let v: f64 = rbatis::decode(Value::Array(vec![make_map("a", Value::F64(1.0))])).unwrap();
        assert_eq!(v, 1.0);
    }

    #[test]
    fn test_decode_u32() {
        // [{k:value}] format
        let v: u32 = rbatis::decode(Value::Array(vec![make_map("a", Value::U64(1))])).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_decode_u64() {
        // [{k:value}] format
        let v: u64 = rbatis::decode(Value::Array(vec![make_map("a", Value::U64(1))])).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_decode_bool() {
        // [{k:value}] format
        let v: bool = rbatis::decode(Value::Array(vec![make_map("a", Value::Bool(true))])).unwrap();
        assert!(v);
    }

    #[test]
    fn test_decode_option_types() {
        // [{k:value}] format

        // Option<i32>
        let v1: Option<i32> =
            rbatis::decode(Value::Array(vec![make_map("a", Value::I32(1))])).unwrap();
        assert_eq!(v1, Some(1));

        // Option<String>
        let v2: Option<String> = rbatis::decode(Value::Array(vec![make_map(
            "a",
            Value::String("test".to_string()),
        )]))
        .unwrap();
        assert_eq!(v2, Some("test".to_string()));

        // Null decodes to None.
        let v3: Option<i32> =
            rbatis::decode(Value::Array(vec![make_map("a", Value::Null)])).unwrap();
        assert_eq!(v3, None);
    }

    #[test]
    fn test_decode_vec() {
        // Decode Vec<T> from multiple rows in [{k:value}] format.
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        pub struct Item {
            pub id: i32,
            pub name: String,
        }

        // [{k:value}] format
        let value = Value::Array(vec![
            make_array_map(vec![
                ("id", Value::I32(1)),
                ("name", Value::String("test1".to_string())),
            ]),
            make_array_map(vec![
                ("id", Value::I32(2)),
                ("name", Value::String("test2".to_string())),
            ]),
        ]);

        let result: Vec<Item> = rbatis::decode(value).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].name, "test1");
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].name, "test2");
    }

    #[test]
    fn test_decode_vec_type_fail() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        pub struct Item {
            pub id: i32,
            pub name: String,
        }

        let value = Value::Array(vec![make_array_map(vec![
            ("id", Value::String("bad".to_string())),
            ("name", Value::String("test".to_string())),
        ])]);

        let err = rbatis::decode::<Vec<Item>>(value).err().unwrap();
        assert_eq!(
            err.to_string(),
            "invalid type: string \"bad\", expected i32, key = `id`"
        );
    }

    #[test]
    fn test_decode_not_array() {
        // Non-array values are rejected.
        let value = Value::I32(1);
        let result = rbatis::decode::<i32>(value);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "decode error: expected array value"
        );
    }

    #[test]
    fn test_is_debug_mode() {
        // Test is_debug_mode.
        let _debug_mode = rbatis::decode::is_debug_mode();
        // The exact value depends on compile mode and enabled features.
    }
}
