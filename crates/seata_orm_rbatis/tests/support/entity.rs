//! 现场使用真实 rbatis CRUD 与原参数序列。
use futures_core::future::BoxFuture;
use rbatis::{
    executor::{Executor, RBatisRef},
    rbdc::{db::ExecResult, Bytes, Decimal},
    Error, RBatis,
};
use rbs::Value;
use seata_orm_rbatis::{RbatisMySqlExecutor, RbatisMySqlStatement};
use std::sync::Mutex;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OrmItem {
    pub id: Option<u64>,
    pub v: Option<i32>,
    pub note: Option<String>,
    pub amount: Decimal,
    pub bytes: Bytes,
    pub payload: String,
}
rbatis::crud!(OrmItem {}, "orm_items");
fn model(id: &str) -> OrmItem {
    OrmItem {
        id: if id == "entity_insert" { None } else { Some(1) },
        v: Some(if id == "entity_insert" { 25 } else { 15 }),
        note: if id == "entity_insert" {
            Some("new ?".into())
        } else {
            None
        },
        amount: "9007199254740993.1250".parse().unwrap(),
        bytes: Bytes::from(if id == "entity_insert" {
            vec![0, 255, 0, 16]
        } else {
            vec![0, 255]
        }),
        payload: if id == "entity_insert" {
            "{\"n\":9007199254740993,\"s\":\"中文\"}".into()
        } else {
            "{\"m\":2}".into()
        },
    }
}
async fn write(id: &str, executor: &dyn Executor) -> ExecResult {
    match id {
        "entity_insert" => OrmItem::insert(executor, &model(id)).await.unwrap(),
        "entity_update" => OrmItem::update_by_map(
            executor,
            &model(id),
            rbs::value! {"id":1,"column":["v","note","amount","bytes","payload"]},
        )
        .await
        .unwrap(),
        "entity_delete" => OrmItem::delete_by_map(executor, rbs::value! {"id":2})
            .await
            .unwrap(),
        _ => panic!("unknown fixture"),
    }
}
struct Capture {
    engine: RBatis,
    plans: Mutex<Vec<RbatisMySqlStatement>>,
}
impl RBatisRef for Capture {
    fn rb_ref(&self) -> &RBatis {
        &self.engine
    }
    fn driver_type(&self) -> Result<&str, Error> {
        Ok("mysql")
    }
}
impl Executor for Capture {
    fn id(&self) -> i64 {
        1
    }
    fn exec(&self, sql: &str, args: Vec<Value>) -> BoxFuture<'_, Result<ExecResult, Error>> {
        let plan = RbatisMySqlStatement::from_sql(sql, args).unwrap();
        self.plans.lock().unwrap().push(plan);
        Box::pin(async {
            Ok(ExecResult {
                rows_affected: 1,
                last_insert_id: Value::U64(101),
            })
        })
    }
    fn query(&self, _: &str, _: Vec<Value>) -> BoxFuture<'_, Result<Value, Error>> {
        Box::pin(async {
            Err(Error::from(
                "classification fixture does not query a database",
            ))
        })
    }
}
pub async fn plan(id: &str) -> RbatisMySqlStatement {
    let capture = Capture {
        engine: RBatis::default(),
        plans: Mutex::new(Vec::new()),
    };
    write(id, &capture).await;
    let mut plans = capture.plans.into_inner().unwrap();
    assert_eq!(plans.len(), 1);
    plans.remove(0)
}
pub async fn apply(id: &str, executor: &RbatisMySqlExecutor) -> (u64, serde_json::Value) {
    let result = write(id, executor).await;
    if id == "entity_insert" {
        assert_eq!(result.last_insert_id, Value::U64(101));
    }
    let models = OrmItem::select_by_map(executor, Value::Map(rbs::value::map::ValueMap::new()))
        .await
        .unwrap();
    let count = rbatis::decode::decode::<u64>(
        executor
            .query("SELECT COUNT(*) AS native_count FROM orm_items", vec![])
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(count, models.len() as u64);
    for model in &models {
        let _: serde_json::Value = serde_json::from_str(&model.payload).unwrap();
    }
    if id != "entity_delete" {
        let changed = models
            .iter()
            .find(|m| m.id == Some(if id == "entity_insert" { 101 } else { 1 }))
            .unwrap();
        assert_eq!(changed.amount.to_string(), "9007199254740993.1250");
        assert_eq!(changed.bytes.0, model(id).bytes.0);
        assert_eq!(changed.note, model(id).note);
    }
    (
        result.rows_affected,
        serde_json::json!({"read_own_ids":models.iter().map(|m|m.id.unwrap()).collect::<Vec<_>>(),"native_count":count,"receipt_last_insert_id":result.last_insert_id.as_u64()}),
    )
}
