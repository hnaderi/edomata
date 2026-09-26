//! End-to-end tests of `SimpleBackend` on PostgreSQL (the Scala `java-api`
//! module has no such test; `JBackend` is only exercised in the docs).

use chrono::Utc;
use edomata_simple::*;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

async fn drop_prefixed(pool: &PgPool, prefix: &str) {
    for t in ["journal", "outbox", "commands", "snapshots", "migrations"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {prefix}_{t}"))
            .execute(pool)
            .await
            .unwrap();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Event {
    Deposited(i64),
    Withdrawn(i64),
}

fn account_model() -> ClosureModel<i64, Event, String> {
    ClosureModel::new(0, |event: &Event, balance: i64| match event {
        Event::Deposited(n) => Ok(balance + n),
        Event::Withdrawn(n) if *n <= balance => Ok(balance - n),
        Event::Withdrawn(_) => Err(vec!["insufficient balance".to_string()]),
    })
}

type Handler = CommandHandler<String, i64, Event, String, String>;

fn handler() -> Handler {
    CommandHandler::new(
        |ctx: Context<String, i64>| match ctx.command.split_once(' ') {
            Some(("deposit", n)) => {
                let n: i64 = n.parse().unwrap();
                AppResult::accept([Event::Deposited(n)]).and_publish([format!("deposited {n}")])
            }
            Some(("withdraw", n)) => {
                let n: i64 = n.parse().unwrap();
                if n > ctx.state {
                    AppResult::reject(["insufficient balance".to_string()])
                        .and_publish(["overdraft attempt".to_string()])
                } else {
                    AppResult::accept([Event::Withdrawn(n)])
                }
            }
            _ => AppResult::reject([format!("unknown command: {}", ctx.command)]),
        },
    )
}

fn cmd(id: &str, address: &str, payload: &str) -> CommandMessage<String> {
    CommandMessage::new(id, Utc::now(), address, payload.to_string())
}

#[tokio::test]
async fn builder_reports_missing_and_invalid_configuration() {
    let err = SimpleBackend::<i64, Event, String, String>::builder(account_model())
        .build()
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "namespace is required");
    let err = SimpleBackend::<i64, Event, String, String>::builder(account_model())
        .namespace("1-bad")
        .build()
        .await
        .unwrap_err();
    assert!(matches!(err, SimpleError::InvalidNamespace(_)), "{err}");
    let err = SimpleBackend::<i64, Event, String, String>::builder(account_model())
        .namespace("simple_missing")
        .build()
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "eventCodec is required");
    let err = SimpleBackend::<i64, Event, String, String>::builder(account_model())
        .namespace("simple_missing")
        .serde_codecs()
        .build()
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "pool or databaseUrl is required");
    let err = SimpleBackend::<i64, Event, String, String>::builder(account_model())
        .namespace("simple_missing")
        .serde_codecs()
        .database_url("not a connection url")
        .build()
        .await
        .unwrap_err();
    assert!(matches!(err, SimpleError::Connection(_)), "{err}");
}

#[tokio::test]
async fn handles_commands_and_reads_journal_and_outbox() {
    let pool = pool().await;
    drop_prefixed(&pool, "simple_accounts").await;
    let backend = SimpleBackend::builder(account_model())
        .namespace("simple_accounts")
        .pool(pool.clone())
        .serde_codecs()
        .max_retry(2)
        .in_mem_snapshot_size(10)
        .build()
        .await
        .unwrap();
    let h = handler();

    assert_eq!(
        backend
            .handle(&h, cmd("c1", "acc-1", "deposit 100"))
            .await
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        backend
            .handle(&h, cmd("c2", "acc-1", "withdraw 30"))
            .await
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        backend
            .handle(&h, cmd("c3", "acc-1", "withdraw 500"))
            .await
            .unwrap(),
        Err(vec!["insufficient balance".to_string()])
    );
    assert_eq!(
        backend
            .handle(&h, cmd("c4", "acc-1", "dance"))
            .await
            .unwrap(),
        Err(vec!["unknown command: dance".to_string()])
    );
    // Redundant commands are accepted silently.
    assert_eq!(
        backend
            .handle(&h, cmd("c1", "acc-1", "deposit 100"))
            .await
            .unwrap(),
        Ok(())
    );
    // A compiled service can be reused.
    let service = backend.compile(&h);
    assert_eq!(
        service(cmd("c5", "acc-2", "deposit 5")).await.unwrap(),
        Ok(())
    );

    let journal = backend.journal();
    let events: Vec<(i64, Event)> = journal
        .read_stream("acc-1")
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e.metadata.version, e.payload))
        .collect();
    assert_eq!(
        events,
        vec![(0, Event::Deposited(100)), (1, Event::Withdrawn(30))]
    );
    assert_eq!(
        journal.read_stream_after("acc-1", 0).await.unwrap().len(),
        1
    );
    let all = journal.read_all().await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        journal
            .read_all_after(all[0].metadata.seq_nr)
            .await
            .unwrap()
            .len(),
        2
    );

    let outbox = backend.outbox();
    let items = outbox.read().await.unwrap();
    let data: Vec<&str> = items.iter().map(|i| i.data.as_str()).collect();
    assert_eq!(
        data,
        vec!["deposited 100", "overdraft attempt", "deposited 5"]
    );
    outbox.mark_as_sent(&items[0]).await.unwrap();
    outbox.mark_all_as_sent(&items[1..]).await.unwrap();
    outbox.mark_all_as_sent(&[]).await.unwrap();
    assert!(outbox.read().await.unwrap().is_empty());

    let state = backend.inner().repository().get("acc-1").await.unwrap();
    assert_eq!(
        state.as_valid().map(|v| (v.state, v.version)),
        Some((70, 2))
    );
    backend.close().await.unwrap();
}

