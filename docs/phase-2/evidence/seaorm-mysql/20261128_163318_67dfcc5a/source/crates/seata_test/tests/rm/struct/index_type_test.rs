//! 对应 Java: org.apache.seata.rm.datasource.sql.struct.IndexTypeTest；保留全部原断言与顺序。
#![allow(clippy::unwrap_used)]
#[path = "../../support/rm_metadata_source.rs"]
#[allow(dead_code)]
mod support;
use seata_sqlparser::IndexType;
use support::{check, observe, TestResult};

#[test]
fn original_rm_test_index_type() -> TestResult {
    let mut values = Vec::new();
    check(
        &mut values,
        "not_null",
        IndexType::try_from(1).is_ok(),
        true,
    );
    let error = IndexType::try_from(4).unwrap_err();
    assert_eq!(error.enum_name(), "IndexType");
    assert_eq!(error.value(), 4);
    // 固定 InvalidEnumValue 映射为 IllegalArgumentException，不把其他错误算作此异常。
    values.push("throws:java.lang.IllegalArgumentException".into());
    observe("IndexTypeTest", "testIndexType", &values)
}
