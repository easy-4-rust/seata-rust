//! Rust SPI 链接、选择及缓存验收。
use seata_db_mysql as _;
use seata_sqlparser::{
    register_escape_handler, EscapeHandler, EscapeHandlerFactory, EscapeHandlerRegistration,
    EscapeHandlerScope, EscapeSymbol, JavaString, MetadataError, MetadataLocale, TableMeta,
};
use std::{
    error::Error,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Barrier, OnceLock,
    },
    time::Duration,
};

static SINGLETON_CREATED: AtomicUsize = AtomicUsize::new(0);
static PROTOTYPE_CREATED: AtomicUsize = AtomicUsize::new(0);
static LOSING_CREATED: AtomicUsize = AtomicUsize::new(0);
static REENTRANT_CREATED: AtomicUsize = AtomicUsize::new(0);
static REENTRANT_FACTORY: OnceLock<Arc<EscapeHandlerFactory>> = OnceLock::new();

struct Provider(u16);

impl EscapeHandler for Provider {
    fn check_if_key_words(&self, _name: Option<&JavaString>) -> Result<bool, MetadataError> {
        Ok(true)
    }

    fn check_if_need_escape(
        &self,
        _name: Option<&JavaString>,
        _table: Option<&TableMeta>,
    ) -> Result<bool, MetadataError> {
        Ok(true)
    }

    fn escape_symbol(&self) -> Result<Option<EscapeSymbol>, MetadataError> {
        Ok(Some(EscapeSymbol::new(self.0)))
    }
}

fn singleton(_: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    SINGLETON_CREATED.fetch_add(1, Ordering::SeqCst);
    Arc::new(Provider(u16::from(b']')))
}

fn losing(_: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    LOSING_CREATED.fetch_add(1, Ordering::SeqCst);
    Arc::new(Provider(u16::from(b'!')))
}

fn prototype(_: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    PROTOTYPE_CREATED.fetch_add(1, Ordering::SeqCst);
    Arc::new(Provider(u16::from(b'#')))
}

fn reentrant(_: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    if REENTRANT_CREATED.fetch_add(1, Ordering::SeqCst) == 0 {
        let Some(factory) = REENTRANT_FACTORY.get() else {
            panic!("reentrant factory not published");
        };
        let Ok(nested) = factory.get_escape_handler(Some(&"TEST-REENTRANT".into())) else {
            panic!("provider alias lookup failed");
        };
        assert!(
            matches!(nested.escape_symbol(), Ok(Some(symbol)) if symbol.left_symbol() == u16::from(b'I'))
        );
        Arc::new(Provider(u16::from(b'O')))
    } else {
        Arc::new(Provider(u16::from(b'I')))
    }
}

register_escape_handler!(EscapeHandlerRegistration::new(
    "test-reentrant",
    0,
    EscapeHandlerScope::Singleton,
    reentrant
));

register_escape_handler!(EscapeHandlerRegistration::new(
    "test-singleton",
    i32::MIN,
    EscapeHandlerScope::Singleton,
    losing
));
register_escape_handler!(EscapeHandlerRegistration::new(
    "test-singleton",
    i32::MAX,
    EscapeHandlerScope::Singleton,
    singleton
));
register_escape_handler!(EscapeHandlerRegistration::new(
    "test-prototype",
    0,
    EscapeHandlerScope::Prototype,
    prototype
));
register_escape_handler!(EscapeHandlerRegistration::new(
    "test-tie",
    0,
    EscapeHandlerScope::Singleton,
    losing
));
register_escape_handler!(EscapeHandlerRegistration::new(
    "test-tie",
    0,
    EscapeHandlerScope::Singleton,
    singleton
));

fn original(key: &str) -> Result<&'static str, Box<dyn Error>> {
    original_value(include_str!("fixtures/escape-spi-java.tsv"), key)
}

