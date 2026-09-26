//! Driver-level behaviour: `skip_setup`, namespace validation, catalog
//! names in prefixed mode, the CQRS transactional handler and version
//! conflicts on duplicate commands.

mod common;

use std::sync::Arc;

use common::*;
use edomata_backend::cqrs::Backend as CqrsBackend;
use edomata_backend::eventsourcing::Backend;
use edomata_backend::{BackendError, RetryConfig};
use edomata_backend_tests::{TestCqrsModel, TestDomain, test_cqrs_dsl, test_domain_dsl};
use edomata_core::{CommandMessage, Edomaton, NonEmpty, ResponseD};
use edomata_sqlx::{PGSchema, SqlxCqrsDriver, SqlxDriver, SqlxHandler};

async fn drop_schema(ns: &str) {
    sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{ns}\" CASCADE"))
        .execute(&pool().await)
        .await
        .unwrap();
}

async fn drop_prefixed(prefix: &str) {
    let pool = pool().await;
    for t in [
        "journal",
        "outbox",
        "commands",
        "snapshots",
        "migrations",
        "states",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {prefix}_{t}"))
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn skip_setup_executes_no_ddl() {
    drop_schema("sqlx_skip_setup").await;
    let pool = pool().await;
    let driver = SqlxDriver::new_with(schema("sqlx_skip_setup"), pool.clone(), true)
        .await
        .unwrap();
    assert!(!driver.auto_setup());
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.schemata where schema_name = 'sqlx_skip_setup')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!exists, "skip_setup must not create the schema");

    // Building a backend does not create tables either: using it fails.
    let backend = Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .build(jsonb_codec(), jsonb_codec())
        .await
        .unwrap();
    let app: edomata_core::App<String, i32, i32, String, i32, ()> =
        Edomaton::lift(ResponseD::accept(NonEmpty::new(1)));
    let result = backend.compile(app)(CommandMessage::new(
        "c",
        chrono::Utc::now(),
        "a",
        "x".to_string(),
    ))
    .await;
    assert!(
        matches!(result, Err(BackendError::UnknownError(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn skip_setup_works_with_tables_created_from_pgschema() {
    drop_prefixed("sqlx_flyway").await;
    let pool = pool().await;
    let naming = prefixed("sqlx_flyway");
    // The Flyway workflow: DDL from PGSchema, driver with skip_setup.
    for statement in PGSchema::eventsourcing(&naming) {
        sqlx::query(&statement).execute(&pool).await.unwrap();
    }
    let driver = SqlxDriver::new_with(naming, pool, true).await.unwrap();
    let backend = Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .persisted_snapshot(jsonb_codec())
        .with_retry_config(RetryConfig {
            max_retry: 1,
            ..RetryConfig::default()
        })
        .build(jsonb_codec(), jsonb_codec())
        .await
        .unwrap();
    let app: edomata_core::App<String, i32, i32, String, i32, ()> =
        Edomaton::lift(ResponseD::accept(NonEmpty::of(1, [2])));
    backend.compile(app)(CommandMessage::new(
        uuid::Uuid::new_v4().to_string(),
        chrono::Utc::now(),
        "flyway-a",
        "x".to_string(),
    ))
    .await
    .unwrap()
    .unwrap();
    let state = backend.repository().get("flyway-a").await.unwrap();
    assert_eq!(state.as_valid().map(|v| (v.state, v.version)), Some((3, 2)));
    backend.close().await.unwrap();
}

#[tokio::test]
async fn prefixed_mode_prefixes_constraints_and_indexes_in_the_catalog() {
    drop_prefixed("sqlx_catalog").await;
    let pool = pool().await;
    let driver = SqlxDriver::new(prefixed("sqlx_catalog"), pool.clone())
        .await
        .unwrap();
    let _backend = Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .build(jsonb_codec(), jsonb_codec())
        .await
        .unwrap();
    let constraints: Vec<String> = sqlx::query_scalar(
        "select conname::text from pg_constraint where conname like 'sqlx_catalog_%' order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        constraints,
        vec![
            "sqlx_catalog_commands_pk".to_string(),
            "sqlx_catalog_journal_pk".to_string(),
            "sqlx_catalog_journal_un".to_string(),
            "sqlx_catalog_outbox_pk".to_string(),
        ]
    );
    let indexes: Vec<String> = sqlx::query_scalar(
        "select indexname::text from pg_indexes where indexname like 'sqlx_catalog_journal_%_idx' order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        indexes,
        vec![
            "sqlx_catalog_journal_seqnr_idx".to_string(),
            "sqlx_catalog_journal_stream_idx".to_string(),
        ]
    );
}

#[tokio::test]
async fn for_namespace_validates_the_name() {
    let err = SqlxDriver::for_namespace("1-bad", pool().await)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
    let err = SqlxCqrsDriver::for_namespace("", pool().await)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
}

#[tokio::test]
async fn cqrs_handler_runs_inside_the_save_transaction() {
    drop_schema("sqlx_handler").await;
    let pool = pool().await;
    sqlx::query("create schema sqlx_handler")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("create table sqlx_handler.projection (value int not null)")
        .execute(&pool)
        .await
        .unwrap();
    let driver = SqlxCqrsDriver::new(schema("sqlx_handler"), pool.clone())
        .await
        .unwrap();
    let handler: SqlxHandler<i32> = SqlxHandler::new(|ns, conn| {
        Box::pin(async move {
            for n in ns.iter() {
                if *n == 13 {
                    return Err(BackendError::persistence("unlucky notification"));
                }
                sqlx::query("insert into sqlx_handler.projection(value) values ($1)")
                    .bind(n)
                    .execute(&mut *conn)
                    .await
                    .map_err(BackendError::unknown)?;
            }
            Ok(())
        })
    });
    let backend = CqrsBackend::builder(TestCqrsModel, test_cqrs_dsl())
        .driver(driver)
        .with_event_handler(handler)
        .with_retry_config(RetryConfig {
            max_retry: 1,
            ..RetryConfig::default()
        })
        .build(json_codec(), json_codec())
        .await
        .unwrap();
    let dsl = test_cqrs_dsl();
    let service = backend.compile(dsl.router(move |n| dsl.publish([n]).then(dsl.set(n))));

    service(CommandMessage::new("h1", chrono::Utc::now(), "agg", 7))
        .await
        .unwrap()
        .unwrap();
    let projected: Vec<i32> = sqlx::query_scalar("select value from sqlx_handler.projection")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(projected, vec![7]);

    // A failing handler rolls the whole save back: no state, no outbox, no command.
    let err = service(CommandMessage::new("h2", chrono::Utc::now(), "agg", 13))
        .await
        .unwrap_err();
    assert_eq!(err, BackendError::persistence("unlucky notification"));
    assert_eq!(backend.repository().get("agg").await.unwrap().state, 7);
    let commands: i64 =
        sqlx::query_scalar("select count(*) from sqlx_handler.commands where id = 'h2'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(commands, 0);
}

#[tokio::test]
async fn duplicate_command_id_is_a_version_conflict() {
    drop_schema("sqlx_dup_cmd").await;
    let pool = pool().await;
    let driver = SqlxDriver::new(schema("sqlx_dup_cmd"), pool.clone())
        .await
        .unwrap();
    let backend = Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .disable_cache()
        .with_retry_config(RetryConfig {
            max_retry: 1,
            ..RetryConfig::default()
        })
        .build(jsonb_codec(), jsonb_codec())
        .await
        .unwrap();
    let app: edomata_core::App<String, i32, i32, String, i32, ()> =
        Edomaton::lift(ResponseD::accept(NonEmpty::new(1)));
    let service = backend.compile(app);
    service(CommandMessage::new(
        "dup",
        chrono::Utc::now(),
        "a",
        "x".to_string(),
    ))
    .await
    .unwrap()
    .unwrap();
    // Without the command cache the repository sees the duplicate id and
    // reports it as redundant (the command was already recorded).
    let again = service(CommandMessage::new(
        "dup",
        chrono::Utc::now(),
        "b",
        "x".to_string(),
    ))
    .await;
    assert_eq!(again, Ok(Ok(())));

    // A genuine race: two handlers load the same new command id before
    // either records it. The loser's insert violates the commands primary
    // key, which the driver maps to VersionConflict; with a single attempt
    // allowed the retry policy reports MaxRetryExceeded.
    let service = Arc::new(service);
    let racing = (0..8).map(|i| {
        let service = Arc::clone(&service);
        async move {
            service(CommandMessage::new(
                "race",
                chrono::Utc::now(),
                format!("race-{i}"),
                "x".to_string(),
            ))
            .await
        }
    });
    let outcomes = futures::future::join_all(racing).await;
    let winners = outcomes.iter().filter(|o| matches!(o, Ok(Ok(())))).count();
    assert!(winners >= 1, "{outcomes:?}");
    for outcome in &outcomes {
        match outcome {
            Ok(Ok(())) | Err(BackendError::MaxRetryExceeded) => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    let recorded: i64 =
        sqlx::query_scalar("select count(*) from sqlx_dup_cmd.commands where id = 'race'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recorded, 1);
    let events: i64 =
        sqlx::query_scalar("select count(*) from sqlx_dup_cmd.journal where stream like 'race-%'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 1, "only the winner's event is journaled");
}
