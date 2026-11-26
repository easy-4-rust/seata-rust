//! 两遍批量访问与 Java 式浅复制差分。
use seata_sqlparser::{
    IndexType, JavaObjectError, JavaString, LowerCaseLinkHashMap, MetadataError, MetadataLocale,
    MetadataMapSource, MetadataMapValue, MetadataMapVisitor,
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    error::Error,
    sync::{Arc, RwLock},
};

struct Number(i32);

impl MetadataMapValue for Number {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        Ok(other.is_some_and(|other| other.0 == self.0))
    }
    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(self.0)
    }
}

struct Text(RwLock<String>);

impl Text {
    fn new(value: &str) -> Arc<Self> {
        Arc::new(Self(RwLock::new(value.into())))
    }
}

impl MetadataMapValue for Text {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        Ok(other.is_some_and(|other| std::ptr::eq(other, self)))
    }
    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(0)
    }
}

struct Ordered(Vec<(Option<JavaString>, Option<Arc<Number>>)>);

impl MetadataMapSource<Number> for Ordered {
    fn for_each_entry(
        &self,
        visitor: &mut MetadataMapVisitor<'_, Number>,
    ) -> Result<(), MetadataError> {
        for (key, value) in &self.0 {
            visitor(key.as_ref(), value.clone())?;
        }
        Ok(())
    }
}

struct Changing {
    locale: MetadataLocale,
    visits: Cell<u8>,
    fail: bool,
}

impl MetadataMapSource<Number> for Changing {
    fn for_each_entry(
        &self,
        visitor: &mut MetadataMapVisitor<'_, Number>,
    ) -> Result<(), MetadataError> {
        let visit = self.visits.get() + 1;
        self.visits.set(visit);
        if visit == 1 {
            visitor(Some(&"I".into()), number(1))?;
            return visitor(Some(&"FirstOnly".into()), number(6));
        }
        self.locale.set_language("tr")?;
        visitor(Some(&"I".into()), number(9))?;
        if self.fail {
            return Err(MetadataError::ObjectException(Arc::new(
                JavaObjectError::new(
                    "java.lang.IllegalStateException",
                    Some("second pass".into()),
                ),
            )));
        }
        visitor(Some(&"SecondOnly".into()), number(3))
    }
}

fn number(n: i32) -> Option<Arc<Number>> {
    Some(Arc::new(Number(n)))
}
fn num(value: Option<Arc<Number>>) -> String {
    value.map_or_else(|| "null".into(), |value| value.0.to_string())
}
fn text(value: Option<Arc<Text>>) -> Result<String, MetadataError> {
    match value {
        Some(value) => Ok(value
            .0
            .read()
            .map_err(|_| MetadataError::LockPoisoned("test text"))?
            .clone()),
        None => Ok("null".into()),
    }
}

fn contents<V: MetadataMapValue>(
    map: &LowerCaseLinkHashMap<V>,
    mut render: impl FnMut(Option<Arc<V>>) -> Result<String, MetadataError>,
) -> Result<String, MetadataError> {
    let mut entries = map.entries()?;
    let mut out = Vec::new();
    while let Some((key, value)) = entries.next_entry()? {
        out.push(format!("{key}={}", render(value)?));
    }
    Ok(format!("{{{}}}", out.join(", ")))
}

fn class<T>(result: Result<T, MetadataError>) -> Result<String, Box<dyn Error>> {
    let error = result.err().ok_or("operation should fail")?;
    Ok(error
        .java_exception_class()
        .ok_or("missing Java exception class")?
        .into())
}

