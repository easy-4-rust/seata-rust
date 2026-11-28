//! 固定版原 Java 的数据源初始化流程和异常优先级。
use seata_db_mysql::{
    JdbcConnection, JdbcDriverLoader, JdbcResourceInitialization, JdbcUtils, JdbcXaConnection,
};
use seata_sqlparser::{
    DruidJdbcDriverResolver, JavaObjectError, JavaString, MetadataError, MetadataLocale,
};
use std::{cell::RefCell, error::Error, rc::Rc, sync::Arc};

type Trace = Rc<RefCell<Vec<String>>>;

fn record(trace: &Trace, step: impl Into<String>) {
    trace.borrow_mut().push(step.into());
}

fn failure(class: &str, message: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(JavaObjectError::new(class, Some(message.into()))))
}

fn sql(step: &str) -> MetadataError {
    failure("java.sql.SQLException", step)
}

struct Connection {
    trace: Trace,
    case: String,
}

impl JdbcConnection for Connection {
    fn jdbc_url(&mut self) -> Result<Option<JavaString>, MetadataError> {
        record(&self.trace, "metadata");
        if self.case == "rust-only-and-close" {
            return Err(MetadataError::InvalidCompatibilityData("host metadata"));
        }
        if self.case == "jdbc-metadata-and-close-sql" {
            return Err(sql("metadata"));
        }
        if self.case.contains("null-metadata") || self.case == "xa-close-replaces-runtime-error" {
            return Err(failure(
                "java.lang.NullPointerException",
                r#"Cannot invoke "java.sql.DatabaseMetaData.getURL()" because the return value of "java.sql.Connection.getMetaData()" is null"#,
            ));
        }
        record(&self.trace, "url");
        let url = match self.case.as_str() {
            "jdbc-null-url" => return Ok(None),
            "jdbc-unknown-driver" => "jdbc:unknown:db?param=1",
            "jdbc-dispatch-only-type" => "jdbc:dbcp:test",
            name if name.starts_with("xa-") => "jdbc:h2:mem:test?param=1",
            _ => "jdbc:mysql://localhost:3306/test?param=1",
        };
        Ok(Some(url.into()))
    }

    fn close(self) -> Result<(), MetadataError> {
        record(&self.trace, "close-connection");
        if matches!(
            self.case.as_str(),
            "jdbc-close-sql"
                | "jdbc-metadata-and-close-sql"
                | "jdbc-null-metadata-and-close-sql"
                | "xa-inner-close-sql"
                | "xa-both-close-sql"
                | "xa-close-replaces-runtime-error"
                | "jdbc-dispatch-only-type"
                | "rust-only-and-close"
        ) {
            Err(sql("close-connection"))
        } else {
            Ok(())
        }
    }
}

struct Xa(Connection);

impl JdbcXaConnection for Xa {
    type Connection = Connection;

    fn get_connection(&mut self) -> Result<Option<Self::Connection>, MetadataError> {
        record(&self.0.trace, "get-connection");
        if self.0.case == "xa-inner-acquire-sql" {
            return Err(sql("get-connection"));
        }
        Ok(Some(Connection {
            trace: self.0.trace.clone(),
            case: self.0.case.clone(),
        }))
    }

    fn close(self) -> Result<(), MetadataError> {
        record(&self.0.trace, "close-xa");
        if matches!(
            self.0.case.as_str(),
            "xa-close-sql" | "xa-both-close-sql" | "xa-close-replaces-runtime-error"
        ) {
            Err(sql("close-xa"))
        } else {
            Ok(())
        }
    }
}

struct Host {
    trace: Trace,
    description: &'static str,
    probe_error: Option<MetadataError>,
}

impl JdbcDriverLoader for Host {
    type Class = JavaString;
    type Driver = JavaString;

    fn load_context_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Option<Self::Class>, MetadataError> {
        Ok(name.cloned())
    }

    fn load_default_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Self::Class, MetadataError> {
        name.cloned()
            .ok_or(MetadataError::NullReference("driverClassName"))
    }

    fn instantiate_driver(&mut self, class: Self::Class) -> Result<Self::Driver, MetadataError> {
        Ok(class)
    }
}

