//! Runs the shared suites against the in-memory driver (port of the
//! `StorageSuite` wiring, with a fresh backend per check).

use std::sync::Arc;
use std::time::Duration;

use edomata_backend::RetryConfig;
use edomata_backend::cqrs::Backend as CqrsBackend;
use edomata_backend::eventsourcing::{Backend, SnapshotPersistence};
use edomata_backend::inmemory::{InMemoryDriver, InMemoryEventStore, InMemorySnapshotPersistence};
use edomata_backend_tests::eventsourcing::{self as es, EsBackend, prepared_data};
use edomata_backend_tests::{TestCqrsModel, TestDomain, cqrs, test_cqrs_dsl, test_domain_dsl};

fn retry() -> RetryConfig {
    RetryConfig {
        max_retry: 5,
        initial_delay: Duration::from_millis(5),
    }
}

async fn es_backend() -> EsBackend {
    Backend::builder(TestDomain, test_domain_dsl())
        .driver(InMemoryDriver::new())
        .with_retry_config(retry())
        .build_default()
        .await
        .unwrap()
}

async fn cqrs_backend() -> cqrs::CqrsBackend {
    CqrsBackend::builder(TestCqrsModel, test_cqrs_dsl())
        .driver(InMemoryDriver::new())
        .with_retry_config(retry())
        .build_default()
        .await
        .unwrap()
}

/// A backend over a store seeded with `PreparedData`, using persisted
/// snapshots so that the seeded snapshot is read.
async fn compatibility_backend() -> EsBackend {
    let store = Arc::new(InMemoryEventStore::<i32, i32, i32>::new());
    store.seed_journal(prepared_data::journal());
    store.seed_outbox(prepared_data::outbox());
    store.seed_commands([prepared_data::REDUNDANT_CMD]);
    store.seed_snapshots([(
        prepared_data::STREAM_ID.to_string(),
        prepared_data::aggregate(),
    )]);
    let driver = InMemoryDriver::with_event_store(store);
    Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .persisted_snapshot(())
        .build_default()
        .await
        .unwrap()
}

// --- PersistenceSuite (in-memory) ------------------------------------------

#[tokio::test]
async fn persistence_must_append_correctly() {
    es::must_append_correctly(&es_backend().await).await;
}

#[tokio::test]
async fn persistence_appending_must_be_idempotent() {
    es::appending_must_be_idempotent(&es_backend().await).await;
}

#[tokio::test]
async fn persistence_must_notify_correctly() {
    es::must_notify_correctly(&es_backend().await).await;
}

#[tokio::test]
async fn persistence_must_consume_outbox_correctly() {
    es::must_consume_outbox_correctly(&es_backend().await).await;
}

#[tokio::test]
async fn persistence_must_read_all_journal() {
    es::must_read_all_journal(&es_backend().await).await;
}

#[tokio::test]
async fn persistence_must_read_single_stream_from_journal() {
    es::must_read_single_stream_from_journal(&es_backend().await).await;
}

// --- BackendCompatibilitySuite (in-memory) ---------------------------------

#[tokio::test]
async fn compatibility_must_read_all_journal() {
    prepared_data::must_read_all_journal(&compatibility_backend().await).await;
}

#[tokio::test]
async fn compatibility_must_read_all_outbox_items() {
    prepared_data::must_read_all_outbox_items(&compatibility_backend().await).await;
}

#[tokio::test]
async fn compatibility_must_load_for_non_existing_command_id() {
    prepared_data::must_load_for_non_existing_command_id(&compatibility_backend().await).await;
}

#[tokio::test]
async fn compatibility_must_skip_loading_for_existing_command_id() {
    prepared_data::must_skip_loading_for_existing_command_id(&compatibility_backend().await).await;
}

// --- SnapshotPersistenceSuite (in-memory) ----------------------------------

fn snapshot_persistence() -> Arc<dyn SnapshotPersistence<i32>> {
    Arc::new(InMemorySnapshotPersistence::<i32>::new())
}

#[tokio::test]
async fn snapshot_must_read_whats_written_single() {
    es::snapshot_must_read_whats_written_single(&*snapshot_persistence()).await;
}

#[tokio::test]
async fn snapshot_must_read_whats_written_chunk() {
    es::snapshot_must_read_whats_written_chunk(&*snapshot_persistence()).await;
}

#[tokio::test]
async fn snapshot_must_deduplicate_write_chunk() {
    es::snapshot_must_deduplicate_write_chunk(&*snapshot_persistence()).await;
}

#[tokio::test]
async fn snapshot_must_write_latest_items_in_chunk() {
    es::snapshot_must_write_latest_items_in_chunk(&*snapshot_persistence()).await;
}

// --- CqrsSuite (in-memory) -------------------------------------------------

#[tokio::test]
async fn cqrs_inserts_state() {
    cqrs::inserts_state(&cqrs_backend().await).await;
}

#[tokio::test]
async fn cqrs_updates_existing_state() {
    cqrs::updates_existing_state(&cqrs_backend().await).await;
}

#[tokio::test]
async fn cqrs_publishes_notifications() {
    cqrs::publishes_notifications(&cqrs_backend().await).await;
}

#[tokio::test]
async fn cqrs_save_must_be_idempotent() {
    cqrs::save_must_be_idempotent(&cqrs_backend().await).await;
}

#[tokio::test]
async fn cqrs_save_must_be_correct() {
    cqrs::save_must_be_correct(&cqrs_backend().await).await;
}
