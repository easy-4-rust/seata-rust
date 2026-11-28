//! 活视图、条目身份及独立键快照的原 Java 差分。
use seata_sqlparser::{
    IndexType, JavaString, LowerCaseLinkHashMap, MetadataError, MetadataLocale, MetadataMapValue,
};
use std::{collections::BTreeMap, error::Error, sync::Arc};

struct Value(i32);
impl MetadataMapValue for Value {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        Ok(other.is_some_and(|value| self.0 == value.0))
    }
    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(self.0)
    }
}

fn value(n: i32) -> Option<Arc<Value>> {
    Some(Arc::new(Value(n)))
}
fn render(value: Option<Arc<Value>>) -> String {
    value.map_or_else(|| "null".into(), |v| v.0.to_string())
}
fn class<T>(result: Result<T, MetadataError>) -> Result<String, Box<dyn Error>> {
    let error = result.err().ok_or("operation should fail")?;
    Ok(error
        .java_exception_class()
        .ok_or("missing Java exception class")?
        .into())
}

#[test]
fn original_live_map_views_preserve_detached_entries_and_origin_side_effects(
) -> Result<(), Box<dyn Error>> {
    let mut seen = BTreeMap::new();
    let mut record = |key: &str, value: String| {
        assert!(seen.insert(key.to_owned(), value).is_none());
    };
    let map = LowerCaseLinkHashMap::new(MetadataLocale::new("en"));
    map.put(Some("First".into()), value(1))?;
    map.put(Some("SECOND".into()), value(2))?;
    let entries = map.entry_set();
    let values = map.values();
    let keys = map.key_snapshot()?;
    let mut cursor = entries.iter()?;
    record("remove-before-next", class(cursor.remove_current())?);
    let first = cursor.next_handle()?;
    record(
        "first",
        format!("{}:{}", first.key(), render(first.value()?)),
    );
    record("set-live", render(first.set_value(value(3))?));
    let render_values = || -> Result<String, MetadataError> {
        let mut cursor = values.iter()?;
        let mut out = Vec::new();
        while cursor.has_next() {
            out.push(render(cursor.next_value()?));
        }
        Ok(format!("[{}]", out.join(", ")))
    };
    record("values-live", render_values()?);
    record(
        "put-replace",
        render(map.put(Some("FIRSt".into()), value(4))?),
    );
    record("handle-saw-put", render(first.value()?));
    cursor.remove_current()?;
    record(
        "removed",
        map.contains_key(Some(&"first".into()))?.to_string(),
    );
    record("removed-entry-retained", render(first.value()?));
    record("double-remove", class(cursor.remove_current())?);
    let next = cursor.next_handle()?;
    record(
        "second-after-remove",
        format!("{}:{}", next.key(), render(next.value()?)),
    );
    record("exhausted-next", class(cursor.next_handle())?);
    record("set-detached", render(first.set_value(value(5))?));
    map.put(Some("FIRST".into()), value(6))?;
    record(
        "detached-vs-reinserted",
        format!(
            "{}:{}",
            render(first.value()?),
            render(map.get(Some(&"FIRST".into()))?)
        ),
    );
    record(
        "old-key-snapshot",
        format!(
            "{}:{}",
            keys.contains(&"First".into()),
            keys.contains(&"FIRST".into())
        ),
    );
    let current_keys = map.key_snapshot()?;
    record(
        "current-key-snapshot",
        format!(
            "{}:{}",
            current_keys.contains(&"FIRST".into()),
            current_keys.contains(&"First".into())
        ),
    );
    let mut values_cursor = values.iter()?;
    record("values-first", render(values_cursor.next_value()?));
    values_cursor.remove_current()?;
    record(
        "values-remove-target-only",
        format!(
            "{}:{}",
            map.contains_key(Some(&"second".into()))?,
            map.key_snapshot()?.contains(&"SECOND".into())
        ),
    );
    let current = entries.iter()?.next_handle()?;
    entries.clear()?;
    record(
        "entries-clear-target-only",
        format!("{}:{}", entries.len()?, map.key_snapshot()?.len()),
    );
    record(
        "clear-detached-entry",
        format!("{}:{}", current.key(), render(current.value()?)),
    );
    map.put(Some("THIRD".into()), None)?;
    record("values-visible-after-clear", render_values()?);
    let mut changed = entries.iter()?;
    changed.next_handle()?;
    map.put(Some("fourth".into()), value(8))?;
    record("changed-remove", class(changed.remove_current())?);
    record("changed-exhausted-next", class(changed.next_handle())?);
    record("values-remove-null", values.remove(None)?.to_string());
    record(
        "null-key-origin-retained",
        map.key_snapshot()?.contains(&"THIRD".into()).to_string(),
    );
    record(
        "entries-remove-lowercase-only",
        format!(
            "{}:{}",
            entries.remove(&"FOURTH".into(), Some(&Value(8)))?,
            entries.remove(&"fourth".into(), Some(&Value(8)))?
        ),
    );
    record(
        "entries-remove-origin-retained",
        map.key_snapshot()?.contains(&"fourth".into()).to_string(),
    );
    map.put(Some("values-Clear".into()), value(9))?;
    values.clear()?;
    record(
        "values-clear-target-only",
        format!(
            "{}:{}",
            values.len()?,
            map.key_snapshot()?.contains(&"values-Clear".into())
        ),
    );
    map.clear()?;
    record(
        "map-clear-both",
        format!("{}:{}", values.len()?, map.key_snapshot()?.len()),
    );
    for count in [1, 11, 12, 13, 21, 48, 65] {
        map.clear()?;
        for index in 0..count {
            map.put(
                Some(JavaString::from(format!("Key-{}", 31 * index))),
                value(index),
            )?;
        }
        record(
            &format!("snapshot-order/{count}"),
            map.key_snapshot()?
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    let expected: BTreeMap<_, _> = include_str!("fixtures/sqlparser_core/map-views-java.tsv")
        .lines()
        .filter_map(|s| s.split_once('\t'))
        .collect();
    assert_eq!(expected.len(), 35);
    assert_eq!(seen.len(), expected.len());
    for (key, expected) in expected {
        assert_eq!(seen.get(key).map(String::as_str), Some(expected), "{key}");
    }
    Ok(())
}
