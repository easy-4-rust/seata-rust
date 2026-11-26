//! 原 JdbcUtils#loadDriver 的加载顺序、选择和异常传播。
use seata_db_mysql::{JdbcDriverLoader, JdbcUtils};
use seata_sqlparser::{JavaObjectError, JavaString, MetadataError};
use std::{error::Error, sync::Arc};

struct Loader {
    context: Result<Option<&'static str>, MetadataError>,
    default: Result<&'static str, MetadataError>,
    constructor_error: Option<MetadataError>,
    calls: Vec<&'static str>,
    names: Vec<Option<JavaString>>,
}

impl JdbcDriverLoader for Loader {
    type Class = &'static str;
    type Driver = &'static str;

    fn load_context_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Option<Self::Class>, MetadataError> {
        self.calls.push("context");
        self.names.push(name.cloned());
        self.context.clone()
    }

    fn load_default_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Self::Class, MetadataError> {
        self.calls.push("default");
        self.names.push(name.cloned());
        self.default.clone()
    }

    fn instantiate_driver(&mut self, class: Self::Class) -> Result<Self::Driver, MetadataError> {
        self.calls.push("construct");
        self.constructor_error.clone().map_or(Ok(class), Err)
    }
}

fn error(class: &str, message: Option<JavaString>, parents: &[&str]) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new(class, message).with_superclasses(parents.iter().copied()),
    ))
}

fn class_not_found(message: Option<JavaString>) -> MetadataError {
    error("java.lang.ClassNotFoundException", message, &[])
}

fn nested_error(class: &str, message: Option<JavaString>, cause: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(JavaObjectError::with_cause(
        class,
        message,
        Some(Arc::new(JavaObjectError::new(cause, None))),
    )))
}

fn class_chain(error: &JavaObjectError) -> String {
    let mut chain = error.class_name().to_owned();
    let mut cause = error.source();
    while let Some(value) = cause.and_then(|source| source.downcast_ref::<JavaObjectError>()) {
        chain.push('>');
        chain.push_str(value.class_name());
        cause = value.source();
    }
    chain
}

#[test]
fn loader_dispatch_matches_original_java_profiles() -> Result<(), Box<dyn Error>> {
    const H2: &str = "org.h2.Driver";
    const MISSING: &str = "com.invalid.NonExistentDriver";
    let mut count = 0;
    for row in include_str!("fixtures/jdbc-load-driver-java.tsv").lines() {
        let [case, context_called, expected, expected_message]: [&str; 4] = row
            .split('\t')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "invalid Java profile row")?;
        // 宿主负责产生 JDK 具体消息，测试验证包装后原消息及异常链。
        let message = (expected_message != "<null>").then(|| JavaString::from(expected_message));
        let mut loader = Loader {
            context: Ok(Some(H2)),
            default: Ok(H2),
            constructor_error: None,
            calls: Vec::new(),
            names: Vec::new(),
        };
        let mut name = Some(H2.into());
        let has_context = !matches!(
            case,
            "no-context" | "null-class-name" | "default-initializer-fails"
        );
        if !has_context {
            loader.context = Ok(None);
        }
        match case {
            "context-driver" | "no-context" => {}
            "context-missing-default-driver" => loader.context = Err(class_not_found(None)),
            "context-null-class" => loader.context = Ok(None),
            "context-selected-class" => {
                name = Some(MISSING.into());
                loader.default = Err(class_not_found(name.clone()));
            }
            "context-linkage" => {
                loader.context = Err(error("java.lang.LinkageError", message, &[]))
            }
            "missing-class" | "empty-class" => {
                name = Some((if case == "missing-class" { MISSING } else { "" }).into());
                loader.context = Err(class_not_found(Some("context-only".into())));
                loader.default = Err(class_not_found(name.clone()));
            }
            "null-class-name" => {
                name = None;
                loader.default = Err(error("java.lang.NullPointerException", None, &[]));
            }
            "default-initializer-fails" => {
                name = Some("JdbcRmDriverLoadOracle$InitializerFailure".into());
                loader.default = Err(nested_error(
                    "java.lang.ExceptionInInitializerError",
                    None,
                    "java.lang.IllegalStateException",
                ));
            }
            _ => {
                let (class, failure) = constructor_profile(case, message)?;
                name = Some(format!("JdbcRmDriverLoadOracle${class}").into());
                loader.constructor_error = Some(failure);
            }
        }
        let (actual, actual_message) = match JdbcUtils::load_driver(name.as_ref(), &mut loader) {
            Ok(driver) => (format!("driver:{driver}"), String::new()),
            Err(failure) => {
                let error = failure
                    .object_exception()
                    .ok_or("expected Java exception")?;
                let message = failure
                    .java_message()
                    .map_or_else(|| "<null>".into(), ToString::to_string);
                (class_chain(error), message)
            }
        };
        let called = has_context && loader.calls.contains(&"context");
        assert_eq!(called.to_string(), context_called, "{case}");
        assert!(loader.names.iter().all(|found| found == &name), "{case}");
        assert_eq!(actual, expected, "{case}");
        assert_eq!(actual_message, expected_message, "{case}");
        println!("SEATA_JDBC_LOAD\t{case}\t{called}\t{actual}\t{actual_message}");
        count += 1;
    }
    assert_eq!(count, 17);
    Ok(())
}

