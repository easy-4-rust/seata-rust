//! 固定 Java UndoLogParserFactory 与 SPI 原用例及补充观察。
use seata_at::{UndoLogParser, UndoLogParserFactory, UndoLogParserSelectionError as Error};

#[test]
fn original_factory_get_instance() -> Result<(), Error> {
    assert!(matches!(
        UndoLogParserFactory::get_instance_by_name(Some("fst")),
        Err(Error::RemovedFst)
    ));
    assert!(matches!(
        UndoLogParserFactory::get_instance()?,
        UndoLogParser::Jackson(_)
    ));
    Ok(())
}

#[test]
fn original_provider_test_load() -> Result<(), Error> {
    assert!(matches!(
        UndoLogParser::load(Some("fastjson"))?,
        UndoLogParser::Fastjson(_)
    ));
    assert!(matches!(
        UndoLogParser::load(Some("jackson"))?,
        UndoLogParser::Jackson(_)
    ));
    assert!(matches!(
        UndoLogParser::load(Some("protostuff"))?,
        UndoLogParser::Protostuff(_)
    ));
    assert!(matches!(
        UndoLogParser::load(Some("kryo"))?,
        UndoLogParser::Kryo(_)
    ));
    assert!(matches!(
        UndoLogParser::load(Some("adadad")),
        Err(Error::ProviderNotFound)
    ));
    Ok(())
}

#[test]
fn rust_matches_original_selection_trace() -> Result<(), Error> {
    let expected = include_str!("fixtures/undo_parser_selection.tsv");
    let class = |parser: &UndoLogParser| match parser {
        UndoLogParser::Fastjson(_) => "FastjsonUndoLogParser",
        UndoLogParser::Jackson(_) => "JacksonUndoLogParser",
        UndoLogParser::Protostuff(_) => "ProtostuffUndoLogParser",
        UndoLogParser::Kryo(_) => "KryoUndoLogParser",
        UndoLogParser::Fastjson2(_) => "Fastjson2UndoLogParser",
        UndoLogParser::Fory(_) => "ForyUndoLogParser",
    };
    let mut actual = vec![format!(
        "DEFAULT\t{}",
        class(UndoLogParserFactory::get_instance()?)
    )];
    for name in [
        Some("fastjson"),
        Some("jackson"),
        Some("protostuff"),
        Some("kryo"),
        Some("fastjson2"),
        Some("fory"),
        Some("FURY"),
        Some("JaCkSoN"),
        Some("jacKson"),
        Some("Kryo"),
        Some("fst"),
        Some("FST"),
        Some("fſt"),
        Some("adadad"),
        Some(""),
        Some(" jackson"),
        None,
    ] {
        type LoadParser = fn(Option<&str>) -> Result<&'static UndoLogParser, Error>;
        let entrances: [(&str, LoadParser); 2] = [
            ("FACTORY", UndoLogParserFactory::get_instance_by_name),
            ("SPI", UndoLogParser::load),
        ];
        for (entry, load) in entrances {
            let result = match load(name) {
                Ok(parser) => format!("{}\t{}", class(parser), std::ptr::eq(parser, load(name)?)),
                Err(Error::RemovedFst | Error::EmptyName) => "IllegalArgumentException".into(),
                Err(Error::ProviderNotFound) => "EnhancedServiceNotFoundException".into(),
                Err(Error::NullFactoryName) => "NullPointerException".into(),
            };
            actual.push(format!("{entry}\t{}\t{result}", name.unwrap_or("<null>")));
        }
    }
    let alias = UndoLogParserFactory::get_instance_by_name(Some("FURY"))?;
    let provider = UndoLogParser::load(Some("fory"))?;
    actual.push(format!("ALIAS\t{}", std::ptr::eq(alias, provider)));
    assert_eq!(actual, expected.lines().collect::<Vec<_>>());
    Ok(())
}

#[test]
fn original_branch_encode_undo_log() -> Result<(), Box<dyn std::error::Error>> {
    use seata_at::{
        FastjsonUndoFieldValue as Value, JacksonSqlUndoLog, JacksonUndoField,
        JacksonUndoFieldValue, JacksonUndoRecords,
    };

    let default = UndoLogParserFactory::get_instance()?;
    assert_eq!(default.default_content()?, b"{}");
    let UndoLogParser::Jackson(parser) = default else {
        return Err("default parser is not Jackson".into());
    };
    let image = |name: &str| {
        let fields = [
            ("id", 4, Value::Integer(1)),
            ("name", 12, Value::Text(name.into())),
            ("since", 12, Value::Text("2014".into())),
        ]
        .into_iter()
        .map(|(label, code, value)| {
            JacksonUndoField::new(
                label.into(),
                code,
                false,
                JacksonUndoFieldValue::Common(value),
            )
        })
        .collect();
        JacksonUndoRecords::new(Some("product".into()), vec![fields])
    };
    let log = JacksonSqlUndoLog::new(
        Some("UPDATE".into()),
        None,
        Some(image("SEATA")),
        Some(image("SEATA_IO")),
    );
    let xid = Some("xid:xxx");
    let branch_id = 641_789_253;
    let bytes = parser.encode_typed_branch(xid, branch_id, Some(&[log]))?;
    let java = include_bytes!("fixtures/branch_undo_java.json");
    assert_eq!(bytes.as_slice(), java);
    let branch = parser
        .decode(&bytes)?
        .ok_or("missing original Undo branch")?;
    assert_eq!(branch.branch_id(), branch_id);
    Ok(())
}