#[tokio::test]
async fn skip_setup_uses_tables_created_from_simple_pgschema() {
    let pool = pool().await;
    drop_prefixed(&pool, "simple_flyway").await;
    for statement in SimplePGSchema::eventsourcing("simple_flyway").unwrap() {
        sqlx::query(&statement).execute(&pool).await.unwrap();
    }
    let codec = ClosureCodec::<Event>::new(
        |e| serde_json::to_string(e).unwrap(),
        |json| serde_json::from_str(json).map_err(|e| e.to_string()),
    );
    let backend = SimpleBackend::builder(account_model())
        .schema_namespace("simple_flyway_schema")
        .namespace("simple_flyway")
        .pool(pool.clone())
        .simple_event_codec(codec)
        .simple_notification_codec(ClosureCodec::<String>::new(
            |s| serde_json::to_string(s).unwrap(),
            |json| serde_json::from_str(json).map_err(|e| e.to_string()),
        ))
        .skip_setup(true)
        .build()
        .await
        .unwrap();
    assert_eq!(
        backend
            .handle(&handler(), cmd("f1", "acc-9", "deposit 7"))
            .await
            .unwrap(),
        Ok(())
    );
    let stored: String = sqlx::query_scalar(
        "select payload::text from simple_flyway_journal where stream = 'acc-9'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "{\"Deposited\": 7}");
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.schemata where schema_name = 'simple_flyway_schema')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !exists,
        "the last namespace setting wins and skip_setup creates nothing"
    );
    backend.close().await.unwrap();
}

#[test]
fn blocking_backend_works_outside_an_async_context() {
    let runtime = SimpleRuntime::create().unwrap();
    let pool = runtime.block_on(pool());
    runtime.block_on(drop_prefixed(&pool, "simple_blocking"));
    let backend = SimpleBackend::builder(account_model())
        .namespace("simple_blocking")
        .pool(pool)
        .serde_codecs()
        .build_blocking(runtime.clone())
        .unwrap();
    let h = handler();
    assert_eq!(
        backend
            .handle(&h, cmd("b1", "acc-b", "deposit 10"))
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        backend
            .handle(&h, cmd("b2", "acc-b", "withdraw 11"))
            .unwrap(),
        Err(vec!["insufficient balance".to_string()])
    );
    assert_eq!(backend.read_stream("acc-b").unwrap().len(), 1);
    assert_eq!(backend.read_stream_after("acc-b", 0).unwrap().len(), 0);
    assert_eq!(backend.read_all().unwrap().len(), 1);
    assert_eq!(backend.read_all_after(i64::MAX).unwrap().len(), 0);
    let items = backend.read_outbox().unwrap();
    assert_eq!(items.len(), 2);
    backend.mark_as_sent(&items[0]).unwrap();
    backend.mark_all_as_sent(&items[1..]).unwrap();
    assert!(backend.read_outbox().unwrap().is_empty());
    assert!(backend.runtime().owns_runtime());
    backend.close().unwrap();
    drop(backend);
    runtime.close();
}
