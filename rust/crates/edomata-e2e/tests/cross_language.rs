//! The Scala/Rust cross-language compatibility test (plan, "Wire
//! compatibility" item 3): Scala writes events, commands, outbox items and
//! a snapshot; Rust reads them back and appends more; then Scala reads the
//! result.
//!
//! The Scala side is `modules/e2e/src/test/scala/CrossLanguage.scala`, run
//! through `sbt` (the Scala build is the external oracle, so `sbt` and a
//! JDK must be installed; set `EDOMATA_SBT` to point at the binary). Both
//! sides use the PostgreSQL instance of `DATABASE_URL` (docker-compose
//! defaults otherwise).

use std::path::PathBuf;
use std::process::Command as Process;

use edomata_backend::eventsourcing::{
    AggregateState, Backend, PersistedSnapshotConfig, ValidState,
};
use edomata_backend::{BackendError, RetryConfig};
use edomata_core::{CommandMessage, DomainModel};
use edomata_e2e::*;
use edomata_sqlx::{PgPool, SqlxCodec, SqlxDriver};
use futures::TryStreamExt;

const NAMESPACE: &str = "cross_language";
const ACCOUNT: &str = "xl-account-1";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string())
}

/// `postgres://user:password@host:port/database` → libpq environment
/// variables for the Scala side.
fn pg_env(url: &str) -> Vec<(&'static str, String)> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let (credentials, location) = rest.split_once('@').unwrap_or(("postgres:postgres", rest));
    let (user, password) = credentials
        .split_once(':')
        .unwrap_or((credentials, "postgres"));
    let (host_port, database) = location.split_once('/').unwrap_or((location, "postgres"));
    let database = database.split('?').next().unwrap_or("postgres");
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    vec![
        ("PGHOST", host.to_string()),
        ("PGPORT", port.to_string()),
        ("PGUSER", user.to_string()),
        ("PGPASSWORD", password.to_string()),
        ("PGDATABASE", database.to_string()),
    ]
}

/// Runs the Scala side (`write` or `verify`) and fails with its output when
/// it does not succeed.
fn scala(phase: &str) {
    let sbt = std::env::var("EDOMATA_SBT").unwrap_or_else(|_| "sbt".to_string());
    let task = format!("e2eTestsJVM/Test/runMain crosslang.CrossLanguage {phase} {NAMESPACE}");
    let mut process = Process::new(&sbt);
    process.arg("-batch").arg(&task).current_dir(repo_root());
    for (k, v) in pg_env(&database_url()) {
        process.env(k, v);
    }
    let output = process
        .output()
        .unwrap_or_else(|e| panic!("could not run `{sbt}` (set EDOMATA_SBT or install sbt): {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Scala `{phase}` phase failed ({}):\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains(&format!(
            "[scala] {}",
            if phase == "write" {
                "wrote"
            } else {
                "verified"
            }
        )),
        "Scala `{phase}` phase did not report success:\n{stdout}"
    );
}

async fn backend(pool: PgPool) -> Backend<Account, Event, Rejection, Notification> {
    let driver = SqlxDriver::for_namespace(NAMESPACE, pool).await.unwrap();
    Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .persisted_snapshot_with(
            SqlxCodec::<Account>::jsonb(),
            PersistedSnapshotConfig {
                size: 100,
                max_buffer: 1,
                flush_on_exit: true,
                ..PersistedSnapshotConfig::default()
            },
        )
        .with_retry_config(RetryConfig {
            max_retry: 1,
            ..RetryConfig::default()
        })
        .build_default()
        .await
        .unwrap()
}

fn cmd(id: &str, command: Command) -> CommandMessage<Command> {
    CommandMessage::new(id, chrono::Utc::now(), ACCOUNT, command)
}

fn open(balance: i64, version: i64) -> AggregateState<Account, Event, Rejection> {
    AggregateState::Valid(ValidState {
        state: Account::open_with(Amount::from(balance)),
        version,
    })
}

