//! 对应原 SQL parser core 的23个案例；原断言与逐项观察分别保留。
use seata_sqlparser::{
    EscapeSymbol, IndexType, NotPlaceholderExpr, Null, SQLParsingException, SQLType,
    SqlDefaultExpr, SqlMethodExpr, SqlSequenceExpr,
};
use std::error::Error;
use std::fmt;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::{Arc, Mutex};

type TestResult = Result<(), Box<dyn Error>>;
static OUTPUT: Mutex<()> = Mutex::new(());

fn record(class: &str, method: &str, values: &[String]) -> TestResult {
    if let Some(path) = std::env::var_os("SEATA_CORE_OBSERVATIONS") {
        let _lock = OUTPUT.lock().map_err(|_| "Observation lock poisoned")?;
        let bytes = values.join("\u{1f}").into_bytes();
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut hex, "{byte:02x}")?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "org.apache.seata.sqlparser.{class}#{method}\t{hex}")?;
    }
    Ok(())
}

#[derive(Debug)]
struct JavaCause;
impl fmt::Display for JavaCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("java.lang.IllegalArgumentException: Test cause")
    }
}
impl Error for JavaCause {}

#[test]
fn escape_left_symbol() -> TestResult {
    let symbol = EscapeSymbol::paired(34, 34);
    assert_eq!(symbol.left_symbol(), 34);
    record(
        "EscapeSymbolTest",
        "testGetLeftSymbol",
        &[symbol.left_symbol().to_string()],
    )
}

#[test]
fn escape_right_symbol() -> TestResult {
    let symbol = EscapeSymbol::paired(34, 34);
    assert_eq!(symbol.right_symbol(), 34);
    record(
        "EscapeSymbolTest",
        "testGetRightSymbol",
        &[symbol.right_symbol().to_string()],
    )
}

#[test]
fn sql_type_value() -> TestResult {
    assert_eq!(SQLType::SELECT.value(), 0);
    assert_eq!(SQLType::INSERT.value(), 1);
    record(
        "SQLTypeTest",
        "testValue",
        &[
            SQLType::SELECT.value().to_string(),
            SQLType::INSERT.value().to_string(),
        ],
    )
}

#[test]
fn sql_type_value_of() -> TestResult {
    let select = SQLType::try_from(0)?;
    let insert = SQLType::try_from(1)?;
    assert_eq!(select, SQLType::SELECT);
    assert_eq!(insert, SQLType::INSERT);
    record(
        "SQLTypeTest",
        "testValueOf",
        &[select.to_string(), insert.to_string()],
    )
}

#[test]
fn sql_type_invalid() -> TestResult {
    let result = SQLType::try_from(100);
    assert!(result.is_err(), "Invalid SQLType must be rejected");
    let error = result.err().ok_or("Missing invalid SQLType error")?;
    assert_eq!(error.enum_name(), "SQLType");
    assert_eq!(error.value(), 100);
    record(
        "SQLTypeTest",
        "testValueOfInvalid",
        &[
            "java.lang.IllegalArgumentException".into(),
            error.to_string(),
        ],
    )
}

#[test]
fn index_type_value() -> TestResult {
    let values = [
        IndexType::PRIMARY,
        IndexType::NORMAL,
        IndexType::UNIQUE,
        IndexType::FULL_TEXT,
    ];
    let numbers: Vec<i32> = values.iter().map(|value| value.value()).collect();
    assert_eq!(numbers, [0, 1, 2, 3]);
    record(
        "struct.IndexTypeTest",
        "testValue",
        &numbers.iter().map(ToString::to_string).collect::<Vec<_>>(),
    )
}

#[test]
fn index_type_value_of() -> TestResult {
    let values = [
        IndexType::try_from(0)?,
        IndexType::try_from(1)?,
        IndexType::try_from(2)?,
        IndexType::try_from(3)?,
    ];
    assert_eq!(
        values,
        [
            IndexType::PRIMARY,
            IndexType::NORMAL,
            IndexType::UNIQUE,
            IndexType::FULL_TEXT
        ]
    );
    record(
        "struct.IndexTypeTest",
        "testValueOf",
        &values.iter().map(ToString::to_string).collect::<Vec<_>>(),
    )
}

#[test]
fn index_type_invalid() -> TestResult {
    let result = IndexType::try_from(4);
    assert!(result.is_err(), "Invalid IndexType must be rejected");
    let error = result.err().ok_or("Missing invalid IndexType error")?;
    assert_eq!(error.enum_name(), "IndexType");
    assert_eq!(error.value(), 4);
    record(
        "struct.IndexTypeTest",
        "testInvalidValueOf",
        &[
            "java.lang.IllegalArgumentException".into(),
            error.to_string(),
        ],
    )
}

#[test]
fn exception_message() -> TestResult {
    let exception = SQLParsingException::new(Some("Test message".into()));
    assert_eq!(exception.message(), Some("Test message"));
    assert!(exception.cause().is_none());
    record(
        "SQLParsingExceptionTest",
        "testConstructorWithMessage",
        &[
            exception.message().ok_or("Missing message")?.into(),
            exception.cause().is_none().to_string(),
        ],
    )
}