fn constructor_profile(
    case: &str,
    message: Option<JavaString>,
) -> Result<(&str, MetadataError), String> {
    let profile = match case {
        "private-constructor" => (
            "PrivateDriver",
            error("java.lang.IllegalAccessException", message, &[]),
        ),
        "abstract-class" => (
            "AbstractDriver",
            error("java.lang.InstantiationException", None, &[]),
        ),
        "missing-constructor" => (
            "MissingConstructor",
            nested_error(
                "java.lang.InstantiationException",
                message,
                "java.lang.NoSuchMethodException",
            ),
        ),
        "constructor-access-subclass" => (
            "ConstructorAccess",
            error(
                "JdbcRmDriverLoadOracle$AccessFailure",
                Some("constructor-access".into()),
                &["java.lang.IllegalAccessException"],
            ),
        ),
        "constructor-sql" => (
            "ConstructorSql",
            error("java.sql.SQLException", Some("constructor-sql".into()), &[]),
        ),
        "constructor-class-missing" => (
            "ConstructorClassNotFound",
            class_not_found(Some("constructor-missing".into())),
        ),
        "wrong-driver-type" => (
            "NotDriver",
            error("java.lang.ClassCastException", message, &[]),
        ),
        _ => return Err(format!("unknown Java profile {case}")),
    };
    Ok(profile)
}

#[test]
fn context_prefers_its_class_and_misses_try_the_default_once() -> Result<(), MetadataError> {
    let mut loader = Loader {
        context: Ok(Some("context-class")),
        default: Ok("default-class"),
        constructor_error: None,
        calls: Vec::new(),
        names: Vec::new(),
    };
    let name = JavaString::from("org.h2.Driver");
    assert_eq!(
        JdbcUtils::load_driver(Some(&name), &mut loader)?,
        "context-class"
    );
    assert_eq!(loader.calls, ["context", "construct"]);
    for context in [Ok(None), Err(class_not_found(Some(name.clone())))] {
        loader.calls.clear();
        loader.context = context;
        assert_eq!(
            JdbcUtils::load_driver(Some(&name), &mut loader)?,
            "default-class"
        );
        assert_eq!(loader.calls, ["context", "default", "construct"]);
    }
    Ok(())
}

#[test]
fn wraps_only_designated_failures_and_keeps_cause_identity() -> Result<(), Box<dyn Error>> {
    let name = JavaString::from_utf16(vec![0x0061, 0xD800]);
    let missing = class_not_found(Some(name.clone()));
    let mut loader = Loader {
        context: Err(class_not_found(None)),
        default: Err(missing.clone()),
        constructor_error: None,
        calls: Vec::new(),
        names: Vec::new(),
    };
    let Err(wrapped) = JdbcUtils::load_driver(Some(&name), &mut loader) else {
        return Err("missing class succeeded".into());
    };
    assert_eq!(loader.calls, ["context", "default"]);
    assert_eq!(loader.names, [Some(name.clone()), Some(name.clone())]);
    check_sql_cause(&wrapped, &missing)?;

    loader.context = Ok(Some("context-class"));
    for ctor in [
        error("java.lang.IllegalAccessException", None, &[]),
        error(
            "example.CustomInstantiationException",
            Some(name.clone()),
            &["java.lang.InstantiationException"],
        ),
    ] {
        loader.calls.clear();
        loader.constructor_error = Some(ctor.clone());
        let Err(wrapped) = JdbcUtils::load_driver(Some(&name), &mut loader) else {
            return Err("failed constructor succeeded".into());
        };
        assert_eq!(loader.calls, ["context", "construct"]);
        check_sql_cause(&wrapped, &ctor)?;
    }
    Ok(())
}

fn check_sql_cause(
    wrapped: &MetadataError,
    original: &MetadataError,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        wrapped.java_exception_class(),
        Some("java.sql.SQLException")
    );
    assert_eq!(wrapped.java_message(), original.java_message());
    let cause = wrapped
        .object_exception()
        .and_then(|error| error.source())
        .and_then(|cause| cause.downcast_ref::<JavaObjectError>())
        .ok_or("missing original cause")?;
    let expected = original
        .object_exception()
        .ok_or("missing original error")?;
    assert!(std::ptr::eq(cause, expected.as_ref()));
    Ok(())
}

#[test]
fn unrelated_errors_stop_the_current_stage_without_wrapping() {
    let failure = error("java.lang.LinkageError", None, &[]);
    let mut loader = Loader {
        context: Err(failure.clone()),
        default: Ok("default-class"),
        constructor_error: None,
        calls: Vec::new(),
        names: Vec::new(),
    };
    assert_eq!(
        JdbcUtils::load_driver(None, &mut loader),
        Err(failure.clone())
    );
    assert_eq!(loader.calls, ["context"]);

    loader.context = Ok(None);
    loader.default = Err(failure.clone());
    loader.calls.clear();
    assert_eq!(JdbcUtils::load_driver(None, &mut loader), Err(failure));
    assert_eq!(loader.calls, ["context", "default"]);

    for class in [
        "java.lang.ClassCastException",
        "java.lang.ClassNotFoundException",
        "java.sql.SQLException",
    ] {
        let failure = error(class, None, &[]);
        loader.context = Ok(Some("context-class"));
        loader.constructor_error = Some(failure.clone());
        loader.calls.clear();
        assert_eq!(JdbcUtils::load_driver(None, &mut loader), Err(failure));
        assert_eq!(loader.calls, ["context", "construct"]);
    }
}
