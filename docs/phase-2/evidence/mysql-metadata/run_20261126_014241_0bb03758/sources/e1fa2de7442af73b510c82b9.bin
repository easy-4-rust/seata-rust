//! 原TableMeta真实观察：主键HashMap树桶顺序、语言折叠及异常继承，不冒充额外源JUnit案例。
use seata_sqlparser::{
    ColumnList, ColumnMeta, IndexMeta, IndexType, JavaObject, JavaObjectError, JavaString,
    MetadataError, MetadataLocale, TableMeta,
};
use std::{
    any::Any,
    error::Error,
    fs::OpenOptions,
    io::Write,
    sync::{Arc, Mutex},
};
type TestResult = Result<(), Box<dyn Error>>;
static OUTPUT: Mutex<()> = Mutex::new(());

fn decode(text: &str) -> Result<Option<JavaString>, Box<dyn Error>> {
    if text == "null" {
        return Ok(None);
    }
    if text.len() % 4 != 0 {
        return Err("Malformed UTF16".into());
    }
    Ok(Some(JavaString::from_utf16(
        (0..text.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&text[i..i + 4], 16))
            .collect::<Result<Vec<_>, _>>()?,
    )))
}
fn encode(text: Option<&JavaString>) -> String {
    let Some(text) = text else {
        return "null".into();
    };
    let digits = b"0123456789abcdef";
    let mut out = String::with_capacity(text.as_utf16().len() * 4);
    for unit in text.as_utf16() {
        for shift in [12, 8, 4, 0] {
            out.push(char::from(digits[usize::from((unit >> shift) & 15)]));
        }
    }
    out
}
fn record(variable: &str, key: &str, values: &[String]) -> TestResult {
    if let Some(path) = std::env::var_os(variable) {
        let _guard = OUTPUT.lock().map_err(|_| "Output lock poisoned")?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(format!("{key}\t{}\n", values.join("\t")).as_bytes())?;
    }
    Ok(())
}
fn fixture(variable: &str, default: &str) -> Result<String, Box<dyn Error>> {
    match std::env::var_os(variable) {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None => Ok(default.into()),
    }
}

