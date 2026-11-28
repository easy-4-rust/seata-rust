//! 七个原 JdbcUtilsTest 资源初始化用例。连接沿用源测试的模拟输入，驱动来自实际 JVM。
#[path = "support/jdbc_driver_primitive_host.rs"]
mod primitive_host;

use primitive_host::{DriverObservation, Host};
use seata_db_mysql::{
    JdbcConnection, JdbcDriverLoader, JdbcResourceInitialization, JdbcUtils, JdbcXaConnection,
};
use seata_sqlparser::{
    DruidJdbcDriverResolver, JavaObjectError, JavaString, MetadataError, MetadataLocale,
};
use std::{cell::RefCell, error::Error, rc::Rc, sync::Arc};

const MYSQL: &str = "jdbc:mysql://localhost:3306/test?useSSL=false&serverTimezone=UTC";
const H2: &str = "jdbc:h2:mem:test";
type Trace = Rc<RefCell<Vec<String>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Success,
    AcquireSql,
    Null,
    CloseSql,
}

struct Connection {
    trace: Trace,
    xa: bool,
    input: Input,
}

fn sql(message: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new("java.sql.SQLException", Some(message.into()))
            .with_superclasses(["java.lang.Exception", "java.lang.Throwable"]),
    ))
}

impl JdbcConnection for Connection {
    fn jdbc_url(&mut self) -> Result<Option<JavaString>, MetadataError> {
        self.trace.borrow_mut().push("metadata".into());
        if !self.xa && self.input == Input::Null {
            return Err(MetadataError::ObjectException(Arc::new(
                JavaObjectError::new("java.lang.NullPointerException", Some(
                    r#"Cannot invoke "java.sql.DatabaseMetaData.getURL()" because the return value of "java.sql.Connection.getMetaData()" is null"#.into()
                )).with_superclasses(["java.lang.RuntimeException","java.lang.Exception","java.lang.Throwable"]),
            )));
        }
        self.trace.borrow_mut().push("url".into());
        Ok(Some(if self.xa { H2 } else { MYSQL }.into()))
    }

    fn close(self) -> Result<(), MetadataError> {
        self.trace.borrow_mut().push("close-connection".into());
        Ok(())
    }
}

struct Xa(Connection);

impl JdbcXaConnection for Xa {
    type Connection = Connection;
    fn get_connection(&mut self) -> Result<Option<Connection>, MetadataError> {
        self.0.trace.borrow_mut().push("get-connection".into());
        Ok(Some(Connection {
            trace: self.0.trace.clone(),
            xa: true,
            input: self.0.input,
        }))
    }
    fn close(self) -> Result<(), MetadataError> {
        self.0.trace.borrow_mut().push("close-xa".into());
        if self.0.input == Input::CloseSql {
            return Err(sql("Close failed"));
        }
        Ok(())
    }
}

struct Resource {
    trace: Trace,
    driver: Host,
    xa: bool,
}

impl JdbcDriverLoader for Resource {
    type Class = ();
    type Driver = DriverObservation;
    fn load_context_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Option<()>, MetadataError> {
        self.driver.load_context_class(name)
    }
    fn load_default_class(&mut self, name: Option<&JavaString>) -> Result<(), MetadataError> {
        self.driver.load_default_class(name)
    }
    fn instantiate_driver(&mut self, class: ()) -> Result<Self::Driver, MetadataError> {
        self.driver.instantiate_driver(class)
    }
}

impl JdbcResourceInitialization for Resource {
    fn class_available(&mut self, name: &str) -> Result<bool, MetadataError> {
        match self.driver.load_default_class(Some(&name.into())) {
            Ok(()) => Ok(true),
            Err(err) if err.java_exception_class() == Some("java.lang.ClassNotFoundException") => {
                Ok(false)
            }
            Err(err) => Err(err),
        }
    }
    fn set_resource_group_id(&mut self, value: Option<JavaString>) -> Result<(), MetadataError> {
        let value = value.ok_or(MetadataError::InvalidCompatibilityData(
            "expected original group",
        ))?;
        self.trace.borrow_mut().push(format!("group={value}"));
        Ok(())
    }
    fn set_resource_id(&mut self, value: JavaString) -> Result<(), MetadataError> {
        self.trace.borrow_mut().push(format!("id={value}"));
        Ok(())
    }
    fn set_driver(&mut self, (class, h2): DriverObservation) -> Result<(), MetadataError> {
        assert_eq!(h2, self.xa);
        self.trace.borrow_mut().push(format!("driver={class}"));
        Ok(())
    }
    fn set_db_type(&mut self, value: JavaString) -> Result<(), MetadataError> {
        self.trace.borrow_mut().push(format!("db={value}"));
        Ok(())
    }
    fn register_resource(&mut self) -> Result<(), MetadataError> {
        self.trace.borrow_mut().push("register".into());
        Ok(())
    }
    fn describe_data_source(&mut self) -> Result<JavaString, MetadataError> {
        Ok(if self.xa { "xa-source" } else { "jdbc-source" }.into())
    }
}

