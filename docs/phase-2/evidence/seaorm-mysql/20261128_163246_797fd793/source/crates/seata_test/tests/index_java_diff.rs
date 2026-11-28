//! 固定原Java148项观察：集合双向交互、backing换代、枚举身份上下文及异常身份。
use seata_sqlparser::{
    ColumnList, ColumnMeta, IndexMeta, IndexType, JavaObject, JavaObjectError, JavaString,
    JavaThrowable, MetadataError, SQLParsingException,
};
use std::{
    any::Any,
    collections::BTreeMap,
    error::Error,
    fmt,
    io::Write,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
type TestResult = Result<(), Box<dyn Error>>;
static OUTPUT: Mutex<()> = Mutex::new(());

fn fixture() -> Result<BTreeMap<String, Vec<String>>, Box<dyn Error>> {
    let text = if let Some(path) = std::env::var_os("SEATA_INDEX_ORACLE_FIXTURE") {
        std::fs::read_to_string(path)?
    } else {
        include_str!("fixtures/sqlparser_core/index-edges-java.tsv").to_owned()
    };
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let key = fields.next().ok_or("Missing edge key")?;
        assert!(rows
            .insert(key.to_owned(), fields.map(str::to_owned).collect())
            .is_none());
    }
    assert_eq!(rows.len(), 148);
    Ok(rows)
}

fn check(key: &str, actual: &[String]) -> TestResult {
    let expected = fixture()?;
    assert_eq!(
        actual,
        expected.get(key).ok_or("Missing original edge")?,
        "Java mismatch: {key}"
    );
    if let Some(path) = std::env::var_os("SEATA_INDEX_EDGE_OBSERVATIONS") {
        let _guard = OUTPUT.lock().map_err(|_| "Observation lock poisoned")?;
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(out, "{key}\t{}", actual.join("\t"))?;
    }
    Ok(())
}