#[test]
fn primary_hash_iteration_matches_original_resize_treeify_root_front_split_and_null_keys(
) -> TestResult {
    let fixture = fixture(
        "SEATA_TABLE_HASH_ORACLE",
        include_str!("fixtures/sqlparser_core/primary-hash-java.tsv"),
    )?;
    let mut count = 0;
    for line in fixture.lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        let names = fields[1]
            .split(';')
            .map(decode)
            .collect::<Result<Vec<_>, _>>()?;
        let table = TableMeta::new(MetadataLocale::new("zh"));
        let index = Arc::new(IndexMeta::default());
        index.set_indextype(Some(IndexType::PRIMARY))?;
        let mut columns = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let col = ColumnMeta::default();
            col.set_column_name(name)?;
            col.set_data_type((i + 1) as i32)?;
            columns.push(Some(col));
        }
        index.set_values(Some(ColumnList::from_values(columns)))?;
        table
            .all_indexes()
            .put(Some("primary".into()), Some(index))?;
        let map = table.primary_key_map()?;
        let keys = map.keys();
        assert_eq!(
            keys,
            table.primary_key_only_name()?,
            "{} projection",
            fields[0]
        );
        let encoded = keys
            .iter()
            .map(|key| encode(key.as_ref()))
            .collect::<Vec<_>>()
            .join(";");
        let values = keys
            .iter()
            .map(|key| {
                map.get(key.as_ref())
                    .ok_or(MetadataError::NullReference("primary.value"))
                    .and_then(|col| col.data_type())
                    .map(|v| v.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(";");
        let actual = vec![encoded, values, map.len().to_string()];
        assert_eq!(actual, fields[2..], "{} input {}", fields[0], fields[1]);
        record("SEATA_TABLE_HASH_OBSERVATIONS", fields[0], &actual)?;
        count += 1;
    }
    assert_eq!(count, 283);
    Ok(())
}

#[test]
fn defaults_and_original_name_case_flag_equals_hash_quirks_match() -> TestResult {
    let data = fixture(
        "SEATA_TABLE_EDGE_ORACLE",
        include_str!("fixtures/sqlparser_core/table-edges-java-v2.tsv"),
    )?;
    let first = TableMeta::new(MetadataLocale::new("zh"));
    let second = TableMeta::new(MetadataLocale::new("zh"));
    let actual = vec![
        encode(first.table_name()?.as_ref()),
        encode(first.original_table_name()?.as_ref()),
        first.is_case_sensitive()?.to_string(),
        first.all_columns().len()?.to_string(),
        first.all_indexes().len()?.to_string(),
        first.java_hash_code()?.to_string(),
    ];
    let expected = data
        .lines()
        .find(|l| l.starts_with("table-default\t"))
        .ok_or("Missing default")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record("SEATA_TABLE_EDGE_OBSERVATIONS", "table-default", &actual)?;
    let mut identities = [0_i32; 4];
    for (index, kind) in IndexType::ALL.iter().enumerate() {
        let key = format!("enum-identity/{kind}");
        identities[index] = data
            .lines()
            .find_map(|line| line.strip_prefix(&(key.clone() + "\t")))
            .ok_or("Missing identity input")?
            .parse()?;
        record(
            "SEATA_TABLE_IDENTITY_INPUTS",
            &key,
            &[identities[index].to_string()],
        )?;
    }
    let locale = MetadataLocale::new("zh");
    let pair = [TableMeta::new(locale.clone()), TableMeta::new(locale)];
    let first_col = Arc::new(ColumnMeta::default());
    first_col.set_column_name(Some("col1".into()))?;
    first_col.set_on_update(true)?;
    let second_col = Arc::new(ColumnMeta::default());
    second_col.set_column_name(Some("col2".into()))?;
    let primary = Arc::new(IndexMeta::default());
    primary.set_indextype(Some(IndexType::PRIMARY))?;
    primary.set_values(Some(ColumnList::fixed(vec![
        Some(first_col.as_ref().clone()),
        Some(second_col.as_ref().clone()),
    ])))?;
    for current in &pair {
        current.set_table_name(Some("tableName".into()))?;
        current
            .all_columns()
            .put(Some("col1".into()), Some(first_col.clone()))?;
        current
            .all_columns()
            .put(Some("col2".into()), Some(second_col.clone()))?;
        current
            .all_indexes()
            .put(Some("primary".into()), Some(primary.clone()))?;
    }
    let hash = |table: &TableMeta| {
        table.java_hash_code_with_enum_identity(|kind| identities[kind.value() as usize])
    };
    let hash1 = hash(&pair[0])?;
    let hash2 = hash(&pair[1])?;
    pair[0].set_case_sensitive(true)?;
    let hash3 = hash(&pair[0])?;
    primary.set_cardinality(i64::MIN)?;
    let actual = vec![
        hash1.to_string(),
        hash2.to_string(),
        hash3.to_string(),
        hash(&pair[0])?.to_string(),
    ];
    let expected = data
        .lines()
        .find(|line| line.starts_with("table-fixture-hash\t"))
        .ok_or("Missing fixture hash")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record(
        "SEATA_TABLE_EDGE_OBSERVATIONS",
        "table-fixture-hash",
        &actual,
    )?;
    first.set_original_table_name(Some("one".into()))?;
    second.set_original_table_name(Some("two".into()))?;
    let same = first.java_equals(Some(&second))?;
    let hash = first.java_hash_code()? == second.java_hash_code()?;
    second.set_case_sensitive(true)?;
    let actual = vec![
        same.to_string(),
        hash.to_string(),
        first.java_equals(Some(&second))?.to_string(),
        first.java_hash_code()?.to_string(),
        second.java_hash_code()?.to_string(),
    ];
    let expected = data
        .lines()
        .find(|l| l.starts_with("ignored-original-and-case-hash\t"))
        .ok_or("Missing flag")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record(
        "SEATA_TABLE_EDGE_OBSERVATIONS",
        "ignored-original-and-case-hash",
        &actual,
    )?;
    Ok(())
}

#[test]
fn actual_language_specific_lowercase_map_keys_values_and_insertion_order_match() -> TestResult {
    let data = fixture(
        "SEATA_TABLE_EDGE_ORACLE",
        include_str!("fixtures/sqlparser_core/table-edges-java-v2.tsv"),
    )?;
    let mut count = 0;
    for line in data.lines().filter(|l| l.starts_with("map-language/")) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let language = fields[0]
            .strip_prefix("map-language/")
            .ok_or("Bad language")?;
        let table = TableMeta::new(MetadataLocale::new(language));
        for (i, key) in ["I", "i", "İ", "ı", "AภาษาΣ"].iter().enumerate() {
            let col = Arc::new(ColumnMeta::default());
            col.set_data_type((i + 1) as i32)?;
            table.all_columns().put(Some((*key).into()), Some(col))?;
        }
        let mut state = Vec::new();
        let mut cursor = table.all_columns().entries()?;
        while let Some((key, value)) = cursor.next_entry()? {
            state.push(format!(
                "{}:{}",
                encode(Some(&key)),
                value.ok_or("Null value")?.data_type()?
            ));
        }
        let actual = vec![state.join(";"), table.all_columns().len()?.to_string()];
        assert_eq!(actual, fields[1..], "{}", fields[0]);
        record("SEATA_TABLE_EDGE_OBSERVATIONS", fields[0], &actual)?;
        count += 1;
    }
    assert_eq!(count, 5);
    Ok(())
}