#[test]
fn java_bulk_write_and_shallow_copy_follow_original_two_pass_semantics(
) -> Result<(), Box<dyn Error>> {
    let mut seen = BTreeMap::new();
    let mut record = |key: &str, value: String| {
        assert!(seen.insert(key.to_owned(), value).is_none());
    };
    let locale = MetadataLocale::new("en");
    let map = LowerCaseLinkHashMap::new(locale.clone());
    map.put(Some("existing".into()), number(7))?;
    let source = Ordered(vec![
        (Some("A".into()), number(1)),
        (None, number(2)),
        (Some("B".into()), number(3)),
    ]);
    record("null-key-first-pass", class(map.put_all(Some(&source)))?);
    let keys = map.key_snapshot()?;
    record(
        "null-key-origins",
        format!(
            "{}:{}",
            keys.contains(&"A".into()),
            keys.contains(&"B".into())
        ),
    );
    record("null-key-target", contents(&map, |v| Ok(num(v)))?);
    record("null-source", class(map.put_all(None))?);

    map.clear()?;
    let source = Ordered(vec![
        (Some("Alpha".into()), number(1)),
        (Some("ALPHA".into()), number(8)),
        (Some("Nullable".into()), None),
    ]);
    map.put_all(Some(&source))?;
    record("bulk-order-and-update", contents(&map, |v| Ok(num(v)))?);
    let keys = map.key_snapshot()?;
    record(
        "bulk-keeps-last-origin",
        format!(
            "{}:{}",
            keys.contains(&"ALPHA".into()),
            keys.contains(&"Alpha".into())
        ),
    );
    let mut cursor = map.entries()?;
    cursor.next_handle()?;
    map.put_all(Some(&map))?;
    record("self-alias", contents(&map, |v| Ok(num(v)))?);
    let keys = map.key_snapshot()?;
    record(
        "self-alias-rewrites-origin",
        format!(
            "{}:{}",
            keys.contains(&"alpha".into()),
            keys.contains(&"ALPHA".into())
        ),
    );
    record(
        "self-alias-cursor-valid",
        cursor.next_handle()?.key().to_string(),
    );

    map.clear()?;
    let source = Changing {
        locale: locale.clone(),
        visits: Cell::new(0),
        fail: false,
    };
    map.put_all(Some(&source))?;
    record("two-visits", source.visits.get().to_string());
    record("two-visits-target", contents(&map, |v| Ok(num(v)))?);
    let keys = map.key_snapshot()?;
    record(
        "two-visits-origins",
        format!(
            "{}:{}",
            keys.contains(&"FirstOnly".into()),
            keys.contains(&"SecondOnly".into())
        ),
    );
    record(
        "turkish-target",
        format!(
            "{}:{}:{}",
            map.contains_key(Some(&"I".into()))?,
            map.contains_key(Some(&"i".into()))?,
            num(map.get(Some(&"I".into()))?)
        ),
    );
    locale.set_language("en")?;
    record(
        "english-target",
        format!(
            "{}:{}",
            map.contains_key(Some(&"I".into()))?,
            map.contains_key(Some(&"ı".into()))?
        ),
    );

    map.clear()?;
    let source = Changing {
        locale: locale.clone(),
        visits: Cell::new(0),
        fail: true,
    };
    record("second-pass-error", class(map.put_all(Some(&source)))?);
    record("second-pass-visited", source.visits.get().to_string());
    record(
        "second-pass-keeps-origin-and-target",
        format!(
            "{}:{}",
            map.key_snapshot()?.len(),
            contents(&map, |v| Ok(num(v)))?
        ),
    );
    locale.set_language("en")?;

    let original = LowerCaseLinkHashMap::new(locale.clone());
    let shared = Text::new("a");
    original.put(Some("CamelCase".into()), Some(shared.clone()))?;
    original.put(Some("RemovedOrigin".into()), Some(Text::new("stale")))?;
    let mut cursor = original.entries()?;
    cursor.next_handle()?;
    cursor.next_handle()?;
    cursor.remove_current()?;
    original.put(Some("Nullable".into()), None)?;
    let copy = original.java_clone()?;
    let keys = copy.key_snapshot()?;
    record(
        "copy-rebuilds-origins",
        format!(
            "{}:{}:{}",
            keys.contains(&"camelcase".into()),
            keys.contains(&"CamelCase".into()),
            keys.contains(&"RemovedOrigin".into())
        ),
    );
    let value = copy
        .get(Some(&"camelcase".into()))?
        .ok_or("missing copied value")?;
    record(
        "copy-shares-value",
        Arc::ptr_eq(&value, &shared).to_string(),
    );
    let entry = copy.entries()?.next_handle()?;
    let previous = entry.set_value(Some(Text::new("probe")))?;
    let original_value = original
        .get(Some(&"camelcase".into()))?
        .ok_or("missing original value")?;
    let current_copy = entry.value()?.ok_or("missing copied entry")?;
    record(
        "copy-shares-no-entry",
        Arc::ptr_eq(&original_value, &current_copy).to_string(),
    );
    entry.set_value(previous)?;
    shared
        .0
        .write()
        .map_err(|_| "test lock poisoned")?
        .push('b');
    record(
        "copy-sees-mutable-value",
        text(copy.get(Some(&"camelcase".into()))?)?,
    );
    copy.put(Some("CAMELCASE".into()), Some(Text::new("copy")))?;
    record(
        "copy-structure-independent",
        format!(
            "{}:{}",
            text(original.get(Some(&"camelcase".into()))?)?,
            text(copy.get(Some(&"camelcase".into()))?)?
        ),
    );
    record(
        "copy-null-entry",
        format!(
            "{}:{}",
            copy.contains_key(Some(&"Nullable".into()))?,
            text(copy.get(Some(&"Nullable".into()))?)?
        ),
    );
    record(
        "copy-constructor",
        contents(
            &LowerCaseLinkHashMap::from_map(locale.clone(), Some(&original))?,
            text,
        )?,
    );
    record(
        "copy-null-source",
        class(LowerCaseLinkHashMap::<Text>::from_map(locale, None))?,
    );

    let expected: BTreeMap<_, _> = include_str!("fixtures/sqlparser_core/map-copy-java.tsv")
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect();
    assert_eq!(expected.len(), 25);
    assert_eq!(seen.len(), expected.len());
    for (key, value) in expected {
        assert_eq!(seen.get(key).map(String::as_str), Some(value), "{key}");
    }
    Ok(())
}
