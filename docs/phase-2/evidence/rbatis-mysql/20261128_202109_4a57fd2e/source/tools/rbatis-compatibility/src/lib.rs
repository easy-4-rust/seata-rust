#![allow(clippy::unwrap_used)]
//! 无数据库的上游发布版兼容探针；不构成 AT 原子性或恢复验收。
#[cfg(test)]
mod tests {
    use futures_core::future::BoxFuture;
    use rbatis::executor::{Executor, RBatisRef};
    use rbatis::plugin::{
        intercept::intercept_page::PageIntercept,
        page::{IPageRequest, PageRequest},
    };
    use rbatis::rbdc::db::{ConnectOptions, Connection, ExecResult};
    use rbatis::{Error, RBatis};
    use rbs::Value;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct CaptureExecutor {
        engine: RBatis,
        calls: Mutex<Vec<(String, Vec<Value>)>>,
    }
    impl CaptureExecutor {
        fn new() -> Self {
            Self {
                engine: RBatis::default(),
                calls: Mutex::new(Vec::new()),
            }
        }
    }
    impl RBatisRef for CaptureExecutor {
        fn rb_ref(&self) -> &RBatis {
            &self.engine
        }
        fn driver_type(&self) -> Result<&str, Error> {
            Ok("mysql")
        }
    }
    impl Executor for CaptureExecutor {
        fn id(&self) -> i64 {
            41
        }
        fn exec(&self, sql: &str, args: Vec<Value>) -> BoxFuture<'_, Result<ExecResult, Error>> {
            self.calls.lock().unwrap().push((sql.into(), args));
            Box::pin(async {
                Ok(ExecResult {
                    rows_affected: 1,
                    last_insert_id: Value::U64(9_223_372_036_854_775_811),
                })
            })
        }
        fn query(&self, sql: &str, args: Vec<Value>) -> BoxFuture<'_, Result<Value, Error>> {
            self.calls.lock().unwrap().push((sql.into(), args));
            Box::pin(async { Ok(rbs::to_value(vec![item()]).unwrap()) })
        }
    }
    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct OrmItem {
        id: u64,
        note: Option<String>,
        amount: rbatis::rbdc::Decimal,
    }
    rbatis::crud!(OrmItem {}, "orm_items");
    fn item() -> OrmItem {
        OrmItem {
            id: 9_223_372_036_854_775_811,
            note: Some("private-bound-中文-?".into()),
            amount: "9007199254740993.1250".parse().unwrap(),
        }
    }

    #[rbatis::py_sql("`update orm_items set amount = #{amount}`\nif note != null:\n `,note = #{note}`\n` where id = #{id}`")]
    async fn dynamic_update(
        executor: &dyn Executor,
        amount: &rbatis::rbdc::Decimal,
        note: Option<&str>,
        id: u64,
    ) -> Result<ExecResult, Error> {
        rbatis::impled!()
    }

    #[tokio::test]
    async fn native_crud_dispatches_through_supplied_executor_and_preserves_receipt() {
        let executor = CaptureExecutor::new();
        assert!(executor.rb_ref().get_pool().is_err());
        assert!(executor.rb_ref().intercepts.is_empty());
        let row = item();
        let result = OrmItem::insert(&executor, &row).await.unwrap();
        assert_eq!(result.rows_affected, 1);
        assert_eq!(result.last_insert_id, Value::U64(row.id));
        let found = OrmItem::select_by_map(&executor, rbs::value! {"id":row.id})
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, row.id);
        assert_eq!(found[0].amount.to_string(), "9007199254740993.1250");
        OrmItem::update_by_map(&executor, &row, rbs::value! {"id":row.id})
            .await
            .unwrap();
        OrmItem::delete_by_map(&executor, rbs::value! {"id":row.id})
            .await
            .unwrap();
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        for (sql, _) in calls.iter() {
            assert!(!sql.contains("private-bound"));
            assert!(!sql.contains("9007199254740993"));
            assert!(!sql.contains("9223372036854775811"));
        }
        assert!(calls[0].1.contains(&Value::Ext(
            "Decimal",
            Box::new(Value::String("9007199254740993.1250".into()))
        )));
    }
    #[tokio::test]
    async fn dynamic_sql_preserves_conditional_parameter_order() {
        let executor = CaptureExecutor::new();
        let row = item();
        dynamic_update(&executor, &row.amount, row.note.as_deref(), row.id)
            .await
            .unwrap();
        dynamic_update(&executor, &row.amount, None, row.id)
            .await
            .unwrap();
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].1,
            vec![
                rbs::to_value(&row.amount).unwrap(),
                Value::String(row.note.unwrap()),
                Value::U64(row.id)
            ]
        );
        assert_eq!(
            calls[1].1,
            vec![rbs::to_value(&row.amount).unwrap(), Value::U64(row.id)]
        );
        assert!(calls[0].0.contains("note"));
        assert!(!calls[1].0.contains("note"));
    }
    #[test]
    fn native_scalar_domains_round_trip_without_float_conversion() {
        let row = item();
        let original = rbs::to_value(&row.amount).unwrap();
        assert_eq!(
            original,
            Value::Ext(
                "Decimal",
                Box::new(Value::String("9007199254740993.1250".into()))
            )
        );
        let restored: rbatis::rbdc::Decimal = rbs::from_value(original).unwrap();
        assert_eq!(restored.to_string(), "9007199254740993.1250");
        let text = "{\"n\":9007199254740993,\"s\":\"中文\"}";
        let encoded = rbs::to_value(rbatis::rbdc::Json(text.into())).unwrap();
        assert_eq!(
            encoded,
            Value::Ext("Json", Box::new(Value::String(text.into())))
        );
        let decoded: String = rbs::from_value(encoded).unwrap();
        assert_eq!(decoded, text);
        assert_eq!(
            rbs::from_value::<u64>(Value::U64(u64::MAX)).unwrap(),
            u64::MAX
        );
        assert_ne!(
            Value::Null,
            Value::Ext("Json", Box::new(Value::String("null".into())))
        );
    }
    #[rbatis::html_sql("<mapper><update id=\"html_update\">update orm_items set note = #{note} where id = #{id}</update></mapper>")]
    async fn html_update(
        executor: &dyn Executor,
        note: &str,
        id: u64,
    ) -> Result<ExecResult, Error> {
        rbatis::impled!()
    }
    #[rbatis::sql("update orm_items set note = ? where id = ?")]
    async fn static_update(
        executor: &dyn Executor,
        note: &str,
        id: u64,
    ) -> Result<ExecResult, Error> {
        rbatis::impled!()
    }
    #[tokio::test]
    async fn html_macro_dispatches_without_pool_and_binds_original_values() {
        let executor = CaptureExecutor::new();
        html_update(&executor, "html-private", u64::MAX)
            .await
            .unwrap();
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].0.contains("html-private"));
        assert_eq!(
            calls[0].1,
            vec![Value::String("html-private".into()), Value::U64(u64::MAX)]
        );
    }
    #[tokio::test]
    async fn static_macro_dispatches_without_pool_and_binds_original_values() {
        let executor = CaptureExecutor::new();
        static_update(&executor, "sql-private", u64::MAX)
            .await
            .unwrap();
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].0.contains("sql-private"));
        assert_eq!(
            calls[0].1,
            vec![Value::String("sql-private".into()), Value::U64(u64::MAX)]
        );
    }
    #[derive(Debug)]
    struct TestOptions {
        uri: String,
    }
    impl ConnectOptions for TestOptions {
        fn connect(&self) -> BoxFuture<'_, Result<Box<dyn Connection>, Error>> {
            Box::pin(async { Err(Error::from("probe owns no database connection")) })
        }
        fn set_uri(&mut self, uri: &str) -> Result<(), Error> {
            self.uri = uri.into();
            Ok(())
        }
    }
    #[derive(Debug)]
    struct OtherOptions;
    impl ConnectOptions for OtherOptions {
        fn connect(&self) -> BoxFuture<'_, Result<Box<dyn Connection>, Error>> {
            Box::pin(async { Err(Error::from("probe owns no database connection")) })
        }
        fn set_uri(&mut self, _: &str) -> Result<(), Error> {
            Err(Error::from("probe option has no URI"))
        }
    }
    #[test]
    fn option_downcast_preserves_identity_mutation_and_type_rejection() {
        let mut original = TestOptions {
            uri: "before".into(),
        };
        let erased: &mut dyn ConnectOptions = &mut original;
        assert_eq!(erased.downcast_ref::<TestOptions>().unwrap().uri, "before");
        assert!(erased.downcast_ref::<OtherOptions>().is_none());
        erased.downcast_ref_mut::<TestOptions>().unwrap().uri = "after".into();
        assert_eq!(original.uri, "after");
    }
    #[test]
    fn interceptor_lookup_preserves_concrete_type() {
        let engine = RBatis::default();
        engine.intercepts.push(Arc::new(PageIntercept::new()));
        assert!(engine.get_intercept::<PageIntercept>().is_some());
        assert!(engine
            .get_intercept::<rbatis::plugin::intercept::intercept_log::LogInterceptor>()
            .is_none());
    }
    #[test]
    fn page_count_preserves_zero_exact_remainder_and_unsigned_bounds() {
        for (total, size, expected) in [
            (0, 10, 0),
            (20, 10, 2),
            (21, 10, 3),
            (u64::MAX, 1, u64::MAX),
            (u64::MAX, 2, (u64::MAX / 2) + 1),
        ] {
            assert_eq!(PageRequest::new(1, size).set_total(total).pages(), expected);
        }
        let mut request = PageRequest::new(1, 1);
        request.page_size = 0;
        assert_eq!(request.pages(), 0);
    }
}

#[cfg(test)]
mod cache_checks;
