//! Runs the shared suites of `edomata-backend-tests` against PostgreSQL:
//! port of `SkunkCompatibilitySuite.scala` and `DoobieCompatibilitySuite.scala`
//! (one sqlx driver replaces both). Namespaces are prefixed with `sqlx_` so
//! that Rust and Scala test runs can share the same database, except for
//! the three `compatibility_*` schemas loaded from `testdata.sql`, which
//! are shared with the Scala suites on purpose.

mod common;

use std::sync::Arc;

use common::*;
use edomata_backend::eventsourcing::SnapshotPersistence;
use edomata_backend::eventsourcing::StorageDriver;
use edomata_backend_tests::cqrs;
use edomata_backend_tests::eventsourcing::{self as es, prepared_data};
use edomata_sqlx::SqlxDriver;

// --- BackendCompatibilitySuite: json / jsonb / binary (testdata.sql) --------

macro_rules! compatibility_suite {
    ($module:ident, $schema:literal, $codec:expr) => {
        mod $module {
            use super::*;

            #[tokio::test]
            async fn must_read_all_journal() {
                prepared_data::must_read_all_journal(&backend(schema($schema), $codec).await).await;
            }

            #[tokio::test]
            async fn must_read_all_outbox_items() {
                prepared_data::must_read_all_outbox_items(&backend(schema($schema), $codec).await)
                    .await;
            }

            #[tokio::test]
            async fn must_load_for_non_existing_command_id() {
                prepared_data::must_load_for_non_existing_command_id(
                    &backend(schema($schema), $codec).await,
                )
                .await;
            }

            #[tokio::test]
            async fn must_skip_loading_for_existing_command_id() {
                prepared_data::must_skip_loading_for_existing_command_id(
                    &backend(schema($schema), $codec).await,
                )
                .await;
            }
        }
    };
}

compatibility_suite!(compatibility_json, "compatibility_json", json_codec());
compatibility_suite!(compatibility_jsonb, "compatibility_jsonb", jsonb_codec());
compatibility_suite!(compatibility_binary, "compatibility_binary", bin_codec());

// --- PersistenceSuite: schema, keyword namespace, prefixed ------------------

macro_rules! persistence_suite {
    ($module:ident, $naming:expr) => {
        mod $module {
            use super::*;

            /// The Scala `StorageSuite` runs its checks one after the other
            /// on a shared storage; `must_consume_outbox_correctly` marks
            /// every unpublished item as sent, so the checks of one namespace
            /// must not interleave. This lock keeps them sequential while
            /// other modules still run in parallel.
            static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

            #[tokio::test]
            async fn must_append_correctly() {
                let _guard = LOCK.lock().await;
                es::must_append_correctly(&backend($naming, jsonb_codec()).await).await;
            }

            #[tokio::test]
            async fn appending_must_be_idempotent() {
                let _guard = LOCK.lock().await;
                es::appending_must_be_idempotent(&backend($naming, jsonb_codec()).await).await;
            }

            #[tokio::test]
            async fn must_notify_correctly() {
                let _guard = LOCK.lock().await;
                es::must_notify_correctly(&backend($naming, jsonb_codec()).await).await;
            }

            #[tokio::test]
            async fn must_consume_outbox_correctly() {
                let _guard = LOCK.lock().await;
                es::must_consume_outbox_correctly(&backend($naming, jsonb_codec()).await).await;
            }

            #[tokio::test]
            async fn must_read_all_journal() {
                let _guard = LOCK.lock().await;
                es::must_read_all_journal(&backend($naming, jsonb_codec()).await).await;
            }

            #[tokio::test]
            async fn must_read_single_stream_from_journal() {
                let _guard = LOCK.lock().await;
                es::must_read_single_stream_from_journal(&backend($naming, jsonb_codec()).await)
                    .await;
            }
        }
    };
}

persistence_suite!(persistence, schema("sqlx_persistence"));
// `order` is a SQL keyword: schema-qualified quoting must cope with it.
persistence_suite!(persistence_keyword_namespace, schema("order"));
persistence_suite!(persistence_prefixed, prefixed("sqlx_pfx"));

// --- SnapshotPersistenceSuite ----------------------------------------------

async fn snapshot_persistence() -> Arc<dyn SnapshotPersistence<i32>> {
    let driver = SqlxDriver::new(schema("sqlx_snapshot_compatibility_json"), pool().await)
        .await
        .unwrap();
    driver.snapshot(json_codec()).await.unwrap()
}

#[tokio::test]
async fn snapshot_must_read_whats_written_single() {
    es::snapshot_must_read_whats_written_single(&*snapshot_persistence().await).await;
}

#[tokio::test]
async fn snapshot_must_read_whats_written_chunk() {
    es::snapshot_must_read_whats_written_chunk(&*snapshot_persistence().await).await;
}

#[tokio::test]
async fn snapshot_must_deduplicate_write_chunk() {
    es::snapshot_must_deduplicate_write_chunk(&*snapshot_persistence().await).await;
}

#[tokio::test]
async fn snapshot_must_write_latest_items_in_chunk() {
    es::snapshot_must_write_latest_items_in_chunk(&*snapshot_persistence().await).await;
}

// --- CqrsSuite: schema and prefixed ------------------------------------------

macro_rules! cqrs_suite {
    ($module:ident, $naming:expr) => {
        mod $module {
            use super::*;

            #[tokio::test]
            async fn inserts_state() {
                cqrs::inserts_state(&backend_cqrs($naming, json_codec()).await).await;
            }

            #[tokio::test]
            async fn updates_existing_state() {
                cqrs::updates_existing_state(&backend_cqrs($naming, json_codec()).await).await;
            }

            #[tokio::test]
            async fn publishes_notifications() {
                cqrs::publishes_notifications(&backend_cqrs($naming, json_codec()).await).await;
            }

            #[tokio::test]
            async fn save_must_be_idempotent() {
                cqrs::save_must_be_idempotent(&backend_cqrs($naming, json_codec()).await).await;
            }

            #[tokio::test]
            async fn save_must_be_correct() {
                cqrs::save_must_be_correct(&backend_cqrs($naming, json_codec()).await).await;
            }
        }
    };
}

cqrs_suite!(cqrs_schema, schema("sqlx_cqrs"));
cqrs_suite!(cqrs_prefixed, prefixed("sqlx_pfx_cqrs"));