fn describe(error: &JavaObjectError) -> String {
    let message = error
        .message_java()
        .map_or_else(|| "null".into(), ToString::to_string);
    let mut description = format!("{}:{message}", error.class_name());
    if let Some(cause) = error
        .source()
        .and_then(|e| e.downcast_ref::<JavaObjectError>())
    {
        description.push_str(&format!(" cause({})", describe(cause)));
    }
    description
}

fn observe(method: &str, xa: bool, input: Input) -> Result<(), Box<dyn Error>> {
    let trace = Trace::default();
    let mut resource = Resource {
        trace: trace.clone(),
        driver: Host::start()?,
        xa,
    };
    let connection = || Connection {
        trace: trace.clone(),
        xa,
        input,
    };
    let group = if xa { "xa-group" } else { "test-group" };
    let resolver = DruidJdbcDriverResolver::default();
    let locale = MetadataLocale::new("en");
    let result = if xa {
        JdbcUtils::init_xa_data_source_resource(
            &mut resource,
            Some(group.into()),
            &resolver,
            &locale,
            || {
                trace.borrow_mut().push("get-xa".into());
                match input {
                    Input::AcquireSql => Err(sql("XA connection failed")),
                    Input::Null => Ok(None),
                    _ => Ok(Some(Xa(connection()))),
                }
            },
        )
    } else {
        JdbcUtils::init_data_source_resource(
            &mut resource,
            Some(group.into()),
            &resolver,
            &locale,
            || {
                trace.borrow_mut().push("get-connection".into());
                match input {
                    Input::AcquireSql => Err(sql("Connection failed")),
                    _ => Ok(Some(connection())),
                }
            },
        )
    };
    let trace = trace.borrow();
    let count = |value: &str| trace.iter().filter(|e| e.as_str() == value).count();
    assert_eq!(count(&format!("group={group}")), 1);
    assert_eq!(count("register"), usize::from(input == Input::Success));
    assert_eq!(
        count("close-connection"),
        usize::from(input != Input::AcquireSql && !(xa && input == Input::Null))
    );
    assert_eq!(
        count("close-xa"),
        usize::from(xa && matches!(input, Input::Success | Input::CloseSql))
    );
    let observed = match input {
        Input::Success => {
            result?;
            let id = if xa {
                H2
            } else {
                "jdbc:mysql://localhost:3306/test"
            };
            let db = if xa { "h2" } else { "mysql" };
            assert_eq!(count(&format!("id={id}")), 1);
            assert_eq!(count(&format!("db={db}")), 1);
            assert_eq!(
                trace
                    .iter()
                    .filter(|step| step.starts_with("driver="))
                    .count(),
                1
            );
            "ok".into()
        }
        _ => {
            let error = result
                .err()
                .ok_or("expected original initialization exception")?;
            let object = error.object_exception().ok_or("expected JVM exception")?;
            assert!(object.is_instance_of("java.lang.Exception"));
            if input == Input::AcquireSql {
                let prefix = if xa {
                    "can not get XAConnection"
                } else {
                    "can not init DataSourceResource"
                };
                assert!(object.is_instance_of("java.lang.IllegalStateException"));
                assert!(object
                    .message_java()
                    .ok_or("expected original wrapper message")?
                    .to_string()
                    .contains(prefix));
                let cause = object
                    .source()
                    .and_then(|e| e.downcast_ref::<JavaObjectError>())
                    .ok_or("expected original SQL cause")?;
                assert!(cause.is_instance_of("java.sql.SQLException"));
            }
            describe(object)
        }
    };
    println!(
        "SEATA_JDBC_INIT_SOURCE\t{method}\t{}\t{observed}",
        trace.join("|")
    );
    Ok(())
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_data_source_resource_success() -> Result<(), Box<dyn Error>> {
    observe("testInitDataSourceResourceSuccess", false, Input::Success)
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_data_source_resource_with_sql_exception() -> Result<(), Box<dyn Error>> {
    observe(
        "testInitDataSourceResourceWithSQLException",
        false,
        Input::AcquireSql,
    )
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_data_source_resource_with_null_meta_data() -> Result<(), Box<dyn Error>> {
    observe(
        "testInitDataSourceResourceWithNullMetaData",
        false,
        Input::Null,
    )
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_xa_data_source_resource_success() -> Result<(), Box<dyn Error>> {
    observe("testInitXADataSourceResourceSuccess", true, Input::Success)
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_xa_data_source_resource_with_sql_exception() -> Result<(), Box<dyn Error>> {
    observe(
        "testInitXADataSourceResourceWithSQLException",
        true,
        Input::AcquireSql,
    )
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_xa_data_source_resource_with_null_xa_connection() -> Result<(), Box<dyn Error>> {
    observe(
        "testInitXADataSourceResourceWithNullXAConnection",
        true,
        Input::Null,
    )
}

#[test]
#[ignore = "requires pinned JVM and original JDBC drivers"]
fn test_init_xa_data_source_resource_ensures_connection_closed() -> Result<(), Box<dyn Error>> {
    observe(
        "testInitXADataSourceResourceEnsuresConnectionClosed",
        true,
        Input::CloseSql,
    )
}
