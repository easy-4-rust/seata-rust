//! Druid 驱动名与原 RM 资源 ID 的 Java 差分。
use seata_db_mysql::JdbcUtils;
use seata_sqlparser::{DruidJdbcDriverResolver, JavaString};
use std::{cell::Cell, collections::BTreeSet, error::Error};

fn decode(input: &str) -> Result<Option<JavaString>, Box<dyn Error>> {
    if input == "null" {
        return Ok(None);
    }
    assert_eq!(input.len() % 4, 0);
    let units = (0..input.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&input[i..i + 4], 16))
        .collect::<Result<_, _>>()?;
    Ok(Some(JavaString::from_utf16(units)))
}

fn hex(value: Option<&JavaString>) -> String {
    let Some(value) = value else {
        return "null".into();
    };
    let mut text = String::with_capacity(value.as_utf16().len() * 4);
    for unit in value.as_utf16() {
        for shift in [12, 8, 4, 0] {
            text.push(char::from(
                b"0123456789abcdef"[usize::from((unit >> shift) & 15)],
            ));
        }
    }
    text
}

#[test]
fn driver_profiles_and_resource_identity_match_original_jdbc() -> Result<(), Box<dyn Error>> {
    for (present, fixture) in [
        (false, include_str!("fixtures/jdbc-driver-absent-java.tsv")),
        (true, include_str!("fixtures/jdbc-driver-present-java.tsv")),
    ] {
        let resolver = DruidJdbcDriverResolver::default();
        let probes = Cell::new(0);
        let mut seen = BTreeSet::new();
        for row in fixture.lines() {
            let mut fields = row.split('\t');
            let input = fields.next().ok_or("missing input")?;
            let driver = fields.next().ok_or("missing driver")?;
            let resource = fields.next().ok_or("missing resource id")?;
            assert!(fields.next().is_none());
            assert!(seen.insert(input));
            if input == "cached" {
                let mysql =
                    resolver.get_driver_class_name(Some(&"jdbc:mysql:".into()), |_| !present)?;
                let clickhouse = resolver
                    .get_driver_class_name(Some(&"jdbc:clickhouse:".into()), |_| !present)?;
                assert_eq!(mysql, Some(JavaString::from(driver)));
                assert_eq!(clickhouse, Some(JavaString::from(resource)));
                continue;
            }
            let value = decode(input)?;
            let result = resolver.get_driver_class_name(value.as_ref(), |name| {
                assert!(matches!(
                    name,
                    "com.mysql.cj.jdbc.Driver" | "com.clickhouse.jdbc.ClickHouseDriver"
                ));
                probes.set(probes.get() + 1);
                present
            });
            let actual = match result {
                Ok(value) => hex(value.as_ref()),
                Err(error) => format!(
                    "{}:{}",
                    error.java_exception_class().ok_or("missing class")?,
                    hex(error.java_message())
                ),
            };
            assert_eq!(actual, driver, "{present}: {input}");
            let actual = match JdbcUtils::build_resource_id(value.as_ref()) {
                Ok(id) => format!(
                    "{}:{}",
                    hex(Some(&id)),
                    value.as_ref().is_some_and(|url| id.same_storage(url))
                ),
                Err(error) => error.java_exception_class().ok_or("missing class")?.into(),
            };
            assert_eq!(actual, resource, "{present}: {input}");
        }
        assert_eq!(seen.len(), 466);
        assert_eq!(probes.get(), 2);
    }
    Ok(())
}

#[test]
fn outer_probe_overwrites_inner_cached_driver_like_original_jvm() -> Result<(), Box<dyn Error>> {
    let resolver = DruidJdbcDriverResolver::default();
    let mut observed = String::new();
    let mut inner = None;
    for (url, class) in [
        ("jdbc:mysql:", "com.mysql.cj.jdbc.Driver"),
        ("jdbc:clickhouse:", "com.clickhouse.jdbc.ClickHouseDriver"),
    ] {
        let value = JavaString::from(url);
        let outer = resolver.get_driver_class_name(Some(&value), |name| {
            assert_eq!(name, class);
            inner = Some(resolver.get_driver_class_name(Some(&value), |_| false));
            true
        })?;
        let nested = inner
            .take()
            .ok_or("missing nested result")??
            .ok_or("missing nested driver")?;
        let outer = outer.ok_or("missing outer driver")?;
        let later = resolver
            .get_driver_class_name(Some(&value), |_| false)?
            .ok_or("missing cached driver")?;
        use std::fmt::Write;
        writeln!(observed, "{class}\tinner\t{nested}")?;
        writeln!(observed, "{url}\touter\t{outer}")?;
        writeln!(observed, "{url}\tlater\t{later}")?;
    }
    assert_eq!(
        observed,
        include_str!("fixtures/jdbc-driver-reentry-java.tsv").replace("\r\n", "\n")
    );
    Ok(())
}
