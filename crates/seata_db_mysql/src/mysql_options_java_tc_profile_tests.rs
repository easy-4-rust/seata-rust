#![allow(clippy::unwrap_used)]
use crate::MySqlOptions;

fn options() -> MySqlOptions {
    MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "writer".into(),
        "test-only".into(),
        "business".into(),
    )
    .unwrap()
}

#[test]
fn fixed_java_tc_mysql_db_profile_enables_all_three_registration_guards() {
    let default = options();
    assert!(!default.require_resource_binding);
    assert!(!default.require_tinyint_profile_binding);
    assert!(!default.java_tc_mysql_lock_limits);
    assert!(!default.java_tc_mysql_lock_collation);

    let fixed = options().with_java_tc_mysql_db_lock_profile();
    assert!(fixed.require_resource_binding);
    assert!(!fixed.require_tinyint_profile_binding);
    assert!(fixed.java_tc_mysql_lock_limits);
    assert!(fixed.java_tc_mysql_lock_collation);
    assert_eq!(fixed.database, default.database);
    assert_eq!(fixed.maximum_connections, default.maximum_connections);
}

#[test]
fn tinyint_profile_binding_also_requires_the_original_resource_binding() {
    let strict = options().require_at_tinyint_profile_binding();
    assert!(strict.require_resource_binding);
    assert!(strict.require_tinyint_profile_binding);
}

#[test]
fn fixed_profile_preserves_explicit_tls_and_existing_guard_settings() {
    let options = options()
        .with_insecure_loopback_tls()
        .unwrap()
        .with_java_tc_mysql_lock_limits()
        .with_java_tc_mysql_db_lock_profile();
    assert!(options.require_resource_binding);
    assert!(options.java_tc_mysql_lock_limits);
    assert!(options.java_tc_mysql_lock_collation);
    assert!(format!("{options:?}").contains("insecure_loopback: true"));
}

#[test]
fn full_jdbc_profile_binding_requires_resource_and_tinyint_bindings() {
    let default = options();
    assert!(!default.require_jdbc_profile_binding);
    let strict = options().require_at_jdbc_profile_binding();
    assert!(
        strict.require_resource_binding
            && strict.require_tinyint_profile_binding
            && strict.require_jdbc_profile_binding
    );
    assert_eq!(strict.database, default.database);
    assert!(format!("{strict:?}").contains("require_jdbc_profile_binding: true"));
}
#[test]
fn fixed_tc_profile_preserves_an_explicit_full_jdbc_guard() {
    let strict = options()
        .require_at_jdbc_profile_binding()
        .with_java_tc_mysql_db_lock_profile();
    assert!(
        strict.require_resource_binding
            && strict.require_tinyint_profile_binding
            && strict.require_jdbc_profile_binding
    );
    assert!(strict.java_tc_mysql_lock_limits && strict.java_tc_mysql_lock_collation);
    assert!(
        !options()
            .with_java_tc_mysql_db_lock_profile()
            .require_jdbc_profile_binding
    );
}