fn units(value: &JavaString) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    for unit in value.as_utf16() {
        for byte in unit.to_be_bytes() {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    text
}

fn column(name: &str) -> Result<ColumnMeta, MetadataError> {
    let value = ColumnMeta::default();
    value.set_column_name(Some(name.into()))?;
    Ok(value)
}

fn list(mode: &str) -> Result<ColumnList, MetadataError> {
    let values = vec![Some(column("first")?), Some(column("second")?)];
    match mode {
        "F" => Ok(ColumnList::fixed(values)),
        "A" => Ok(ColumnList::from_values(values)),
        _ => {
            let result = ColumnList::default();
            for value in values {
                result.push(value)?;
            }
            Ok(result)
        }
    }
}

fn names(values: &ColumnList) -> Result<String, Box<dyn Error>> {
    let mut result = Vec::new();
    for i in 0..values.len()? {
        result.push(match values.get(i32::try_from(i)?)? {
            Some(value) => value.column_name()?.ok_or("Missing name")?.to_string(),
            None => "null".into(),
        });
    }
    Ok(result.join(","))
}

fn release(values: &ColumnList) -> Result<(), Box<dyn Error>> {
    // 测试回调捕获列表句柄；在锁外释放循环引用，固定列表也允许set。
    for i in 0..values.len()? {
        drop(values.set(i32::try_from(i)?, None)?);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Action {
    name: &'static str,
    left: ColumnList,
    right: ColumnList,
}
impl Action {
    fn run(&self) -> Result<(), MetadataError> {
        match self.name {
            "clear-left" => self.left.clear()?,
            "clear-right" => self.right.clear()?,
            "clear-both" => {
                self.left.clear()?;
                self.right.clear()?;
            }
            "set-left" => {
                self.left.set(1, Some(column("changed")?))?;
            }
            "set-right" => {
                self.right.set(1, Some(column("changed")?))?;
            }
            "null-left" => {
                self.left.set(1, None)?;
            }
            "null-right" => {
                self.right.set(1, None)?;
            }
            "remove-left" => {
                self.left.remove(1)?;
            }
            "remove-right" => {
                self.right.remove(1)?;
            }
            "grow-left-set" => {
                self.left.push(Some(column("extra")?))?;
                self.left.set(1, Some(column("changed")?))?;
            }
            "grow-right-set" => {
                self.right.push(Some(column("extra")?))?;
                self.right.set(1, Some(column("changed")?))?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Probe {
    action: Action,
    count: Arc<AtomicUsize>,
    fired: AtomicBool,
}
impl Probe {
    fn fire(&self) -> Result<(), MetadataError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        if !self.fired.swap(true, Ordering::SeqCst) {
            self.action.run()?;
        }
        Ok(())
    }
}
impl JavaObject for Probe {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> Result<bool, MetadataError> {
        self.fire()?;
        Ok(other.as_any().is::<Self>())
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Ok(11)
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        self.fire()?;
        Ok(Some("callback".into()))
    }
}

fn set_probe(
    values: &ColumnList,
    index: i32,
    action: Action,
    count: Arc<AtomicUsize>,
) -> Result<(), Box<dyn Error>> {
    values
        .get(index)?
        .ok_or("Missing probe column")?
        .set_char_octet_length(Some(Arc::new(Probe {
            action,
            count,
            fired: AtomicBool::new(false),
        })))?;
    Ok(())
}

#[test]
fn all_list_equals_modes_and_mutations_match_jdk17() -> TestResult {
    let actions = [
        "none",
        "clear-left",
        "clear-right",
        "clear-both",
        "set-left",
        "set-right",
        "null-left",
        "null-right",
        "remove-left",
        "remove-right",
        "grow-left-set",
        "grow-right-set",
    ];
    for lm in ["A", "D", "F"] {
        for rm in ["A", "D", "F"] {
            for op in actions {
                let left = list(lm)?;
                let right = list(rm)?;
                let count = Arc::new(AtomicUsize::new(0));
                let noop = Action {
                    name: "none",
                    left: left.clone(),
                    right: right.clone(),
                };
                let action = Action {
                    name: op,
                    ..noop.clone()
                };
                set_probe(&left, 0, noop.clone(), Arc::new(AtomicUsize::new(0)))?;
                set_probe(&right, 0, action, Arc::new(AtomicUsize::new(0)))?;
                set_probe(&left, 1, noop.clone(), Arc::new(AtomicUsize::new(0)))?;
                set_probe(&right, 1, noop, Arc::clone(&count))?;
                let result = match left.java_equals(&right) {
                    Ok(value) => format!("B:{value}"),
                    Err(error) => format!(
                        "E:{}",
                        error.java_exception_class().ok_or("Non-Java error")?
                    ),
                };
                let actual = [
                    result,
                    left.len()?.to_string(),
                    right.len()?.to_string(),
                    count.load(Ordering::SeqCst).to_string(),
                    names(&left)?,
                    names(&right)?,
                ];
                release(&left)?;
                release(&right)?;
                check(&format!("eq/{lm}/{rm}/{op}"), &actual)?;
            }
        }
    }
    Ok(())
}

#[test]
fn live_to_string_iteration_modes_match_jdk17() -> TestResult {
    for mode in ["A", "D", "F"] {
        for op in [
            "none",
            "clear-left",
            "remove-left",
            "null-left",
            "set-left",
            "grow-left-set",
        ] {
            let values = list(mode)?;
            let count = Arc::new(AtomicUsize::new(0));
            let action = Action {
                name: op,
                left: values.clone(),
                right: values.clone(),
            };
            set_probe(&values, 0, action.clone(), Arc::new(AtomicUsize::new(0)))?;
            set_probe(
                &values,
                1,
                Action {
                    name: "none",
                    ..action
                },
                Arc::clone(&count),
            )?;
            let result = match values.java_to_string() {
                Ok(value) => format!("S:{}", units(&value)),
                Err(error) => format!(
                    "E:{}",
                    error.java_exception_class().ok_or("Non-Java error")?
                ),
            };
            let actual = [
                result,
                values.len()?.to_string(),
                count.load(Ordering::SeqCst).to_string(),
                names(&values)?,
            ];
            release(&values)?;
            check(&format!("text/{mode}/{op}"), &actual)?;
        }
    }
    Ok(())
}

fn index_fixture() -> Result<IndexMeta, MetadataError> {
    let value = IndexMeta::default();
    value.set_values(Some(ColumnList::fixed(vec![
        Some(ColumnMeta::default()),
        Some(ColumnMeta::default()),
    ])))?;
    value.set_non_unique(true)?;
    value.set_index_qualifier(Some("qualifier".into()))?;
    value.set_index_name(Some("indexName".into()))?;
    value.set_type(1)?;
    value.set_indextype(Some(IndexType::PRIMARY))?;
    value.set_asc_or_desc(Some("A".into()))?;
    value.set_ordinal_position(1)?;
    Ok(value)
}

#[test]
fn nullable_defaults_and_hash_replay_with_explicit_jvm_identity_match() -> TestResult {
    let left = IndexMeta::default();
    let right = IndexMeta::default();
    check(
        "index-default",
        &[
            left.java_hash_code()?.to_string(),
            units(&left.java_to_string()?),
        ],
    )?;
    let error = left
        .java_equals(Some(&right))
        .err()
        .ok_or("Missing null-type error")?;
    check(
        "index-null-type",
        &[error
            .java_exception_class()
            .ok_or("Missing Java class")?
            .into()],
    )?;
    right.set_values(None)?;
    check(
        "index-null-list-shortcut",
        &[left.java_equals(Some(&right))?.to_string()],
    )?;
    left.set_values(None)?;
    left.set_indextype(Some(IndexType::PRIMARY))?;
    right.set_indextype(Some(IndexType::PRIMARY))?;
    left.set_cardinality(i64::MIN)?;
    right.set_cardinality(i64::MAX)?;
    check(
        "index-ignored-cardinality",
        &[
            left.java_equals(Some(&right))?.to_string(),
            (left.java_hash_code()? == right.java_hash_code()?).to_string(),
        ],
    )?;
    check("index-null-list-text", &[units(&left.java_to_string()?)])?;
    for (suffix, type_code, cardinality) in
        [("min", i16::MIN, i64::MIN), ("max", i16::MAX, i64::MAX)]
    {
        left.set_type(type_code)?;
        left.set_cardinality(cardinality)?;
        check(
            &format!("index-wide-{suffix}"),
            &[left.r#type()?.to_string(), left.cardinality()?.to_string()],
        )?;
    }
    let data = fixture()?;
    for variant in IndexType::ALL {
        let key = format!("enum-identity/{variant}");
        let identity: i32 = data.get(&key).ok_or("Missing JVM identity")?[0].parse()?;
        // 身份值是显式输入，不是本机计算结果；原始输出hash另行由被测算法计算比较。
        if let Some(path) = std::env::var_os("SEATA_INDEX_IDENTITY_INPUTS") {
            let _guard = OUTPUT.lock().map_err(|_| "Observation lock poisoned")?;
            writeln!(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?,
                "{key}\t{identity}"
            )?;
        }
        let value = index_fixture()?;
        value.set_indextype(Some(variant))?;
        check(
            &format!("index-hash/{variant}"),
            &[value
                .java_hash_code_with_enum_identity(|_| identity)?
                .to_string()],
        )?;
    }
    Ok(())
}

#[derive(Debug)]
struct Failing(Arc<JavaObjectError>);
impl fmt::Display for Failing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failing fixture")
    }
}
impl Error for Failing {}
impl JavaObject for Failing {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
}
impl JavaThrowable for Failing {
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
}

#[test]
fn original_exception_identity_propagates_through_all_seven_paths() -> TestResult {
    let original = Arc::new(JavaObjectError::new(
        "java.lang.IllegalStateException",
        Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])),
    ));
    let good = ColumnMeta::default();
    good.set_char_octet_length(Some(Arc::new(1_i32)))?;
    let bad = ColumnMeta::default();
    bad.set_char_octet_length(Some(Arc::new(Failing(Arc::clone(&original)))))?;
    let index = IndexMeta::default();
    index.set_values(Some(ColumnList::fixed(vec![Some(bad.clone())])))?;
    let right = IndexMeta::default();
    right.set_values(Some(ColumnList::fixed(vec![Some(good.clone())])))?;
    let errors = [
        good.java_equals(Some(&bad)).err(),
        bad.java_hash_code().err(),
        bad.java_to_string().err(),
        ColumnList::fixed(vec![Some(bad)]).java_to_string().err(),
        index.java_to_string().err(),
        index.java_equals(Some(&right)).err(),
        SQLParsingException::from_java_cause(Some(Arc::new(Failing(Arc::clone(&original))))).err(),
    ];
    for (i, error) in errors.into_iter().enumerate() {
        let error = error.ok_or("Original failure swallowed")?;
        let actual = error.object_exception().ok_or("Original exception lost")?;
        check(
            &format!("failure/{i}"),
            &[
                actual.class_name().into(),
                units(actual.message_java().ok_or("Message lost")?),
                Arc::ptr_eq(actual, &original).to_string(),
            ],
        )?;
    }
    Ok(())
}