impl JdbcResourceInitialization for Host {
    fn class_available(&mut self, _class_name: &str) -> Result<bool, MetadataError> {
        if let Some(error) = self.probe_error.take() {
            record(&self.trace, "probe");
            return Err(error);
        }
        Ok(false)
    }

    fn set_resource_group_id(&mut self, value: Option<JavaString>) -> Result<(), MetadataError> {
        let value = value.map_or_else(|| "null".into(), |v| v.to_string());
        record(&self.trace, format!("group={value}"));
        Ok(())
    }

    fn set_resource_id(&mut self, value: JavaString) -> Result<(), MetadataError> {
        record(&self.trace, format!("id={value}"));
        Ok(())
    }

    fn set_driver(&mut self, value: JavaString) -> Result<(), MetadataError> {
        record(&self.trace, format!("driver={value}"));
        Ok(())
    }

    fn set_db_type(&mut self, value: JavaString) -> Result<(), MetadataError> {
        record(&self.trace, format!("db={value}"));
        Ok(())
    }

    fn register_resource(&mut self) -> Result<(), MetadataError> {
        record(&self.trace, "register");
        Ok(())
    }

    fn describe_data_source(&mut self) -> Result<JavaString, MetadataError> {
        Ok(self.description.into())
    }
}

fn describe(error: &JavaObjectError) -> String {
    let message = error
        .message_java()
        .map_or_else(|| "null".into(), ToString::to_string);
    let mut result = format!("{}:{message}", error.class_name());
    if let Some(cause) = error
        .source()
        .and_then(|e| e.downcast_ref::<JavaObjectError>())
    {
        result.push_str(&format!(" cause({})", describe(cause)));
    }
    for suppressed in error.suppressed() {
        result.push_str(&format!(" suppressed({})", describe(&suppressed)));
    }
    result
}

#[test]
fn db_type_null_reference_preserves_the_close_exception() -> Result<(), Box<dyn Error>> {
    let trace = Trace::default();
    let mut host = Host {
        trace: trace.clone(),
        description: "jdbc-probe",
        probe_error: None,
    };
    let result = JdbcUtils::init_data_source_resource(
        &mut host,
        None,
        &DruidJdbcDriverResolver::default(),
        &MetadataLocale::new("en"),
        || {
            Ok(Some(Connection {
                trace: trace.clone(),
                case: "jdbc-dispatch-only-type".into(),
            }))
        },
    );
    let failure = result.err().ok_or("database type unexpectedly resolved")?;
    assert_eq!(
        failure.java_exception_class(),
        Some("java.lang.NullPointerException")
    );
    let primary = failure
        .object_exception()
        .ok_or("close failure was dropped")?;
    assert!(primary.is_instance_of("java.lang.RuntimeException"));
    assert!(primary.is_instance_of("java.lang.Exception"));
    assert!(primary.is_instance_of("java.lang.Throwable"));
    let suppressed = primary.suppressed();
    assert_eq!(suppressed.len(), 1);
    assert_eq!(suppressed[0].class_name(), "java.sql.SQLException");
    assert_eq!(
        suppressed[0].message_java(),
        Some(&JavaString::from("close-connection"))
    );
    assert_eq!(
        trace.borrow().join("|"),
        "group=null|metadata|url|id=jdbc:dbcp:test|driver=com.dbcp.jdbc.Driver|close-connection"
    );
    Ok(())
}

#[test]
fn suppressed_retains_identity_and_does_not_form_an_arc_cycle() -> Result<(), Box<dyn Error>> {
    let primary = Arc::new(JavaObjectError::new("java.sql.SQLException", None));
    let secondary = Arc::new(JavaObjectError::new("java.sql.SQLException", None));
    let primary_weak = Arc::downgrade(&primary);
    let secondary_weak = Arc::downgrade(&secondary);
    primary.add_suppressed(secondary.clone())?;
    let saved = primary.suppressed();
    assert!(Arc::ptr_eq(&saved[0], &secondary));
    assert!(secondary.add_suppressed(primary.clone()).is_err());
    let dependent = Arc::new(JavaObjectError::with_cause(
        "java.sql.SQLException",
        None,
        Some(primary.clone()),
    ));
    assert!(primary.add_suppressed(dependent).is_err());
    let detached = primary
        .add_suppressed(primary.clone())
        .err()
        .ok_or("self-suppression succeeded")?;
    assert_eq!(
        detached.java_exception_class(),
        Some("java.lang.IllegalArgumentException")
    );
    let cause = detached
        .object_exception()
        .and_then(|error| error.source())
        .and_then(|cause| cause.downcast_ref::<JavaObjectError>())
        .ok_or("self-suppression error lost its cause")?;
    assert!(std::ptr::eq(cause, primary.as_ref()));
    drop(detached);
    drop(saved);
    drop(primary);
    drop(secondary);
    assert!(primary_weak.upgrade().is_none());
    assert!(secondary_weak.upgrade().is_none());
    Ok(())
}