#[derive(Debug)]
struct ThrowingValue(Arc<JavaObjectError>);
impl JavaObject for ThrowingValue {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        Err(MetadataError::ObjectException(self.0.clone()))
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Ok(7)
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some("throwing".into()))
    }
}

#[test]
fn original_exception_hierarchy_inputs_preserve_direct_identity_and_map_catch_behavior(
) -> TestResult {
    let data = fixture(
        "SEATA_TABLE_EDGE_ORACLE",
        include_str!("fixtures/sqlparser_core/table-edges-java-v2.tsv"),
    )?;
    let mut count = 0;
    for line in data.lines().filter(|l| l.starts_with("hierarchy/")) {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 7);
        let failure = Arc::new(
            JavaObjectError::new(
                fields[1],
                Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])),
            )
            .with_superclasses(fields[2].split(';')),
        );
        let mine = TableMeta::new(MetadataLocale::new("zh"));
        let theirs = TableMeta::new(MetadataLocale::new("zh"));
        let left = Arc::new(ColumnMeta::default());
        let right = Arc::new(ColumnMeta::default());
        left.set_char_octet_length(Some(Arc::new(ThrowingValue(failure.clone()))))?;
        right.set_char_octet_length(Some(Arc::new(1_i32)))?;
        mine.all_columns()
            .put(Some("c".into()), Some(left.clone()))?;
        theirs
            .all_columns()
            .put(Some("c".into()), Some(right.clone()))?;
        let direct = right
            .java_equals(Some(left.as_ref()))
            .err()
            .ok_or("Missing direct exception")?;
        let outcome = match mine.java_equals(Some(&theirs)) {
            Ok(value) => format!("B:{value}"),
            Err(error) => format!(
                "E:{}",
                error.java_exception_class().ok_or("Unmapped exception")?
            ),
        };
        let actual = vec![
            direct.java_exception_class().ok_or("Missing class")?.into(),
            encode(direct.java_message()),
            Arc::ptr_eq(
                direct.object_exception().ok_or("Missing identity")?,
                &failure,
            )
            .to_string(),
            outcome,
        ];
        assert_eq!(actual, fields[3..], "{}", fields[0]);
        record("SEATA_TABLE_EDGE_OBSERVATIONS", fields[0], &actual)?;
        count += 1;
    }
    assert_eq!(count, 4);
    Ok(())
}