fn balance_updated(balance: i64) -> Notification {
    Notification::BalanceUpdated {
        account_id: ACCOUNT.to_string(),
        balance: Amount::from(balance),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scala_writes_rust_reads_and_appends_scala_verifies() {
    let pool = pool().await;
    sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{NAMESPACE}\" CASCADE"))
        .execute(&pool)
        .await
        .unwrap();

    // 1. Scala writes 3 events, 3 commands, 3 outbox items and a snapshot.
    tokio::task::spawn_blocking(|| scala("write"))
        .await
        .unwrap();

    // 2. Rust reads everything Scala wrote...
    let backend = backend(pool.clone()).await;
    let snapshot: (i64, serde_json::Value) = sqlx::query_as(&format!(
        "select \"version\", state from \"{NAMESPACE}\".snapshots where id = $1"
    ))
    .bind(ACCOUNT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snapshot.0, 3,
        "Scala must have persisted the snapshot on shutdown"
    );
    assert_eq!(snapshot.1, serde_json::json!({"Open": {"balance": 150}}));
    // ... through the repository (snapshot + journal),
    assert_eq!(
        backend.repository().get(ACCOUNT).await.unwrap(),
        open(150, 3)
    );
    // ... the journal,
    let events: Vec<(i64, Event)> = backend
        .journal()
        .read_stream(ACCOUNT)
        .map_ok(|e| (e.metadata.version, e.payload))
        .try_collect()
        .await
        .unwrap();
    assert_eq!(
        events,
        vec![
            (0, Event::Opened {}),
            (
                1,
                Event::Deposited {
                    amount: Amount::from(100)
                }
            ),
            (
                2,
                Event::Deposited {
                    amount: Amount::from(50)
                }
            ),
        ]
    );
    // ... the outbox,
    let outbox: Vec<Notification> = backend
        .outbox()
        .read()
        .map_ok(|i| i.data)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(
        outbox,
        vec![
            Notification::AccountOpened {
                account_id: ACCOUNT.to_string()
            },
            balance_updated(100),
            balance_updated(150),
        ]
    );
    // ... and the commands table (Scala's command ids are redundant now).
    let service = backend.compile(account_service());
    assert_eq!(
        service(cmd("scala-deposit-100", Command::Deposit(Amount::from(1))))
            .await
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        backend.repository().get(ACCOUNT).await.unwrap(),
        open(150, 3)
    );

    // 3. Rust appends more.
    assert_eq!(
        service(cmd("rust-deposit-25", Command::Deposit(Amount::from(25))))
            .await
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        service(cmd("rust-withdraw-5", Command::Withdraw(Amount::from(5))))
            .await
            .unwrap(),
        Ok(())
    );
    let rejected = service(cmd(
        "rust-withdraw-999",
        Command::Withdraw(Amount::from(999)),
    ))
    .await
    .unwrap();
    assert_eq!(
        rejected.unwrap_err().head(),
        &Rejection::InsufficientBalance
    );
    assert_eq!(
        backend.repository().get(ACCOUNT).await.unwrap(),
        open(170, 5)
    );
    // Closing flushes the snapshot for Scala to read.
    backend.close().await.unwrap();
    let version: i64 = sqlx::query_scalar(&format!(
        "select \"version\" from \"{NAMESPACE}\".snapshots where id = $1"
    ))
    .bind(ACCOUNT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(version, 5);

    // 4. Scala verifies the state, journal, outbox, commands and snapshot.
    tokio::task::spawn_blocking(|| scala("verify"))
        .await
        .unwrap();
}

#[test]
fn pg_env_parses_connection_urls() {
    let env = pg_env("postgres://alice:secret@db.example:6543/edomata?sslmode=disable");
    assert_eq!(
        env,
        vec![
            ("PGHOST", "db.example".to_string()),
            ("PGPORT", "6543".to_string()),
            ("PGUSER", "alice".to_string()),
            ("PGPASSWORD", "secret".to_string()),
            ("PGDATABASE", "edomata".to_string()),
        ]
    );
    let env = pg_env("postgres://postgres:postgres@localhost:5432/postgres");
    assert_eq!(env[0].1, "localhost");
    assert_eq!(env[4].1, "postgres");
}

#[allow(dead_code)]
fn unused(_: BackendError) {}
