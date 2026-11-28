use crate::{EscapeHandler, JavaObjectError, MetadataError};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, ThreadId},
};

type Handler = Arc<dyn EscapeHandler + Send + Sync>;

/// Rust 服务实例持有器，保留 Java 构造锁的同线程重入。
/// 对应 Java: EnhancedServiceLoader 的 singleton Holder 与 synchronized(holder)。
#[derive(Default)]
pub(crate) struct EscapeProviderInstance {
    state: Mutex<InstanceState>,
    changed: Condvar,
}

#[derive(Default)]
struct InstanceState {
    value: Option<Handler>,
    owner: Option<(ThreadId, usize)>,
    #[cfg(test)]
    waiters: usize,
}

impl EscapeProviderInstance {
    /// 读取或构造实例。同线程重入只允许用于服务单例，原始名缓存拒绝递归更新。
    /// 对应 Java: synchronized(holder) 与 ConcurrentHashMap#computeIfAbsent。
    pub(crate) fn get_or_init(
        &self,
        reentrant: bool,
        create: impl FnOnce() -> Result<Handler, MetadataError>,
    ) -> Result<Handler, MetadataError> {
        let current = thread::current().id();
        let mut state = self.state.lock().map_err(|_| lock_poisoned())?;
        let mut waited_for_owner = false;
        loop {
            // Java monitor 中已排队的调用必须等外层退出，首次读取仍可看到内层发布值。
            if let Some(value) = state
                .value
                .as_ref()
                .filter(|_| !waited_for_owner || state.owner.is_none())
            {
                return Ok(value.clone());
            }
            match &mut state.owner {
                None => {
                    state.owner = Some((current, 1));
                    break;
                }
                Some((owner, depth)) if *owner == current => {
                    if !reentrant {
                        return Err(MetadataError::ObjectException(Arc::new(
                            JavaObjectError::new(
                                "java.lang.IllegalStateException",
                                Some("Recursive update".into()),
                            )
                            .with_superclasses([
                                "java.lang.RuntimeException",
                                "java.lang.Exception",
                                "java.lang.Throwable",
                            ]),
                        )));
                    }
                    *depth += 1;
                    break;
                }
                Some(_) => {
                    waited_for_owner = true;
                    #[cfg(test)]
                    {
                        state.waiters += 1;
                    }
                    state = self.changed.wait(state).map_err(|_| lock_poisoned())?;
                    #[cfg(test)]
                    {
                        state.waiters -= 1;
                    }
                }
            }
        }
        drop(state);
        let initialization = Initialization(self);
        let value = create()?;
        // 嵌套构造先完成时会先发布，外层构造完成后按原 Holder 语义替换。
        let previous = self
            .state
            .lock()
            .map_err(|_| lock_poisoned())?
            .value
            .replace(value.clone());
        drop(initialization);
        drop(previous);
        Ok(value)
    }
}

struct Initialization<'a>(&'a EscapeProviderInstance);

impl Drop for Initialization<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            if let Some((_, depth)) = &mut state.owner {
                if *depth == 1 {
                    state.owner = None;
                    self.0.changed.notify_all();
                } else {
                    *depth -= 1;
                }
            }
        }
    }
}

fn lock_poisoned() -> MetadataError {
    MetadataError::LockPoisoned("EscapeProviderInstance")
}

#[cfg(test)]
mod tests {
    use super::{EscapeProviderInstance, Handler};
    use crate::{EscapeHandler, JavaString, MetadataError, TableMeta};
    use std::{
        error::Error,
        sync::{mpsc, Arc},
        thread,
        time::{Duration, Instant},
    };

    struct Provider(bool);

    impl EscapeHandler for Provider {
        fn check_if_key_words(&self, _: Option<&JavaString>) -> Result<bool, MetadataError> {
            Ok(self.0)
        }

        fn check_if_need_escape(
            &self,
            _: Option<&JavaString>,
            _: Option<&TableMeta>,
        ) -> Result<bool, MetadataError> {
            Ok(self.0)
        }
    }

    #[test]
    fn existing_waiter_gets_outer_while_new_lookup_can_see_published_inner(
    ) -> Result<(), Box<dyn Error>> {
        let cell = Arc::new(EscapeProviderInstance::default());
        let (begin, started) = mpsc::channel();
        let (nested_ok, nested_done) = mpsc::channel();
        let (unblock, blocked) = mpsc::channel();
        let outer_cell = cell.clone();
        let outer = thread::spawn(move || {
            outer_cell.get_or_init(true, || {
                begin
                    .send(())
                    .map_err(|_| MetadataError::InvalidCompatibilityData("test start"))?;
                // 先让另一个调用实际进入 monitor 等待，再发布内层实例。
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let state = outer_cell
                        .state
                        .lock()
                        .map_err(|_| super::lock_poisoned())?;
                    if state.waiters > 0 {
                        break;
                    }
                    drop(state);
                    if Instant::now() >= deadline {
                        return Err(MetadataError::InvalidCompatibilityData(
                            "test waiter absent",
                        ));
                    }
                    thread::yield_now();
                }
                outer_cell.get_or_init(true, || Ok(Arc::new(Provider(false))))?;
                nested_ok.send(()).map_err(|_| {
                    MetadataError::InvalidCompatibilityData("test nested publication")
                })?;
                blocked
                    .recv_timeout(Duration::from_secs(3))
                    .map_err(|_| MetadataError::InvalidCompatibilityData("test outer release"))?;
                Ok(Arc::new(Provider(true)))
            })
        });
        started.recv_timeout(Duration::from_secs(3))?;
        let wait_cell = cell.clone();
        let (result, observed) = mpsc::channel();
        let waiter = thread::spawn(move || {
            let resolved = wait_cell.get_or_init(true, || Ok(Arc::new(Provider(false))));
            let _ = result.send(resolved);
        });
        nested_done.recv_timeout(Duration::from_secs(3))?;
        let newcomer = cell.get_or_init(true, || Ok(Arc::new(Provider(true))))?;
        let waiting = observed.recv_timeout(Duration::from_millis(100));
        unblock.send(())?;
        let outer = outer.join().map_err(|_| "outer panicked")??;
        waiter.join().map_err(|_| "waiter panicked")?;
        assert!(!newcomer.check_if_key_words(None)?);
        assert!(matches!(waiting, Err(mpsc::RecvTimeoutError::Timeout)));
        let from_waiter: Handler = observed.recv_timeout(Duration::from_secs(3))??;
        assert!(Arc::ptr_eq(&outer, &from_waiter));
        Ok(())
    }
}
