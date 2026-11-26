//! TableMeta依赖的原JDK17比较器与不可变主键集合；补充观察不计入76个原案例。
use seata_sqlparser::{CaseInsensitivePkSet, JavaString, MetadataError};
use std::{error::Error, fs::OpenOptions, io::Write, sync::Mutex};

static OUTPUT: Mutex<()> = Mutex::new(());

type TestResult = Result<(), Box<dyn Error>>;

fn decode(text: &str) -> Result<JavaString, Box<dyn Error>> {
    if text.len() % 4 != 0 {
        return Err("Invalid UTF16 fixture".into());
    }
    let units = (0..text.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&text[i..i + 4], 16))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(JavaString::from_utf16(units))
}

fn encode(text: &JavaString) -> String {
    let digits = b"0123456789abcdef";
    let mut encoded = String::with_capacity(text.as_utf16().len() * 4);
    for unit in text.as_utf16() {
        for shift in [12, 8, 4, 0] {
            encoded.push(char::from(digits[usize::from((unit >> shift) & 15)]));
        }
    }
    encoded
}

fn record(key: &str, values: &[String]) -> TestResult {
    if let Some(path) = std::env::var_os("SEATA_TABLE_SET_OBSERVATIONS") {
        let _guard = OUTPUT.lock().map_err(|_| "Observation lock poisoned")?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{key}\t{}", values.join("\t"))?;
    }
    Ok(())
}

fn boolean(value: Result<bool, MetadataError>) -> Result<String, Box<dyn Error>> {
    match value {
        Ok(v) => Ok(v.to_string()),
        Err(e) => Ok(e.java_exception_class().ok_or("Unmapped error")?.into()),
    }
}

#[test]
fn all_java17_comparisons_match_including_raw_surrogates() -> TestResult {
    let data = include_str!("fixtures/sqlparser_core/java17-comparisons.tsv");
    let mut count = 0;
    for (i, line) in data.lines().enumerate() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        let actual = decode(fields[0])?.java_compare_to_ignore_case(&decode(fields[1])?);
        assert_eq!(
            actual,
            fields[2].parse::<i32>()?,
            "comparison {i}: {} vs {}",
            fields[0],
            fields[1]
        );
        record(&format!("comparison/{i}"), &[actual.to_string()])?;
        count += 1;
    }
    assert_eq!(count, 22492);
    Ok(())
}

#[test]
fn original_table_primary_sets_match_order_representative_hash_and_errors() -> TestResult {
    let data = include_str!("fixtures/sqlparser_core/table-primary-sets-v2.tsv");
    let mut count = 0;
    for line in data.lines().filter(|line| line.starts_with("set/")) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let input = fields[1]
            .split(';')
            .map(|text| {
                if text == "null" {
                    Ok(None)
                } else {
                    decode(text).map(Some)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = CaseInsensitivePkSet::from_names(input.clone());
        if fields[2] != "OK" {
            let error = result.err().ok_or("Expected null key failure")?;
            assert_eq!(error.java_exception_class(), Some(fields[2]));
            record(
                fields[0],
                &[
                    fields[1].into(),
                    error.java_exception_class().ok_or("Unmapped")?.into(),
                ],
            )?;
        } else {
            let set = result?;
            let names = set.names().iter().map(encode).collect::<Vec<_>>().join(";");
            let mut probes = input
                .iter()
                .map(|name| boolean(set.contains(name.as_ref())))
                .collect::<Result<Vec<_>, _>>()?;
            probes.push(boolean(set.contains(None))?);
            probes.push(boolean(set.contains(Some(&"missing".into())))?);
            let errors = [
                boolean(set.add(None))?,
                boolean(set.remove(Some(&"missing".into())))?,
                set.clear()
                    .err()
                    .ok_or("clear must reject")?
                    .java_exception_class()
                    .ok_or("Unmapped")?
                    .into(),
                boolean(set.add_all(&[]))?,
            ];
            let actual = vec![
                fields[1].into(),
                "OK".into(),
                names,
                set.len().to_string(),
                set.java_hash_code().to_string(),
                probes.join(";"),
                errors.join(";"),
            ];
            assert_eq!(actual, fields[1..], "{}", fields[0]);
            record(fields[0], &actual)?;
        }
        count += 1;
    }
    assert_eq!(count, 839);
    Ok(())
}

#[test]
fn unmodifiable_empty_and_case_equality_keep_java_hash_inconsistency() -> TestResult {
    let data = include_str!("fixtures/sqlparser_core/table-primary-sets-v2.tsv");
    let empty = CaseInsensitivePkSet::from_names(vec![])?;
    assert!(empty.is_empty());
    let actual = vec![
        boolean(empty.contains(None))?,
        boolean(empty.add(None))?,
        boolean(empty.remove(Some(&"missing".into())))?,
        empty
            .clear()
            .err()
            .ok_or("clear must reject")?
            .java_exception_class()
            .ok_or("Unmapped")?
            .into(),
        boolean(empty.add_all(&[]))?,
    ];
    let expected = data
        .lines()
        .find(|line| line.starts_with("empty\t"))
        .ok_or("Missing empty row")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record("empty", &actual)?;
    let upper = CaseInsensitivePkSet::from_names(vec![Some("A".into())])?;
    let lower = CaseInsensitivePkSet::from_names(vec![Some("a".into())])?;
    let actual = vec![
        upper.java_equals(&lower).to_string(),
        lower.java_equals(&upper).to_string(),
        upper.java_hash_code().to_string(),
        lower.java_hash_code().to_string(),
    ];
    let expected = data
        .lines()
        .find(|line| line.starts_with("set-equality\t"))
        .ok_or("Missing equality row")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record("set-equality", &actual)?;
    let b = JavaString::from_utf16(vec![0xd800, 0xdc00]);
    let c = JavaString::from_utf16(vec![0xd800, 0xd800, 0xdc00]);
    let d = JavaString::from_utf16(vec![0xe000]);
    let actual = vec![
        b.java_compare_to_ignore_case(&c).to_string(),
        c.java_compare_to_ignore_case(&d).to_string(),
        d.java_compare_to_ignore_case(&b).to_string(),
    ];
    let expected = data
        .lines()
        .find(|line| line.starts_with("comparator-cycle\t"))
        .ok_or("Missing cycle row")?
        .split('\t')
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    record("comparator-cycle", &actual)?;
    let permutations = [
        [&b, &c, &d],
        [&b, &d, &c],
        [&c, &b, &d],
        [&c, &d, &b],
        [&d, &c, &b],
        [&d, &b, &c],
    ];
    let sets = permutations
        .iter()
        .map(|names| {
            CaseInsensitivePkSet::from_names(
                names.iter().map(|name| Some((*name).clone())).collect(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for line in data
        .lines()
        .filter(|line| line.starts_with("cycle-equality/"))
    {
        let (key, value) = line.split_once('\t').ok_or("Invalid cycle equality")?;
        let indices = key
            .split('/')
            .skip(1)
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()?;
        let actual = sets[indices[0]].java_equals(&sets[indices[1]]).to_string();
        assert_eq!(actual, value, "{key}");
        record(key, &[actual])?;
    }
    Ok(())
}
