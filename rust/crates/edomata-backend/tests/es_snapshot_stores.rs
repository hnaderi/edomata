//! Port of `eventsourcing/InMemorySnapshotSuite.scala` and
//! `eventsourcing/PersistedSnapshotStoreSuite.scala` (with Tokio's paused
//! clock in place of `TestControl`).

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use edomata_backend::eventsourcing::{
    InMemorySnapshotStore, PersistedSnapshotConfig, PersistedSnapshotStore, SnapshotPersistence,
    SnapshotReader, SnapshotStore, ValidState, dedup,
};

fn aggregate(s: &str, v: i64) -> ValidState<String> {
    ValidState::new(s.to_string(), v)
}

// --- InMemorySnapshotStoreSuite ------------------------------------------

#[tokio::test]
async fn in_mem_must_not_store_more_than_size() {
    let sut = InMemorySnapshotStore::<String>::new(1);
    sut.put("a", aggregate("va", 1)).await.unwrap();
    assert_eq!(sut.get("a").await.unwrap(), Some(aggregate("va", 1)));
    sut.put("b", aggregate("vb", 3)).await.unwrap();
    assert_eq!(sut.get("b").await.unwrap(), Some(aggregate("vb", 3)));
    assert_eq!(sut.get("a").await.unwrap(), None);
}

#[tokio::test]
async fn in_mem_must_be_convergent() {
    let sut = InMemorySnapshotStore::<String>::new(1);
    sut.put("a", aggregate("vvv", 3)).await.unwrap();
    sut.put("a", aggregate("vv", 2)).await.unwrap();
    assert_eq!(sut.get("a").await.unwrap(), Some(aggregate("vvv", 3)));
    assert_eq!(sut.get_fast("a").await, Some(aggregate("vvv", 3)));
}

// --- PersistedSnapshotStoreSuite -----------------------------------------

fn a() -> ValidState<String> {
    aggregate("a", 1)
}

fn b() -> ValidState<String> {
    aggregate("b", 2)
}

struct Universe {
    persistence: Arc<dyn SnapshotPersistence<String>>,
    store: PersistedSnapshotStore<String>,
}

impl Universe {
    fn new(config: PersistedSnapshotConfig, failures: usize) -> Self {
        let persistence: Arc<dyn SnapshotPersistence<String>> = if failures > 0 {
            Arc::new(FailingSnapshotPersistence::<String>::new(failures))
        } else {
            Arc::new(FakeSnapshotPersistence::<String>::new())
        };
        let store = PersistedSnapshotStore::new(Arc::clone(&persistence), config);
        Self { persistence, store }
    }

    async fn assert_not_persisted(&self, ids: &[&str]) {
        for id in ids {
            assert_eq!(
                self.persistence.get(id).await.unwrap(),
                None,
                "{id} persisted"
            );
        }
    }

    async fn assert_persisted(&self, items: &[(&str, ValidState<String>)]) {
        for (id, v) in items {
            assert_eq!(
                self.persistence.get(id).await.unwrap(),
                Some(v.clone()),
                "{id}"
            );
        }
    }

    async fn assert_not_present(&self, ids: &[&str]) {
        for id in ids {
            assert_eq!(self.store.get(id).await.unwrap(), None, "{id} present");
        }
    }

    async fn assert_present(&self, items: &[(&str, ValidState<String>)]) {
        for (id, v) in items {
            assert_eq!(self.store.get(id).await.unwrap(), Some(v.clone()), "{id}");
        }
    }
}

fn config(
    size: usize,
    max_buffer: usize,
    max_wait: Duration,
    flush_on_exit: bool,
) -> PersistedSnapshotConfig {
    PersistedSnapshotConfig {
        size,
        max_buffer,
        max_wait,
        flush_on_exit,
    }
}

const MINUTE: Duration = Duration::from_secs(60);

