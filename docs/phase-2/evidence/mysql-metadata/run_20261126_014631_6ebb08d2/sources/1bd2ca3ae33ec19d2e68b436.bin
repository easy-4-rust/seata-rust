//! 原 Java 方言选择、列名与 TableMeta 主键转义的差分验收。
use seata_db_mysql::MySQLEscapeHandler;
use seata_sqlparser::{
    ColumnList, ColumnMeta, ColumnUtils, EscapeHandlerFactory, IndexMeta, IndexType, JavaString,
    MetadataError, MetadataLocale, TableMeta,
};
use std::{collections::BTreeMap, error::Error, sync::Arc};

fn name(value: &str) -> Option<JavaString> {
    Some(value.into())
}

fn text(value: Option<JavaString>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn list(values: Option<Vec<Option<JavaString>>>) -> String {
    values.map_or_else(
        || "null".into(),
        |values| {
            format!(
                "[{}]",
                values.into_iter().map(text).collect::<Vec<_>>().join(", ")
            )
        },
    )
}

fn observe<T>(result: Result<T, MetadataError>, success: impl FnOnce(T) -> String) -> String {
    result.map_or_else(
        |error| {
            error
                .java_exception_class()
                .map_or_else(|| error.to_string(), str::to_owned)
        },
        success,
    )
}

#[test]
fn original_mysql_provider_and_table_primary_key_escape_match() -> Result<(), Box<dyn Error>> {
    let locale = MetadataLocale::new("en");
    let factory = EscapeHandlerFactory::discover(locale.clone())?;
    let db = JavaString::from("mysql");
    let unknown = JavaString::from("invalid");
    let empty = JavaString::from("");
    let spaced = JavaString::from(" mysql ");
    let handler = factory.get_escape_handler(Some(&db))?;
    let same = factory.get_escape_handler(Some(&"MYSQL".into()))?;
    let mut actual = BTreeMap::new();
    actual.insert(
        "same-provider".into(),
        Arc::ptr_eq(&handler, &same).to_string(),
    );
    for (key, name) in [
        ("unknown", Some(&unknown)),
        ("empty-db", Some(&empty)),
        ("null-db", None),
        ("untrimmed-db", Some(&spaced)),
    ] {
        actual.insert(
            key.into(),
            observe(factory.get_escape_handler(name), |_| {
                "unexpected provider".into()
            }),
        );
    }
    for word in include_str!("../src/mysql/mysql_keywords.txt").lines() {
        let upper = handler.check_if_key_words(name(word).as_ref())?;
        let lower = handler.check_if_key_words(name(&word.to_ascii_lowercase()).as_ref())?;
        actual.insert(format!("kw/{word}"), format!("{upper},{lower}"));
    }
    actual.insert(
        "add-blank".into(),
        observe(
            ColumnUtils::add_escape(name(" "), Some(&unknown), None, &factory),
            text,
        ),
    );
    actual.insert(
        "delete-blank".into(),
        observe(
            ColumnUtils::del_escape(name(" "), Some(&unknown), &factory),
            text,
        ),
    );
    actual.insert(
        "add-empty-list".into(),
        ColumnUtils::add_escape_list(Some(vec![]), Some(&unknown), &factory)?
            .is_some_and(|items| items.is_empty())
            .to_string(),
    );
    actual.insert(
        "add-null-list".into(),
        list(ColumnUtils::add_escape_list(
            None,
            Some(&unknown),
            &factory,
        )?),
    );
    actual.insert(
        "add-list".into(),
        list(ColumnUtils::add_escape_list(
            Some(vec![
                name("select"),
                name("sch.order"),
                None,
                name("id"),
                name("`select`"),
            ]),
            Some(&db),
            &factory,
        )?),
    );
    actual.insert(
        "delete-list".into(),
        list(ColumnUtils::del_escape_list(
            Some(vec![name("`sch`.`order`"), None, name("id")]),
            Some(&db),
            &factory,
        )?),
    );
    for (key, input) in [
        ("framing", "one.two.order"),
        ("nbsp", "\u{a0}select"),
        ("locale-en", "insert"),
    ] {
        actual.insert(
            key.into(),
            text(ColumnUtils::add_escape(
                name(input),
                Some(&db),
                None,
                &factory,
            )?),
        );
    }
    locale.set_language("tr")?;
    actual.insert(
        "locale-tr".into(),
        text(ColumnUtils::add_escape(
            name("insert"),
            Some(&db),
            None,
            &factory,
        )?),
    );
    locale.set_language("en")?;
    let table = TableMeta::new(locale);
    table.set_table_name(name("events"))?;
    actual.insert(
        "missing-pk-before-missing-provider".into(),
        observe(table.escape_pk_name_list(Some(&unknown), &factory), |v| {
            list(Some(v))
        }),
    );
    let index = Arc::new(IndexMeta::default());
    index.set_indextype(Some(IndexType::PRIMARY))?;
    let mut columns = Vec::new();
    for value in [name("select"), name("schema.order"), None, name("id")] {
        let column = ColumnMeta::default();
        column.set_column_name(value)?;
        columns.push(Some(column));
    }
    index.set_values(Some(ColumnList::from_values(columns)))?;
    table.all_indexes().put(name("primary"), Some(index))?;
    actual.insert(
        "table-entrypoint".into(),
        list(Some(table.escape_pk_name_list(Some(&db), &factory)?)),
    );

    let mut expected = BTreeMap::new();
    for row in include_str!("fixtures/escape-pk-java.tsv").lines() {
        let (key, value) = row.split_once('\t').ok_or("invalid oracle row")?;
        assert!(expected.insert(key.to_owned(), value.to_owned()).is_none());
    }
    assert_eq!(expected.len(), 280);
    for (key, value) in &expected {
        assert_eq!(actual.get(key), Some(value), "oracle record {key}");
    }
    assert_eq!(actual.len(), expected.len());

    Ok(())
}

#[test]
fn provider_cache_uses_raw_name_and_retries_lookup_after_locale_change(
) -> Result<(), Box<dyn Error>> {
    let locale = MetadataLocale::new("en");
    let provider = Arc::new(MySQLEscapeHandler::new(locale.clone()));
    let factory = EscapeHandlerFactory::new(
        locale.clone(),
        [(
            JavaString::from("mariadb"),
            provider as Arc<dyn seata_sqlparser::EscapeHandler + Send + Sync>,
        )],
    )?;
    let cached = JavaString::from("MARIADB");
    let unseen = JavaString::from("MARIADb");
    let before = factory.get_escape_handler(Some(&cached))?;

    locale.set_language("tr")?;
    let after = factory.get_escape_handler(Some(&cached))?;
    assert!(Arc::ptr_eq(&before, &after));
    let unknown = factory
        .get_escape_handler(Some(&unseen))
        .err()
        .ok_or("unexpected provider")?;
    assert_eq!(
        unknown.java_exception_class(),
        Some("org.apache.seata.common.loader.EnhancedServiceNotFoundException")
    );

    locale.set_language("en")?;
    let recovered = factory.get_escape_handler(Some(&unseen))?;
    assert!(Arc::ptr_eq(&before, &recovered));
    Ok(())
}
