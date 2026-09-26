//! Event migration runner tests (port of the behaviour of
//! `SkunkMigrations` / `DoobieMigrations`: apply pending, skip applied,
//! rewrite payloads, truncate snapshots, fail atomically).

mod common;

use common::*;
use edomata_backend::eventsourcing::{StorageDriver, ValidState};
use edomata_backend_tests::eventsourcing::EsBackend;
use edomata_core::{CommandMessage, Edomaton, NonEmpty, ResponseD};
use edomata_sqlx::{EventMigration, MigrationResult, SqlxDriver, SqlxMigrations};
use futures::TryStreamExt;

async fn fresh_backend(ns: &str) -> (EsBackend, edomata_sqlx::PGNaming) {
    let naming = schema(ns);
    let pool = pool().await;
    sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{ns}\" CASCADE"))
        .execute(&pool)
        .await
        .unwrap();
    (backend(naming.clone(), jsonb_codec()).await, naming)
}

async fn append(b: &EsBackend, address: &str, events: NonEmpty<i32>) {
    let app: edomata_core::App<String, i32, i32, String, i32, ()> =
        Edomaton::lift(ResponseD::accept(events));
    let cmd = CommandMessage::new(uuid_str(), chrono::Utc::now(), address, "cmd".to_string());
    b.compile(app)(cmd).await.unwrap().unwrap();
}

fn uuid_str() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn double() -> EventMigration {
    EventMigration::typed(
        "001",
        "double every event",
        |raw| raw.parse::<i32>().map_err(|e| e.to_string()),
        |i| i * 2,
        |i: &i32| i.to_string(),
    )
}

fn add_one() -> EventMigration {
    EventMigration::new("002", "add one", |raw| {
        raw.parse::<i32>()
            .map(|i| (i + 1).to_string())
            .map_err(|e| e.to_string())
    })
}

async fn journal_of(b: &EsBackend, address: &str) -> Vec<i32> {
    b.journal()
        .read_stream(address)
        .map_ok(|e| e.payload)
        .try_collect()
        .await
        .unwrap()
}

#[tokio::test]
async fn applies_pending_migrations_and_skips_applied_ones() {
    let (b, naming) = fresh_backend("sqlx_migrations_apply").await;
    let pool = pool().await;
    append(&b, "a", NonEmpty::of(1, [2, 3])).await;

    let first = SqlxMigrations::run(&naming, &pool, &[double()])
        .await
        .unwrap();
    assert_eq!(
        first,
        MigrationResult {
            applied: vec!["001".into()],
            skipped: vec![]
        }
    );
    assert_eq!(journal_of(&b, "a").await, vec![2, 4, 6]);

    let second = SqlxMigrations::run(&naming, &pool, &[double(), add_one()])
        .await
        .unwrap();
    assert_eq!(
        second,
        MigrationResult {
            applied: vec!["002".into()],
            skipped: vec!["001".into()]
        }
    );
    assert_eq!(journal_of(&b, "a").await, vec![3, 5, 7]);

    let third = SqlxMigrations::run(&naming, &pool, &[double(), add_one()])
        .await
        .unwrap();
    assert_eq!(third.applied, Vec::<String>::new());
    assert_eq!(third.skipped, vec!["001".to_string(), "002".to_string()]);
}

#[tokio::test]
async fn truncates_snapshots_after_a_migration() {
    let (b, naming) = fresh_backend("sqlx_migrations_snapshots").await;
    let pool = pool().await;
    let driver = SqlxDriver::new(naming.clone(), pool.clone()).await.unwrap();
    let snapshots = driver.snapshot(jsonb_codec()).await.unwrap();
    snapshots
        .put(vec![("a".into(), ValidState::new(6, 3))])
        .await
        .unwrap();
    assert!(snapshots.get("a").await.unwrap().is_some());
    append(&b, "a", NonEmpty::new(1)).await;

    SqlxMigrations::run(&naming, &pool, &[double()])
        .await
        .unwrap();
    assert_eq!(snapshots.get("a").await.unwrap(), None);
}

#[tokio::test]
async fn a_failing_migration_changes_nothing() {
    let (b, naming) = fresh_backend("sqlx_migrations_failure").await;
    let pool = pool().await;
    append(&b, "a", NonEmpty::of(1, [2])).await;
    let failing = EventMigration::new("bad", "fails on 2", |raw| {
        if raw == "2" {
            Err("cannot migrate 2".to_string())
        } else {
            Ok(raw.to_string())
        }
    });
    let err = SqlxMigrations::run(&naming, &pool, &[double(), failing])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("Migration 'bad' failed for event"),
        "{err}"
    );
    assert!(err.to_string().contains("cannot migrate 2"), "{err}");
    // "001" was applied and committed; "bad" was rolled back.
    assert_eq!(journal_of(&b, "a").await, vec![2, 4]);
    let applied: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT \"version\" FROM {} ORDER BY 1",
        naming.table("migrations")
    ))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(applied, vec!["001".to_string()]);
}

#[tokio::test]
async fn batch_size_does_not_change_the_result() {
    let (b, naming) = fresh_backend("sqlx_migrations_batches").await;
    let pool = pool().await;
    append(&b, "a", NonEmpty::of(1, [2, 3, 4, 5])).await;
    SqlxMigrations::run_with_batch_size(&naming, &pool, &[double()], 2)
        .await
        .unwrap();
    assert_eq!(journal_of(&b, "a").await, vec![2, 4, 6, 8, 10]);
}
