use crate::{
    escape_provider_instance::EscapeProviderInstance, EscapeHandler, EscapeHandlerRegistration,
    EscapeHandlerScope, JavaObjectError, JavaString, MetadataError, MetadataLocale,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

type Handler = Arc<dyn EscapeHandler + Send + Sync>;

enum Provider {
    Explicit(Handler),
    Linked {
        registration: &'static EscapeHandlerRegistration,
        singleton: EscapeProviderInstance,
    },
    Ambiguous,
}

struct CachedBinding {
    provider: Arc<Provider>,
    instance: EscapeProviderInstance,
}

impl CachedBinding {
    fn resolve(&self, locale: &MetadataLocale) -> Result<Handler, MetadataError> {
        match self.provider.as_ref() {
            Provider::Explicit(instance) => Ok(instance.clone()),
            Provider::Linked {
                registration,
                singleton,
            } => self.instance.get_or_init(false, || {
                let create = || Ok((registration.constructor)(locale.clone()));
                match registration.scope {
                    EscapeHandlerScope::Singleton => singleton.get_or_init(true, create),
                    EscapeHandlerScope::Prototype => create(),
                }
            }),
            Provider::Ambiguous => Err(service_error(
                "java.lang.IllegalArgumentException",
                "Multiple escape providers share the highest order",
            )),
        }
    }
}

/// 按数据库类型选择共享的标识符转义处理器。
/// 对应 Java: org.apache.seata.sqlparser.EscapeHandlerFactory。
/// 支持链接时发现与宿主显式注册，保留原始名称缓存与 Locale 查找规则。
pub struct EscapeHandlerFactory {
    locale: MetadataLocale,
    providers: HashMap<JavaString, Arc<Provider>>,
    cache: RwLock<HashMap<JavaString, Arc<CachedBinding>>>,
}

impl EscapeHandlerFactory {
    /// 用 locale 和同一服务域的 providers 构造工厂，服务名按初始 Locale 规范化。
    /// 对应 Java: EnhancedServiceLoader 服务定义初始化；Rust 由宿主提供注册项。
    pub fn new(
        locale: MetadataLocale,
        providers: impl IntoIterator<Item = (JavaString, Arc<dyn EscapeHandler + Send + Sync>)>,
    ) -> Result<Self, MetadataError> {
        let mut registered = HashMap::new();
        for (name, handler) in providers {
            let name = locale.lower(&name)?;
            if registered
                .insert(name, Arc::new(Provider::Explicit(handler)))
                .is_some()
            {
                return Err(service_error(
                    "java.lang.IllegalArgumentException",
                    "Duplicate escape provider name",
                ));
            }
        }
        Ok(Self {
            locale,
            providers: registered,
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// 用 locale 发现已链接方言，按原服务定义的有符号 order 选择最高优先级。
    /// 同名最高优先级并列时，选择报错，因为 Rust 链接注册无稳定遍历顺序。
    /// 对应 Java: EnhancedServiceLoader 的有名服务发现；工厂对应一个加载域。
    pub fn discover(locale: MetadataLocale) -> Result<Self, MetadataError> {
        let mut selected: HashMap<JavaString, (&EscapeHandlerRegistration, bool)> = HashMap::new();
        for registration in inventory::iter::<EscapeHandlerRegistration> {
            let name = locale.lower(&registration.name.into())?;
            let (current, tied) = selected.entry(name).or_insert((registration, false));
            if registration.order > current.order {
                *current = registration;
                *tied = false;
            } else if registration.order == current.order && !std::ptr::eq(*current, registration) {
                *tied = true;
            }
        }
        let providers = selected
            .into_iter()
            .map(|(name, (registration, tied))| {
                let provider = if tied {
                    Provider::Ambiguous
                } else {
                    Provider::Linked {
                        registration,
                        singleton: EscapeProviderInstance::default(),
                    }
                };
                (name, Arc::new(provider))
            })
            .collect();
        Ok(Self {
            locale,
            providers,
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// 用可空 db_type 选择共享处理器。缓存保留调用端原始拼写，未命中时按当前 Locale 查找。
    /// 对应 Java: EscapeHandlerFactory#getEscapeHandler(String)。
    pub fn get_escape_handler(
        &self,
        db_type: Option<&JavaString>,
    ) -> Result<Arc<dyn EscapeHandler + Send + Sync>, MetadataError> {
        let db_type = db_type.ok_or(MetadataError::NullReference("dbType"))?;
        let cached = self
            .cache
            .read()
            .map_err(|_| MetadataError::LockPoisoned("EscapeHandlerFactory"))?
            .get(db_type)
            .cloned();
        if let Some(binding) = cached {
            return binding.resolve(&self.locale);
        }
        if db_type.as_utf16().is_empty() {
            return Err(service_error(
                "java.lang.IllegalArgumentException",
                "the name of service provider for [org.apache.seata.sqlparser.EscapeHandler] name is null",
            ));
        }
        let binding = {
            let mut cache = self
                .cache
                .write()
                .map_err(|_| MetadataError::LockPoisoned("EscapeHandlerFactory"))?;
            if let Some(binding) = cache.get(db_type) {
                binding.clone()
            } else {
                let name = self.locale.lower(db_type)?;
                let provider = self.providers.get(&name).ok_or_else(|| {
                    service_error(
                        "org.apache.seata.common.loader.EnhancedServiceNotFoundException",
                        "not found service provider for : org.apache.seata.sqlparser.EscapeHandler",
                    )
                })?;
                let binding = Arc::new(CachedBinding {
                    provider: provider.clone(),
                    instance: EscapeProviderInstance::default(),
                });
                if !matches!(provider.as_ref(), Provider::Ambiguous) {
                    cache.insert(db_type.clone(), binding.clone());
                }
                binding
            }
        };
        // 只在释放名称缓存锁后才运行提供者构造器。
        binding.resolve(&self.locale)
    }
}

fn service_error(class: &str, message: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new(class, Some(message.into())).with_superclasses([
            "java.lang.RuntimeException",
            "java.lang.Exception",
            "java.lang.Throwable",
        ]),
    ))
}