#[tokio::test(start_paused = true)]
async fn must_not_store_more_than_size() {
    let sut = Universe::new(config(1, 100, MINUTE, true), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.assert_present(&[("a", a())]).await;
    sut.store.put("b", b()).await.unwrap();
    sut.assert_not_present(&["a"]).await;
    sut.assert_present(&[("b", b())]).await;
    sut.store.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn must_be_convergent() {
    let sut = Universe::new(config(1, 100, MINUTE, true), 0);
    sut.store.put("a", aggregate("aa", 2)).await.unwrap();
    sut.store.put("a", aggregate("a", 1)).await.unwrap();
    sut.assert_present(&[("a", aggregate("aa", 2))]).await;
    sut.store.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn must_persist_evicted_items_asynchronously_when_max_wait_reached() {
    let sut = Universe::new(config(1, 100, MINUTE, true), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    sut.assert_not_persisted(&["a", "b"]).await;
    tokio::time::sleep(Duration::from_secs(61)).await;
    sut.assert_persisted(&[("a", a())]).await;
    sut.assert_not_persisted(&["b"]).await;
}

#[tokio::test(start_paused = true)]
async fn must_persist_evicted_items_asynchronously_when_max_buffer_reached() {
    let sut = Universe::new(config(1, 1, MINUTE, true), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    sut.assert_not_persisted(&["a", "b"]).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    sut.assert_persisted(&[("a", a())]).await;
    sut.assert_not_persisted(&["b"]).await;
}

#[tokio::test(start_paused = true)]
async fn must_not_flush_on_exit_if_flush_on_exit_is_false() {
    let sut = Universe::new(config(1000, 100, MINUTE, false), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    sut.assert_not_persisted(&["a", "b"]).await;
    sut.store.close().await.unwrap();
    assert_eq!(sut.persistence.get("a").await.unwrap(), None);
    assert_eq!(sut.persistence.get("b").await.unwrap(), None);
}

#[tokio::test(start_paused = true)]
async fn must_flush_on_exit_if_flush_on_exit_is_true() {
    let sut = Universe::new(config(1000, 100, MINUTE, true), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    sut.assert_not_persisted(&["a", "b"]).await;
    sut.store.close().await.unwrap();
    assert_eq!(sut.persistence.get("a").await.unwrap(), Some(a()));
    assert_eq!(sut.persistence.get("b").await.unwrap(), Some(b()));
}

#[tokio::test(start_paused = true)]
async fn must_not_persist_anything_if_neither_evicted_or_requested_to_flush() {
    let sut = Universe::new(config(1000, 100, MINUTE, false), 0);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    sut.store.close().await.unwrap();
    assert_eq!(sut.persistence.get("a").await.unwrap(), None);
    assert_eq!(sut.persistence.get("b").await.unwrap(), None);
}

#[tokio::test(start_paused = true)]
async fn must_retry_persisting() {
    let sut = Universe::new(config(1, 1, MINUTE, false), 1);
    sut.store.put("a", a()).await.unwrap();
    sut.store.put("b", b()).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    sut.assert_persisted(&[("a", a())]).await;
}

#[tokio::test(start_paused = true)]
async fn get_falls_back_to_persistence() {
    let sut = Universe::new(config(1, 1, MINUTE, false), 0);
    sut.persistence
        .put(vec![("z".into(), aggregate("z", 9))])
        .await
        .unwrap();
    assert_eq!(sut.store.get_fast("z").await, None);
    assert_eq!(sut.store.get("z").await.unwrap(), Some(aggregate("z", 9)));
}

#[test]
fn dedup_keeps_the_latest_version_of_each_id() {
    let items = vec![
        ("a".to_string(), aggregate("a1", 1)),
        ("b".to_string(), aggregate("b1", 1)),
        ("a".to_string(), aggregate("a3", 3)),
        ("a".to_string(), aggregate("a2", 2)),
    ];
    assert_eq!(
        dedup(items),
        vec![
            ("a".to_string(), aggregate("a3", 3)),
            ("b".to_string(), aggregate("b1", 1)),
        ]
    );
}
