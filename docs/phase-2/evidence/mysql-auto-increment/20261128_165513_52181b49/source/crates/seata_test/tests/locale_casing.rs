//! TableMeta默认Locale依赖的实际JDK17上下文观察；不扩充原76案例分母。
use seata_sqlparser::{JavaString, JdkCasing};
use std::{error::Error, fs::OpenOptions, io::Write};

type TestResult = Result<(), Box<dyn Error>>;

fn decode(text: &str) -> Result<JavaString, Box<dyn Error>> {
    if text.len() % 4 != 0 {
        return Err("Invalid UTF16 fixture".into());
    }
    Ok(JavaString::from_utf16(
        (0..text.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&text[i..i + 4], 16))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn encode(text: &JavaString) -> String {
    let digits = b"0123456789abcdef";
    let mut result = String::with_capacity(text.as_utf16().len() * 4);
    for unit in text.as_utf16() {
        for shift in [12, 8, 4, 0] {
            result.push(char::from(digits[usize::from((unit >> shift) & 15)]));
        }
    }
    result
}

fn record(variable: &str, key: &str, values: &[String]) -> TestResult {
    if let Some(path) = std::env::var_os(variable) {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(format!("{key}\t{}\n", values.join("\t")).as_bytes())?;
    }
    Ok(())
}

#[test]
fn contextual_string_lower_and_upper_match_original_jdk17() -> TestResult {
    let mut count = 0;
    for line in include_str!("fixtures/sqlparser_core/jdk17-locale-observations.tsv").lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        let (language, _) = fields[0]
            .strip_prefix("case/")
            .ok_or("Invalid key")?
            .split_once('/')
            .ok_or("Invalid language")?;
        let input = decode(fields[1])?;
        let actual = vec![
            encode(&JdkCasing::lower_case(&input, language)?),
            encode(&JdkCasing::upper_case(&input, language)?),
        ];
        assert_eq!(actual, fields[2..4], "{} input {}", fields[0], fields[1]);
        record("SEATA_LOCALE_CASING_OBSERVATIONS", fields[0], &actual)?;
        count += 1;
    }
    assert_eq!(count, 12384);
    Ok(())
}

#[test]
fn word_boundaries_match_original_root_and_thai_dictionary_profiles() -> TestResult {
    let mut count = 0;
    for line in include_str!("fixtures/sqlparser_core/jdk17-locale-observations.tsv").lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        let (language, _) = fields[0]
            .strip_prefix("case/")
            .ok_or("Invalid key")?
            .split_once('/')
            .ok_or("Invalid language")?;
        let flags = JdkCasing::word_boundaries(&decode(fields[1])?, language)?;
        let actual = flags
            .iter()
            .map(|flag| if *flag { '1' } else { '0' })
            .collect::<String>();
        assert_eq!(actual, fields[4], "{} input {}", fields[0], fields[1]);
        record("SEATA_LOCALE_BOUNDARY_OBSERVATIONS", fields[0], &[actual])?;
        count += 1;
    }
    assert_eq!(count, 12384);
    Ok(())
}
