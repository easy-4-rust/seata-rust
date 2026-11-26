//! 原 Java Map 默认更新顺序及回调行为的差分。
use seata_sqlparser::{
    IndexType, JavaObjectError, JavaString, LowerCaseLinkHashMap, MetadataError, MetadataLocale,
    MetadataMapValue,
};
use std::{collections::BTreeMap, error::Error, sync::Arc};

struct Value(i32);
impl MetadataMapValue for Value {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        Ok(other.is_some_and(|other| other.0 == self.0))
    }
    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(self.0)
    }
}

struct Directional {
    value: i32,
    matches: i32,
    clear: Option<LowerCaseLinkHashMap<Self>>,
}
impl MetadataMapValue for Directional {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        if let Some(map) = &self.clear {
            map.clear()?;
        }
        Ok(other.is_some_and(|other| self.matches == other.value))
    }
    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(0)
    }
}

type Nullable = Option<Arc<Value>>;
type MapAction = fn(&JavaString, Nullable) -> Result<(), MetadataError>;
type Remap = fn(&JavaString, Nullable) -> Result<Nullable, MetadataError>;

fn value(n: i32) -> Nullable {
    Some(Arc::new(Value(n)))
}
fn render(value: Nullable) -> String {
    value.map_or_else(|| "null".into(), |v| v.0.to_string())
}
fn exception() -> MetadataError {
    MetadataError::ObjectException(Arc::new(JavaObjectError::new(
        "java.lang.IllegalStateException",
        Some("test".into()),
    )))
}
fn observe<T>(result: Result<T, MetadataError>, render: impl FnOnce(T) -> String) -> String {
    result.map_or_else(
        |error| {
            error
                .java_exception_class()
                .map_or_else(|| error.to_string(), str::to_owned)
        },
        render,
    )
}