#[test]
fn rust_host_failure_keeps_both_operation_and_close_errors() -> Result<(), Box<dyn Error>> {
    let trace = Trace::default();
    let mut host = Host {
        trace: trace.clone(),
        description: "jdbc-probe",
        probe_error: None,
    };
    let result = JdbcUtils::init_data_source_resource(
        &mut host,
        None,
        &DruidJdbcDriverResolver::default(),
        &MetadataLocale::new("en"),
        || {
            Ok(Some(Connection {
                trace: trace.clone(),
                case: "rust-only-and-close".into(),
            }))
        },
    );
    let Err(MetadataError::CleanupFailed { primary, cleanup }) = result else {
        return Err("expected both Rust and Java errors".into());
    };
    assert_eq!(
        *primary,
        MetadataError::InvalidCompatibilityData("host metadata")
    );
    assert_eq!(
        cleanup.java_exception_class(),
        Some("java.sql.SQLException")
    );
    assert_eq!(
        trace.borrow().join("|"),
        "group=null|metadata|close-connection"
    );
    Ok(())
}

#[test]
fn jdbc_and_xa_initialization_match_original_close_and_registration_order(
) -> Result<(), Box<dyn Error>> {
    let resolver = DruidJdbcDriverResolver::default();
    let locale = MetadataLocale::new("en");
    for row in include_str!("fixtures/jdbc-resource-init-java.tsv").lines() {
        let [case, expected_trace, expected_result]: [&str; 3] = row
            .split('\t')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "invalid Java row")?;
        let xa = case.starts_with("xa-");
        let trace = Trace::default();
        let mut host = Host {
            trace: trace.clone(),
            description: if xa { "xa-probe" } else { "jdbc-probe" },
            probe_error: (case == "jdbc-driver-probe-fails")
                .then(|| failure("java.lang.LinkageError", "driver-probe")),
        };
        let connection = || Connection {
            trace: trace.clone(),
            case: case.into(),
        };
        let group = Some("group".into());
        let result = if xa {
            JdbcUtils::init_xa_data_source_resource(&mut host, group, &resolver, &locale, || {
                record(&trace, "get-xa");
                match case {
                    "xa-acquire-sql" => Err(sql("get-xa")),
                    "xa-null-connection" => Ok(None),
                    _ => Ok(Some(Xa(connection()))),
                }
            })
        } else {
            JdbcUtils::init_data_source_resource(&mut host, group, &resolver, &locale, || {
                record(&trace, "get-connection");
                match case {
                    "jdbc-acquire-sql" => Err(sql("get-connection")),
                    "jdbc-null-connection" => Ok(None),
                    _ => Ok(Some(connection())),
                }
            })
        };
        let actual_trace = trace.borrow().join("|");
        let actual_result = match result {
            Ok(()) => "ok".to_owned(),
            Err(error) => describe(error.object_exception().ok_or("expected Java error")?),
        };
        assert_eq!(actual_trace, expected_trace, "{case}");
        assert_eq!(actual_result, expected_result, "{case}");
        println!("SEATA_JDBC_INIT\t{case}\t{actual_trace}\t{actual_result}");
        if case == "jdbc-driver-probe-fails" {
            let retry = failure("java.lang.LinkageError", "probe-retried");
            assert_eq!(
                resolver.try_get_driver_class_name(
                    Some(&"jdbc:mysql://localhost:3306/test".into()),
                    |_| Err(retry.clone()),
                ),
                Err(retry),
            );
        }
    }
    Ok(())
}