fn original_value(fixture: &'static str, key: &str) -> Result<&'static str, Box<dyn Error>> {
    fixture
        .lines()
        .filter_map(|row| row.split_once('\t'))
        .find_map(|(name, value)| (name == key).then_some(value))
        .ok_or_else(|| format!("missing Java observation: {key}").into())
}

#[test]
fn discovers_linked_mysql_and_constructs_only_winning_singleton_on_concurrent_lookup(
) -> Result<(), Box<dyn Error>> {
    let factory = Arc::new(EscapeHandlerFactory::discover(MetadataLocale::new("en"))?);
    assert_eq!(SINGLETON_CREATED.load(Ordering::SeqCst), 0);
    let mysql = factory.get_escape_handler(Some(&"MYSQL".into()))?;
    assert_eq!(
        mysql.add_col_name_escape(Some(&"select".into()))?,
        "`select`".into()
    );
    let barrier = Arc::new(Barrier::new(8));
    let mut joins = Vec::new();
    for n in 0..8 {
        let factory = factory.clone();
        let barrier = barrier.clone();
        joins.push(std::thread::spawn(move || {
            let key = if n % 2 == 0 {
                "test-singleton"
            } else {
                "TEST-SINGLETON"
            };
            barrier.wait();
            factory.get_escape_handler(Some(&key.into()))
        }));
    }
    let resolved = joins
        .into_iter()
        .map(|join| {
            join.join()
                .map_err(|_| "lookup thread panicked")?
                .map_err(Into::into)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let selected = resolved.first().ok_or("no provider resolved")?;
    let shared = resolved.iter().all(|actual| Arc::ptr_eq(actual, selected));
    assert_eq!(shared.to_string(), original("singleton-identity")?);
    let left = selected
        .escape_symbol()?
        .ok_or("missing escape symbol")?
        .left_symbol();
    assert_eq!(
        char::try_from(u32::from(left))?.to_string(),
        original("winner")?
    );
    assert_eq!(
        SINGLETON_CREATED.load(Ordering::SeqCst).to_string(),
        original("singleton-count")?
    );
    assert_eq!(
        LOSING_CREATED.load(Ordering::SeqCst).to_string(),
        original("losing-count")?
    );
    Ok(())
}

#[test]
fn prototype_is_cached_once_per_raw_request_name() -> Result<(), Box<dyn Error>> {
    let factory = EscapeHandlerFactory::discover(MetadataLocale::new("en"))?;
    let low = factory.get_escape_handler(Some(&"test-prototype".into()))?;
    let again = factory.get_escape_handler(Some(&"test-prototype".into()))?;
    let upper = factory.get_escape_handler(Some(&"TEST-PROTOTYPE".into()))?;
    assert_eq!(
        Arc::ptr_eq(&low, &again).to_string(),
        original("prototype-same-raw")?
    );
    assert_eq!(
        Arc::ptr_eq(&low, &upper).to_string(),
        original("prototype-case-variant")?
    );
    assert_eq!(
        PROTOTYPE_CREATED.load(Ordering::SeqCst).to_string(),
        original("prototype-count")?
    );
    Ok(())
}

#[test]
fn tied_highest_priority_reports_configuration_error_without_selecting_a_provider(
) -> Result<(), Box<dyn Error>> {
    let factory = EscapeHandlerFactory::discover(MetadataLocale::new("en"))?;
    let error = factory
        .get_escape_handler(Some(&"test-tie".into()))
        .err()
        .ok_or("tie selected a provider")?;
    assert_eq!(
        error.java_exception_class(),
        Some("java.lang.IllegalArgumentException")
    );
    assert_eq!(
        error.java_message().and_then(JavaString::as_str),
        Some("Multiple escape providers share the highest order")
    );
    Ok(())
}

#[test]
fn singleton_constructor_can_resolve_its_alias_and_preserves_both_raw_bindings(
) -> Result<(), Box<dyn Error>> {
    let factory = Arc::new(EscapeHandlerFactory::discover(MetadataLocale::new("en"))?);
    REENTRANT_FACTORY
        .set(factory.clone())
        .map_err(|_| "factory already created")?;
    let (send, receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = factory.get_escape_handler(Some(&"test-reentrant".into()));
        let _ = send.send(result);
    });
    let outer = receive.recv_timeout(Duration::from_secs(3))??;
    worker.join().map_err(|_| "constructor panicked")?;
    let inner = REENTRANT_FACTORY
        .get()
        .ok_or("missing factory")?
        .get_escape_handler(Some(&"TEST-REENTRANT".into()))?;
    let fresh = REENTRANT_FACTORY
        .get()
        .ok_or("missing factory")?
        .get_escape_handler(Some(&"Test-Reentrant".into()))?;
    let fixture = include_str!("fixtures/escape-spi-reentry-java.tsv");
    for (key, handler) in [("outer", &outer), ("inner", &inner)] {
        let symbol = handler.escape_symbol()?.ok_or("missing escape symbol")?;
        assert_eq!(
            char::try_from(u32::from(symbol.left_symbol()))?.to_string(),
            original_value(fixture, key)?
        );
    }
    assert_eq!(
        Arc::ptr_eq(&inner, &outer).to_string(),
        original_value(fixture, "same-inner-outer")?
    );
    assert_eq!(
        Arc::ptr_eq(&outer, &fresh).to_string(),
        original_value(fixture, "same-fresh-outer")?
    );
    assert_eq!(
        REENTRANT_CREATED.load(Ordering::SeqCst).to_string(),
        original_value(fixture, "created")?
    );
    Ok(())
}
