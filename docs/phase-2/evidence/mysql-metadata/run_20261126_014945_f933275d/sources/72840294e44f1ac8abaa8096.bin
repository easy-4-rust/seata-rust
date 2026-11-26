//! 配置/XID及跨await任务上下文隔离，不依赖TC模拟状态。
#![allow(clippy::unwrap_used)]
use seata_tm::{
    GlobalTransactionRole, Propagation, RootContext, TransactionConfig, TransactionContext, Xid,
};
use std::time::Duration;

fn xid(value: &str) -> Xid {
    Xid::parse(value.to_owned()).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn xid_validation_preserves_exact_utf8_and_never_normalizes_or_synthesizes() {
    for value in ["198.18.0.1:38091:9007199254740993", "opaque:身份"] {
        let id = xid(value);
        assert_eq!(id.as_str(), value);
        assert_eq!(id.to_string(), value);
        assert!(!format!("{id:?}").contains(value));
    }
    for value in ["", " x", "x ", "x\n", "x\0y", "\tx"] {
        assert!(Xid::parse(value.to_owned()).is_err(), "{value:?}");
    }
    assert!(Xid::parse("x".repeat(32767)).is_ok());
    assert!(Xid::parse("x".repeat(32768)).is_err());
    assert!(Xid::parse("中".repeat(10923)).is_err());
}

#[test]
fn config_builder_keeps_tc_timeout_exact_and_defaults_required() {
    assert!(
        !format!("{:?}", TransactionConfig::builder("private-name".into()))
            .contains("private-name")
    );
    let config = TransactionConfig::builder("订单".to_owned())
        .transaction_timeout(Duration::from_millis(1700))
        .build()
        .unwrap();
    assert_eq!(config.name(), "订单");
    assert_eq!(config.timeout_millis(), 1700);
    assert_eq!(config.propagation(), Propagation::Required);
    let config = TransactionConfig::builder("nested".to_owned())
        .propagation(Propagation::RequiresNew)
        .build()
        .unwrap();
    assert_eq!(config.timeout_millis(), 60000);
    assert_eq!(config.propagation(), Propagation::RequiresNew);
}

#[test]
fn config_rejects_lossy_zero_or_overflow_timeout_and_invalid_name() {
    for value in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_nanos(1000001),
        Duration::from_millis(i32::MAX as u64 + 1),
        Duration::MAX,
    ] {
        assert!(
            TransactionConfig::builder("x".into())
                .transaction_timeout(value)
                .build()
                .is_err(),
            "{value:?}"
        );
    }
    for name in [String::new(), "x\0y".into(), "x".repeat(32768)] {
        assert!(TransactionConfig::builder(name).build().is_err());
    }
    assert!(TransactionConfig::builder("max".into())
        .transaction_timeout(Duration::from_millis(i32::MAX as u64))
        .build()
        .is_ok());
}

#[tokio::test]
async fn nested_context_is_restored_after_return_and_business_error() {
    assert!(RootContext::current().is_none());
    let outer = TransactionContext::participant(xid("outer"));
    RootContext::scope(Some(outer), async {
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "outer");
        let error =
            RootContext::scope(Some(TransactionContext::participant(xid("inner"))), async {
                assert_eq!(
                    RootContext::current().unwrap().role(),
                    GlobalTransactionRole::Participant
                );
                tokio::task::yield_now().await;
                assert_eq!(RootContext::current().unwrap().xid().as_str(), "inner");
                Err::<(), _>("business")
            })
            .await;
        assert_eq!(error, Err("business"));
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "outer");
        RootContext::scope(None, async {
            assert!(RootContext::current().is_none());
        })
        .await;
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "outer");
    })
    .await;
    assert!(RootContext::current().is_none());
}

#[tokio::test]
async fn interleaved_futures_on_one_task_keep_independent_xids_each_poll() {
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let left = RootContext::scope(Some(TransactionContext::participant(xid("left"))), async {
        barrier.wait().await;
        tokio::task::yield_now().await;
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "left");
    });
    let right = RootContext::scope(Some(TransactionContext::participant(xid("right"))), async {
        barrier.wait().await;
        tokio::task::yield_now().await;
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "right");
    });
    tokio::join!(left, right);
    assert!(RootContext::current().is_none());
}

#[tokio::test]
async fn spawned_tasks_require_explicit_participant_capture_and_never_inherit_implicitly() {
    RootContext::scope(Some(TransactionContext::participant(xid("outer"))), async {
        tokio::spawn(async {
            assert!(RootContext::current().is_none());
        })
        .await
        .unwrap();
        let captured = RootContext::capture();
        tokio::spawn(RootContext::scope(captured, async {
            let context = RootContext::current().unwrap();
            assert_eq!(context.xid().as_str(), "outer");
            assert_eq!(context.role(), GlobalTransactionRole::Participant);
        }))
        .await
        .unwrap();
    })
    .await;
}

#[tokio::test]
async fn panic_or_cancellation_does_not_leak_context_into_worker_or_outer_scope() {
    let task = tokio::spawn(RootContext::scope(
        Some(TransactionContext::participant(xid("panic"))),
        async {
            panic!("business panic");
        },
    ));
    assert!(task.await.unwrap_err().is_panic());
    assert!(RootContext::current().is_none());
    RootContext::scope(Some(TransactionContext::participant(xid("outer"))), async {
        let (ready, seen) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(RootContext::scope(
            Some(TransactionContext::participant(xid("cancel"))),
            async {
                assert_eq!(RootContext::current().unwrap().xid().as_str(), "cancel");
                ready.send(()).unwrap();
                std::future::pending::<()>().await;
            },
        ));
        seen.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(RootContext::current().unwrap().xid().as_str(), "outer");
    })
    .await;
    assert!(RootContext::current().is_none());
}