#[test]
fn lower_case_map_defaults_match_original_callback_and_null_semantics() -> Result<(), Box<dyn Error>>
{
    let mut seen = BTreeMap::new();
    let mut record = |key: &str, value: String| {
        assert!(seen.insert(key.to_owned(), value).is_none());
    };
    let locale = MetadataLocale::new("en");
    let map = LowerCaseLinkHashMap::new(locale.clone());
    let null_key = JavaString::from("NULLKEY");
    let absent = JavaString::from("ABSENT");
    let new = JavaString::from("NEW");
    let drop_key = JavaString::from("DROP");
    let empty = JavaString::from("EMPTY");
    let side = JavaString::from("SIDE");
    let merge = JavaString::from("MERGE");
    let yes = |n: bool| n.to_string();
    map.put(Some("NullKey".into()), None)?;

    record(
        "default-null",
        render(map.get_or_default(Some(&null_key), value(9))?),
    );
    record(
        "default-missing",
        render(map.get_or_default(Some(&absent), value(9))?),
    );
    record(
        "default-null-key",
        render(map.get_or_default(None, value(9))?),
    );
    record(
        "put-null-value",
        render(map.put_if_absent(Some(&null_key), value(4))?),
    );
    record(
        "put-existing",
        render(map.put_if_absent(Some(&"NuLlKeY".into()), value(5))?),
    );
    record(
        "put-origin-folded",
        format!(
            "{}:{}",
            map.key_snapshot()?.contains(&"nullkey".into()),
            render(map.get(Some(&null_key))?)
        ),
    );
    record(
        "replace-value",
        render(map.replace(Some(&null_key), value(6))?),
    );
    record(
        "replace-missing",
        render(map.replace(Some(&absent), value(7))?),
    );
    record(
        "replace-wrong",
        yes(map.replace_if_equal(Some(&null_key), Some(&Value(4)), value(8))?),
    );
    record(
        "replace-match",
        yes(map.replace_if_equal(Some(&null_key), Some(&Value(6)), None)?),
    );
    record(
        "replace-present-null",
        yes(map.replace_if_equal(Some(&null_key), None, value(9))?),
    );

    let mut callback_key = String::new();
    let result = map.compute_if_absent(
        Some(&new),
        Some(|key: &JavaString| {
            callback_key = key.to_string();
            Ok(value(2))
        }),
    )?;
    record(
        "compute-absent-folded",
        format!("{callback_key}:{}", render(result)),
    );
    record(
        "compute-absent-no-callback",
        render(map.compute_if_absent(Some(&new), Some(|_: &JavaString| Err(exception())))?),
    );
    let result = map.compute_if_absent(Some(&drop_key), Some(|_: &JavaString| Ok(None)))?;
    record(
        "compute-absent-return-null",
        format!("{}:{}", render(result), map.contains_key(Some(&drop_key))?),
    );
    let mut callback_present = String::new();
    let result = map.compute_if_present(
        Some(&new),
        Some(|key: &JavaString, value: Arc<Value>| {
            callback_present = format!("{key}:{}", value.0);
            Ok(None)
        }),
    )?;
    record(
        "compute-present-remove",
        format!(
            "{callback_present}:{}:{}",
            render(result),
            map.contains_key(Some(&new))?
        ),
    );
    map.put(Some(empty.clone()), None)?;
    let result = map.compute_if_present(
        Some(&empty),
        Some(|_: &JavaString, _: Arc<Value>| Ok(value(5))),
    )?;
    record(
        "present-skips-null-value",
        format!("{}:{}", render(result), map.contains_key(Some(&empty))?),
    );
    let result = map.compute(Some(&empty), Some(|_: &JavaString, _: Nullable| Ok(None)))?;
    record(
        "compute-removes-null-entry",
        format!("{}:{}", render(result), map.contains_key(Some(&empty))?),
    );
    let result = map.compute(
        Some(&side),
        Some(|key: &JavaString, _: Nullable| {
            map.put(Some(key.clone()), value(8))?;
            Ok(None)
        }),
    )?;
    record(
        "compute-rechecks-after-callback",
        format!("{}:{}", render(result), map.contains_key(Some(&side))?),
    );
    let result = map.compute(
        Some(&side),
        Some(|key: &JavaString, _: Nullable| {
            map.put(Some(key.clone()), value(8))?;
            Ok(value(3))
        }),
    )?;
    record(
        "compute-outputs-own-value",
        format!("{}:{}", render(result), render(map.get(Some(&side))?)),
    );
    record(
        "compute-null-key-callback-precedence",
        observe(map.compute(None, None::<Remap>), render),
    );
    record(
        "compute-null-function",
        observe(map.compute(Some(&side), None::<Remap>), render),
    );
    record(
        "compute-callback-error",
        observe(
            map.compute(
                Some(&side),
                Some(|_: &JavaString, _: Nullable| Err(exception())),
            ),
            render,
        ),
    );
    record("compute-after-error", render(map.get(Some(&side))?));
    let mut locale_key = String::new();
    let result = map.compute_if_absent(
        Some(&"I".into()),
        Some(|key: &JavaString| {
            locale_key = key.to_string();
            locale.set_language("tr")?;
            Ok(value(7))
        }),
    )?;
    record(
        "compute-locale-callback",
        format!(
            "{locale_key}:{}:{}:{}",
            render(result),
            map.contains_key(Some(&"I".into()))?,
            render(map.get(Some(&"i".into()))?)
        ),
    );
    locale.set_language("en")?;
    record(
        "merge-absent",
        render(map.merge(
            Some(&merge),
            value(11),
            Some(|_: Arc<Value>, _: Arc<Value>| Err(exception())),
        )?),
    );
    record(
        "merge-existing",
        render(map.merge(
            Some(&merge),
            value(4),
            Some(|a: Arc<Value>, b: Arc<Value>| Ok(value(a.0 * 10 + b.0))),
        )?),
    );
    let result = map.merge(
        Some(&merge),
        value(9),
        Some(|_: Arc<Value>, _: Arc<Value>| Ok(None)),
    )?;
    record(
        "merge-remove",
        format!("{}:{}", render(result), map.contains_key(Some(&merge))?),
    );
    record(
        "merge-null-value",
        observe(
            map.merge(
                Some(&new),
                None,
                Some(|_: Arc<Value>, _: Arc<Value>| Ok(value(1))),
            ),
            render,
        ),
    );
    record(
        "remove-wrong",
        yes(map.remove_if_equal(Some(&side), Some(&Value(8)))?),
    );
    record(
        "remove-match",
        yes(map.remove_if_equal(Some(&side), Some(&Value(3)))?),
    );
    record(
        "remove-missing-null",
        yes(map.remove_if_equal(Some(&absent), None)?),
    );
    let nullable = JavaString::from("NULLABLE");
    map.put(Some("nullable".into()), None)?;
    record(
        "remove-present-null",
        yes(map.remove_if_equal(Some(&nullable), None)?),
    );

    map.clear()?;
    map.put(Some("First".into()), value(1))?;
    map.put(Some("Second".into()), value(2))?;
    let mut log = Vec::new();
    map.for_each(Some(|key: &JavaString, current: Nullable| {
        log.push(format!("{key}={}", render(current)));
        if key.as_str() == Some("first") {
            map.put(Some("SECOND".into()), value(8))?;
        }
        Ok(())
    }))?;
    record("for-each-future-value", format!("[{}]", log.join(", ")));
    map.replace_all(Some(|key: &JavaString, current: Nullable| {
        if key.as_str() == Some("second") {
            map.clear()?;
        }
        Ok(current.map(|v| Arc::new(Value(v.0 * 10))))
    }))?;
    record("replace-all-clear-last", map.len()?.to_string());
    map.put(Some("First".into()), value(1))?;
    map.put(Some("Second".into()), value(2))?;
    let result = map.replace_all(Some(|_: &JavaString, current: Nullable| {
        map.clear()?;
        Ok(current)
    }));
    record(
        "replace-all-clear-first",
        observe(result, |()| "unexpected success".into()),
    );
    map.put(Some("First".into()), value(1))?;
    record(
        "replace-all-callback-exception",
        observe(
            map.replace_all(Some(|_: &JavaString, _: Nullable| Err(exception()))),
            |()| "unexpected success".into(),
        ),
    );
    record(
        "for-each-null-callback",
        observe(map.for_each(None::<MapAction>), |()| {
            "unexpected success".into()
        }),
    );

    let directed = LowerCaseLinkHashMap::new(locale.clone());
    let expected = Directional {
        value: 2,
        matches: 99,
        clear: None,
    };
    directed.put(
        Some("X".into()),
        Some(Arc::new(Directional {
            value: 1,
            matches: 2,
            clear: None,
        })),
    )?;
    record(
        "replace-equals-current-first",
        yes(directed.replace_if_equal(
            Some(&"X".into()),
            Some(&expected),
            Some(Arc::new(Directional {
                value: 3,
                matches: 2,
                clear: None,
            })),
        )?),
    );
    record(
        "remove-equals-current-first",
        yes(directed.remove_if_equal(Some(&"X".into()), Some(&expected))?),
    );
    let reentrant = LowerCaseLinkHashMap::new(locale);
    reentrant.put(
        Some("Key".into()),
        Some(Arc::new(Directional {
            value: 1,
            matches: 2,
            clear: Some(reentrant.clone()),
        })),
    )?;
    let removed = reentrant.remove_if_equal(Some(&"KEY".into()), Some(&expected))?;
    record(
        "remove-reenters-public-map-remove",
        format!("{removed}:{}", reentrant.len()?),
    );

    let expected: BTreeMap<_, _> = include_str!("fixtures/sqlparser_core/map-defaults-java.tsv")
        .lines()
        .filter_map(|s| s.split_once('\t'))
        .collect();
    assert_eq!(expected.len(), 40);
    assert_eq!(expected.len(), seen.len());
    for (key, expected) in expected {
        assert_eq!(seen.get(key).map(String::as_str), Some(expected), "{key}");
    }
    Ok(())
}