#[test]
fn exception_message_and_cause() -> TestResult {
    let cause: Arc<dyn Error + Send + Sync> = Arc::new(JavaCause);
    let exception =
        SQLParsingException::with_cause(Some("Test message".into()), Some(Arc::clone(&cause)));
    assert_eq!(exception.message(), Some("Test message"));
    let actual = exception.cause().ok_or("Missing cause")?;
    assert!(Arc::ptr_eq(actual, &cause));
    assert!(exception.source().is_some());
    record(
        "SQLParsingExceptionTest",
        "testConstructorWithMessageAndCause",
        &[
            exception.message().ok_or("Missing message")?.into(),
            Arc::ptr_eq(actual, &cause).to_string(),
        ],
    )
}

#[test]
fn exception_cause() -> TestResult {
    let cause: Arc<dyn Error + Send + Sync> = Arc::new(JavaCause);
    let exception = SQLParsingException::from_cause(Some(Arc::clone(&cause)));
    assert_eq!(exception.message(), Some(cause.to_string().as_str()));
    let actual = exception.cause().ok_or("Missing cause")?;
    assert!(Arc::ptr_eq(actual, &cause));
    record(
        "SQLParsingExceptionTest",
        "testConstructorWithCause",
        &[
            exception.message().ok_or("Missing message")?.into(),
            Arc::ptr_eq(actual, &cause).to_string(),
        ],
    )
}

#[test]
fn sequence_default() -> TestResult {
    let expr = SqlSequenceExpr::default();
    assert!(expr.sequence().is_none());
    assert!(expr.function().is_none());
    record(
        "struct.SqlSequenceExprTest",
        "testDefaultConstructor",
        &[
            expr.sequence().is_none().to_string(),
            expr.function().is_none().to_string(),
        ],
    )
}

#[test]
fn sequence_constructor() -> TestResult {
    let expr = SqlSequenceExpr::new(Some("mySequence".into()), Some("myFunction".into()));
    assert_eq!(expr.sequence(), Some("mySequence"));
    assert_eq!(expr.function(), Some("myFunction"));
    record(
        "struct.SqlSequenceExprTest",
        "testParameterizedConstructor",
        &[
            expr.sequence().ok_or("Missing sequence")?.into(),
            expr.function().ok_or("Missing function")?.into(),
        ],
    )
}

#[test]
fn sequence_set_sequence() -> TestResult {
    let mut expr = SqlSequenceExpr::default();
    expr.set_sequence(Some("newSequence".into()));
    assert_eq!(expr.sequence(), Some("newSequence"));
    record(
        "struct.SqlSequenceExprTest",
        "testSetGetSequence",
        &[expr.sequence().ok_or("Missing sequence")?.into()],
    )
}

#[test]
fn sequence_set_function() -> TestResult {
    let mut expr = SqlSequenceExpr::default();
    expr.set_function(Some("newFunction".into()));
    assert_eq!(expr.function(), Some("newFunction"));
    record(
        "struct.SqlSequenceExprTest",
        "testSetGetFunction",
        &[expr.function().ok_or("Missing function")?.into()],
    )
}

#[test]
fn method_get() -> TestResult {
    let instance = SqlMethodExpr::get();
    assert_eq!(instance, SqlMethodExpr::get());
    record(
        "struct.SqlMethodExprTest",
        "testGet",
        &[(instance == SqlMethodExpr::get()).to_string()],
    )
}

#[test]
fn method_string() -> TestResult {
    let actual = SqlMethodExpr::get().to_string();
    assert_eq!(actual.trim(), "SQL_METHOD".trim());
    record(
        "struct.SqlMethodExprTest",
        "testToString",
        &[actual.trim().into()],
    )
}

#[test]
fn default_get() -> TestResult {
    let instance = SqlDefaultExpr::get();
    assert!(std::ptr::eq(instance, SqlDefaultExpr::get()));
    record(
        "struct.SqlDefaultExprTest",
        "testGet",
        &[std::ptr::eq(instance, SqlDefaultExpr::get()).to_string()],
    )
}

#[test]
fn default_string() -> TestResult {
    let actual = SqlDefaultExpr::get().to_string();
    assert_eq!(actual.trim(), "DEFAULT".trim());
    record(
        "struct.SqlDefaultExprTest",
        "testToString",
        &[actual.trim().into()],
    )
}

#[test]
fn null_get() -> TestResult {
    let instance = Null::get();
    assert!(std::ptr::eq(instance, Null::get()));
    record(
        "struct.NullTest",
        "testGet",
        &[std::ptr::eq(instance, Null::get()).to_string()],
    )
}

#[test]
fn null_string() -> TestResult {
    let actual = Null::get().to_string();
    assert_eq!(actual.trim(), "NULL".trim());
    record("struct.NullTest", "testToString", &[actual.trim().into()])
}

#[test]
fn not_placeholder_get() -> TestResult {
    let instance = NotPlaceholderExpr::get();
    assert!(std::ptr::eq(instance, NotPlaceholderExpr::get()));
    record(
        "struct.NotPlaceHolderExprTest",
        "testGet",
        &[std::ptr::eq(instance, NotPlaceholderExpr::get()).to_string()],
    )
}

#[test]
fn not_placeholder_string() -> TestResult {
    let actual = NotPlaceholderExpr::get().to_string();
    assert_eq!(actual, "NOT_PLACEHOLDER");
    record("struct.NotPlaceHolderExprTest", "testToString", &[actual])
}
