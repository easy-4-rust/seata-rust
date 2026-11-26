//! 读取由固定 H2 原用例导出的 JDBC 镜像。
#![allow(clippy::unwrap_used)]
use seata_at::{
    FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue, FastjsonUndoRecords, SqlType,
};

pub(super) fn from_h2(method: &str) -> (String, i64, Vec<FastjsonSqlUndoLog>) {
    let path = std::env::var_os("SEATA_FASTJSON_H2_SOURCE").unwrap();
    let input = std::fs::read_to_string(path).unwrap();
    let mut branch = None;
    let mut logs = Vec::new();
    for line in input.lines() {
        let parts: Vec<_> = line.split('\t').collect();
        if parts.get(1) != Some(&method) {
            continue;
        }
        match parts.as_slice() {
            ["BRANCH", _, xid, branch_id] => {
                assert!(branch
                    .replace((xid.to_string(), branch_id.parse().unwrap()))
                    .is_none());
            }
            ["LOG", _, "UPDATE", table] => {
                logs.push((table.to_string(), Vec::<FastjsonUndoRecords>::new()));
            }
            ["IMAGE", _, table] => {
                logs.last_mut().unwrap().1.push(FastjsonUndoRecords::new(
                    Some(table.to_string()),
                    Vec::new(),
                ));
            }
            ["ROW", _, id_name, id_jdbc, id_key, id, name_name, name_jdbc, name_key, name] => {
                let field = |name: &str, jdbc: &str, key: &str, value| {
                    assert!(matches!(key, "PRIMARY_KEY" | "NULL"));
                    FastjsonUndoField::new(
                        name.into(),
                        jdbc.parse().unwrap(),
                        key == "PRIMARY_KEY",
                        value,
                    )
                };
                logs.last_mut()
                    .unwrap()
                    .1
                    .last_mut()
                    .unwrap()
                    .push_row(vec![
                        field(
                            id_name,
                            id_jdbc,
                            id_key,
                            FastjsonUndoFieldValue::Integer(id.parse().unwrap()),
                        ),
                        field(
                            name_name,
                            name_jdbc,
                            name_key,
                            FastjsonUndoFieldValue::Text(name.to_string()),
                        ),
                    ]);
            }
            _ => panic!("invalid H2 trace record"),
        }
    }
    let (xid, branch_id) = branch.unwrap();
    assert_eq!(logs.len(), 2);
    let logs = logs
        .into_iter()
        .map(|(table, images)| {
            let [before, after]: [FastjsonUndoRecords; 2] = images.try_into().ok().unwrap();
            assert_eq!(before.rows().len(), 2);
            assert_eq!(after.rows().len(), 2);
            FastjsonSqlUndoLog::new(SqlType::Update, Some(table), before, after)
        })
        .collect();
    (xid, branch_id, logs)
}

#[allow(dead_code)]
pub(super) fn typed_logs(method: &str) -> (String, i64, Vec<seata_at::JacksonSqlUndoLog>) {
    use seata_at::{JacksonUndoField, JacksonUndoFieldValue as V, JacksonUndoRecords};
    let (xid, id, source) = from_h2(method);
    let image = |records: &seata_at::FastjsonUndoRecords| {
        let rows = records
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(|field| {
                        JacksonUndoField::new(
                            field.name().to_owned(),
                            field.jdbc_type(),
                            field.is_primary(),
                            V::Common(field.value().clone()),
                        )
                    })
                    .collect()
            })
            .collect();
        JacksonUndoRecords::new(records.table_name().map(str::to_owned), rows)
    };
    let logs = source
        .into_iter()
        .map(|log| {
            assert_eq!(log.sql_type(), seata_at::SqlType::Update);
            seata_at::JacksonSqlUndoLog::new(
                Some("UPDATE".into()),
                log.table_name().map(str::to_owned),
                Some(image(log.before_image())),
                Some(image(log.after_image())),
            )
        })
        .collect();
    (xid, id, logs)
}
