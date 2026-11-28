//! 固定原 JdbcUtilsTest 的四个驱动用例，经实际 JVM 原语执行。
#[path = "support/jdbc_driver_primitive_host.rs"]
mod primitive_host;

use primitive_host::{DriverObservation, Host};
use seata_db_mysql::JdbcUtils;
use seata_sqlparser::{JavaObjectError, JavaString, MetadataError};
use std::error::Error;

fn load(name: Option<&str>) -> Result<Result<DriverObservation, MetadataError>, Box<dyn Error>> {
    Ok(JdbcUtils::load_driver(
        name.map(JavaString::from).as_ref(),
        &mut Host::start()?,
    ))
}

fn observe_error(case: &str, error: &JavaObjectError) {
    let mut chain = error.class_name().to_owned();
    let mut cause = error.source();
    while let Some(current) = cause.and_then(|value| value.downcast_ref::<JavaObjectError>()) {
        chain.push('>');
        chain.push_str(current.class_name());
        cause = current.source();
    }
    println!("SEATA_JDBC_JVM_SOURCE\t{case}\t{chain}");
}

#[test]
#[ignore = "requires pinned Java and original H2 JDBC driver"]
fn test_load_driver_with_valid_h2_driver() -> Result<(), Box<dyn Error>> {
    let loaded = load(Some("org.h2.Driver"))?;
    if let Err(error) = &loaded {
        if let Some(original) = error.object_exception() {
            observe_error("testLoadDriverWithValidH2Driver", original);
        }
    }
    let (class, h2) = loaded?;
    assert!(h2);
    println!("SEATA_JDBC_JVM_SOURCE\ttestLoadDriverWithValidH2Driver\tdriver:{class}:{h2}");
    Ok(())
}

#[test]
#[ignore = "requires pinned Java and original H2 JDBC driver"]
fn test_load_driver_with_invalid_driver_class() -> Result<(), Box<dyn Error>> {
    let Err(error) = load(Some("com.invalid.NonExistentDriver"))? else {
        return Err("expected SQLException".into());
    };
    let error = error.object_exception().ok_or("not a Java exception")?;
    assert!(error.is_instance_of("java.sql.SQLException"));
    let cause = error.source().ok_or("expected an original cause")?;
    let cause = cause
        .downcast_ref::<JavaObjectError>()
        .ok_or("invalid original cause")?;
    assert!(cause.is_instance_of("java.lang.ClassNotFoundException"));
    observe_error("testLoadDriverWithInvalidDriverClass", error);
    Ok(())
}

fn assert_java_exception(case: &str, input: Option<&str>) -> Result<(), Box<dyn Error>> {
    let Err(error) = load(input)? else {
        return Err("expected a Java Exception".into());
    };
    let error = error.object_exception().ok_or("not a Java exception")?;
    assert!(error.is_instance_of("java.lang.Exception"));
    observe_error(case, error);
    Ok(())
}

#[test]
#[ignore = "requires pinned Java and original H2 JDBC driver"]
fn test_load_driver_with_null_class_name() -> Result<(), Box<dyn Error>> {
    assert_java_exception("testLoadDriverWithNullClassName", None)
}

#[test]
#[ignore = "requires pinned Java and original H2 JDBC driver"]
fn test_load_driver_with_empty_class_name() -> Result<(), Box<dyn Error>> {
    assert_java_exception("testLoadDriverWithEmptyClassName", Some(""))
}
