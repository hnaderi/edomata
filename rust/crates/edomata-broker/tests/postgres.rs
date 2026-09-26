//! PostgreSQL parts: leader election, `LISTEN/NOTIFY` wake-ups, the
//! checkpoint table, and two relays competing on one outbox.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use edomata_backend::eventsourcing::Backend;
use edomata_broker::postgres::{LeaderLock, PgCheckpointStore, listen};
use edomata_broker::{CancellationToken, CheckpointStore, OutboxRelay, RecordingPublisher};
use edomata_core::DomainModel;
use edomata_postgres::{PGNaming, PGSchema};
use edomata_sqlx::{PgPool, SqlxDriver};
use sqlx::postgres::PgPoolOptions;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

async fn pg_backend(pool: &PgPool, ns: &str, notify: Option<&str>) -> TestBackend {
    sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{ns}\" CASCADE"))
        .execute(pool)
        .await
        .unwrap();
    let mut driver = SqlxDriver::for_namespace(ns, pool.clone()).await.unwrap();
    if let Some(channel) = notify {
        driver = driver.with_outbox_notify_channel(channel);
    }
    Backend::builder(Counter, Counter.dsl::<i32, Notif>())
        .driver(driver)
        .build_default()
        .await
        .unwrap()
}

#[tokio::test]
async fn leader_lock_is_exclusive_until_released() {
    let pool = pool().await;
    let source = format!("lock-{}", uuid::Uuid::new_v4());
    let lock_a = LeaderLock::new(pool.clone(), &source);
    let lock_b = LeaderLock::new(pool.clone(), &source);
    assert_eq!(lock_a.key(), format!("edomata-relay:{source}"));
    let mut guard = lock_a.try_acquire().await.unwrap().expect("first acquires");
    assert!(guard.is_held().await);
    assert!(
        lock_b.try_acquire().await.unwrap().is_none(),
        "second is refused"
    );
    guard.release().await;
    let guard_b = lock_b
        .try_acquire()
        .await
        .unwrap()
        .expect("acquired after release");
    // Dropping the guard closes its connection, which releases the lock.
    drop(guard_b);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(lock_a.try_acquire().await.unwrap().is_some());
}

#[tokio::test]
async fn pg_checkpoint_store_round_trips() {
    let pool = pool().await;
    let naming = PGNaming::prefixed_str("relay_ckpt_test").unwrap();
    sqlx::query("DROP TABLE IF EXISTS relay_ckpt_test_relay_checkpoints")
        .execute(&pool)
        .await
        .unwrap();
    let ddl = PGSchema::relay_checkpoints(&naming);
    assert_eq!(
        ddl,
        vec![
            "CREATE TABLE IF NOT EXISTS relay_ckpt_test_relay_checkpoints (\n  relay text NOT NULL,\n  seqnr int8 NOT NULL,\n  updated_at timestamptz NOT NULL DEFAULT now(),\n  CONSTRAINT relay_ckpt_test_relay_checkpoints_pk PRIMARY KEY (relay)\n);".to_string()
        ]
    );
    // The default DDL is untouched (byte-identical to Scala).
    assert!(
        PGSchema::eventsourcing(&naming)
            .iter()
            .all(|s| !s.contains("relay_checkpoints"))
    );
    let store = PgCheckpointStore::new(pool.clone(), naming);
    store.setup().await.unwrap();
    store.setup().await.unwrap();
    assert_eq!(store.load("r").await.unwrap(), None);
    store.save("r", 10).await.unwrap();
    store.save("other", 3).await.unwrap();
    store.save("r", 12).await.unwrap();
    assert_eq!(store.load("r").await.unwrap(), Some(12));
    assert_eq!(store.load("other").await.unwrap(), Some(3));
}

#[tokio::test]
async fn listen_notify_wakes_a_relay_in_another_process() {
    let pool = pool().await;
    let channel = "edomata_relay_test";
    let backend = pg_backend(&pool, "relay_listen", Some(channel)).await;
    let publisher = Arc::new(RecordingPublisher::new());
    // No in-process signal: this relay only knows the LISTEN stream (and a
    // polling fallback too long to matter).
    let relay = Arc::new(
        OutboxRelay::new(
            Arc::clone(backend.outbox()),
            Arc::clone(&publisher) as Arc<_>,
            encoder(),
            config("relay_listen").with_poll_interval(Duration::from_secs(60)),
        )
        .wake_on(listen(pool.clone(), channel)),
    );
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let relay = Arc::clone(&relay);
        let cancel = cancel.clone();
        async move { relay.run(cancel).await }
    });
    // Let the listener subscribe.
    tokio::time::sleep(Duration::from_millis(500)).await;
    write(&backend, &["a"], 2).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while publisher.messages().len() < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("NOTIFY woke the relay up");
    cancel.cancel();
    running.await.unwrap().unwrap();
    backend.close().await.unwrap();
}

#[tokio::test]
async fn only_the_leader_publishes_and_a_standby_takes_over() {
    let pool = pool().await;
    let backend = pg_backend(&pool, "relay_leader", None).await;
    let source = format!("relay_leader-{}", uuid::Uuid::new_v4());
    let (pub_a, pub_b) = (
        Arc::new(RecordingPublisher::new()),
        Arc::new(RecordingPublisher::new()),
    );
    let relay_a = Arc::new(OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::clone(&pub_a) as Arc<_>,
        encoder(),
        config(&source),
    ));
    let relay_b = Arc::new(OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::clone(&pub_b) as Arc<_>,
        encoder(),
        config(&source),
    ));
    let (cancel_a, cancel_b) = (CancellationToken::new(), CancellationToken::new());
    let run_a = tokio::spawn({
        let (relay, cancel, pool) = (Arc::clone(&relay_a), cancel_a.clone(), pool.clone());
        async move {
            relay
                .run_as_leader(LeaderLock::new(pool, &source_of(&relay)), cancel)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let run_b = tokio::spawn({
        let (relay, cancel, pool) = (Arc::clone(&relay_b), cancel_b.clone(), pool.clone());
        async move {
            relay
                .run_as_leader(LeaderLock::new(pool, &source_of(&relay)), cancel)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    write(&backend, &["a", "b"], 5).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while pub_a.messages().len() < 10 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the leader relayed everything");
    assert_eq!(pub_b.messages().len(), 0, "the stand-by published nothing");
    // The relay marks a batch as sent right after the broker acknowledged
    // it, so the marking may lag the publication by a moment.
    tokio::time::timeout(Duration::from_secs(10), async {
        while !pending(backend.outbox().as_ref()).await.is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the leader marked everything as sent");

    // The leader stops: the stand-by takes over.
    cancel_a.cancel();
    run_a.await.unwrap().unwrap();
    write(&backend, &["c"], 3).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while pub_b.messages().len() < 3 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the stand-by became the leader");
    assert_eq!(pub_a.messages().len(), 10);
    let all: Vec<String> = pub_a.ids().into_iter().chain(pub_b.ids()).collect();
    let unique: std::collections::BTreeSet<&String> = all.iter().collect();
    assert_eq!(unique.len(), 13, "no duplicates with a single leader");
    cancel_b.cancel();
    run_b.await.unwrap().unwrap();
    backend.close().await.unwrap();
}

fn source_of(relay: &OutboxRelay<Notif>) -> String {
    relay.config().source.clone()
}
