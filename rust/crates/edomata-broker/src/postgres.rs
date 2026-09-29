//! PostgreSQL support for relays: leader election with advisory locks,
//! `LISTEN`-based wake-ups and a checkpoint table.

use std::time::Duration;

use async_trait::async_trait;
use edomata_postgres::{PGNaming, PGSchema};
use futures::stream::BoxStream;
use sqlx::postgres::PgListener;
use sqlx::{Connection, PgConnection, PgPool};

use crate::{CheckpointStore, RelayError};

fn leader_error(e: sqlx::Error) -> RelayError {
    RelayError::Leader(Box::new(e))
}

fn checkpoint_error(e: sqlx::Error) -> RelayError {
    RelayError::Checkpoint(Box::new(e))
}

/// Leader election through a session-level advisory lock
/// (`pg_try_advisory_lock(hashtext('edomata-relay:{source}'))`): the lock
/// is held by a dedicated connection for as long as the [`LeaderGuard`]
/// lives, and released when the guard is released or its connection closes.
///
/// Give every replica of one relay the same source (or key); give different
/// relays different ones. The key is hashed to a 32-bit advisory lock id,
/// so unrelated keys can collide, which only makes the relays wait for each
/// other. [`OutboxRelay::run_as_leader`](crate::OutboxRelay::run_as_leader)
/// and [`JournalRelay::run_as_leader`](crate::JournalRelay::run_as_leader)
/// drive the lock; it can also be used directly:
///
/// ```no_run
/// # use std::time::Duration;
/// # use edomata_broker::postgres::LeaderLock;
/// # async fn example(pool: sqlx::PgPool) -> Result<(), edomata_broker::RelayError> {
/// let lock = LeaderLock::new(pool, "accounts").with_health_interval(Duration::from_secs(2));
/// assert_eq!(lock.key(), "edomata-relay:accounts");
/// if let Some(mut guard) = lock.try_acquire().await? {
///     // This process is the leader until the guard is released or lost.
///     assert!(guard.is_held().await);
///     guard.release().await;
/// }
/// # Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct LeaderLock {
    pool: PgPool,
    key: String,
    health_interval: Duration,
}

impl LeaderLock {
    /// A lock for the relay of `source`, with the key
    /// `"edomata-relay:{source}"` and a 5-second health check.
    pub fn new(pool: PgPool, source: &str) -> Self {
        Self {
            pool,
            key: format!("edomata-relay:{source}"),
            health_interval: Duration::from_secs(5),
        }
    }

    /// A lock with an explicit key (used as is) and a 5-second health check.
    pub fn with_key(pool: PgPool, key: impl Into<String>) -> Self {
        Self {
            pool,
            key: key.into(),
            health_interval: Duration::from_secs(5),
        }
    }

    /// How often the leader checks that its connection (and thus the lock)
    /// is still alive.
    pub fn with_health_interval(mut self, interval: Duration) -> Self {
        self.health_interval = interval;
        self
    }

    /// The lock key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Tries to take the lock without waiting: `Some(guard)` when this
    /// process is now the leader, `None` when another session holds it. The
    /// guard owns a connection detached from the pool.
    ///
    /// # Errors
    ///
    /// [`RelayError::Leader`] when no connection can be acquired or the
    /// query fails.
    pub async fn try_acquire(&self) -> Result<Option<LeaderGuard>, RelayError> {
        let mut conn = self.pool.acquire().await.map_err(leader_error)?.detach();
        let acquired: bool = sqlx::query_scalar("select pg_try_advisory_lock(hashtext($1))")
            .bind(&self.key)
            .fetch_one(&mut conn)
            .await
            .map_err(leader_error)?;
        if acquired {
            Ok(Some(LeaderGuard {
                conn: Some(conn),
                key: self.key.clone(),
                health_interval: self.health_interval,
            }))
        } else {
            let _ = conn.close().await;
            Ok(None)
        }
    }
}

/// The held leader lock. Dropping it closes the connection, which releases
/// the lock server-side; [`release`](Self::release) does so explicitly.
#[derive(Debug)]
pub struct LeaderGuard {
    conn: Option<PgConnection>,
    key: String,
    health_interval: Duration,
}

impl LeaderGuard {
    /// Whether the lock's connection still answers (`select 1`). A session
    /// advisory lock lives as long as its session, so a live connection
    /// means the lock is still held.
    pub async fn is_held(&mut self) -> bool {
        match &mut self.conn {
            Some(conn) => sqlx::query("select 1").execute(&mut *conn).await.is_ok(),
            None => false,
        }
    }

    /// Resolves when the lock is lost (the connection stopped answering).
    pub async fn lost(&mut self) {
        loop {
            tokio::time::sleep(self.health_interval).await;
            if !self.is_held().await {
                return;
            }
        }
    }

    /// Releases the lock and closes the connection. Errors are ignored: if
    /// the unlock fails, closing the connection releases the lock anyway.
    pub async fn release(mut self) {
        if let Some(mut conn) = self.conn.take() {
            let _ = sqlx::query("select pg_advisory_unlock(hashtext($1))")
                .bind(&self.key)
                .execute(&mut conn)
                .await;
            let _ = conn.close().await;
        }
    }
}

/// A wake-up stream fed by `LISTEN channel`: yields once per `NOTIFY`
/// (payloads are ignored), reconnecting after connection errors. Pair it
/// with the drivers' `with_outbox_notify_channel` / `with_journal_notify_channel`
/// so that a relay in another process is woken up by the writers.
///
/// The stream never ends. After a lost connection it yields once (anything
/// written meanwhile gets relayed), then reconnects on the next poll,
/// retrying every second and logging a `tracing` warning; notifications
/// sent while disconnected are lost, which the relays' polling fallback
/// covers.
///
/// ```no_run
/// # use edomata_broker::{OutboxRelay, postgres::listen};
/// # fn example(relay: OutboxRelay<String>, pool: sqlx::PgPool) {
/// // The writer side: `SqlxDriver::...with_outbox_notify_channel("accounts_outbox")`.
/// let relay = relay.wake_on(listen(pool, "accounts_outbox"));
/// # let _ = relay; }
/// ```
pub fn listen(pool: PgPool, channel: impl Into<String>) -> BoxStream<'static, ()> {
    let channel = channel.into();
    Box::pin(futures::stream::unfold(
        (pool, channel, None::<PgListener>),
        |(pool, channel, listener)| async move {
            let mut listener = match listener {
                Some(l) => l,
                None => loop {
                    match connect(&pool, &channel).await {
                        Ok(l) => break l,
                        Err(e) => {
                            tracing::warn!(channel = %channel, error = %e, "LISTEN failed, retrying");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                },
            };
            match listener.recv().await {
                Ok(_) => Some(((), (pool, channel, Some(listener)))),
                Err(e) => {
                    tracing::warn!(channel = %channel, error = %e, "LISTEN connection lost, reconnecting");
                    // Reconnect on the next poll; report a wake-up so that
                    // anything written meanwhile is relayed.
                    Some(((), (pool, channel, None)))
                }
            }
        },
    ))
}

async fn connect(pool: &PgPool, channel: &str) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(channel).await?;
    Ok(listener)
}

/// A [`CheckpointStore`] in the `relay_checkpoints` table of a namespace
/// (DDL: [`PGSchema::relay_checkpoints`]), one row per relay name.
///
/// ```no_run
/// # use std::sync::Arc;
/// # use edomata_broker::CheckpointStore;
/// # use edomata_broker::postgres::PgCheckpointStore;
/// # use edomata_postgres::{PGNaming, PGNamespace};
/// # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// let naming = PGNaming::prefixed(PGNamespace::try_from("accounts")?);
/// let checkpoints = PgCheckpointStore::new(pool, naming); // table `accounts_relay_checkpoints`
/// checkpoints.setup().await?; // or create the table with a migration tool
/// assert_eq!(checkpoints.load("accounts:journal").await?, None);
/// checkpoints.save("accounts:journal", 42).await?;
/// let checkpoints: Arc<dyn CheckpointStore> = Arc::new(checkpoints); // for `JournalRelay::new`
/// # let _ = checkpoints; Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct PgCheckpointStore {
    pool: PgPool,
    naming: PGNaming,
    select: String,
    upsert: String,
}

impl PgCheckpointStore {
    /// A store for the tables of `naming`. It does not touch the database;
    /// call [`setup`](Self::setup) or create the table beforehand.
    pub fn new(pool: PgPool, naming: PGNaming) -> Self {
        let t = naming.table("relay_checkpoints");
        Self {
            pool,
            select: format!("select seqnr from {t} where relay = $1"),
            upsert: format!(
                "insert into {t} (relay, seqnr, updated_at) values ($1, $2, now()) on conflict (relay) do update set seqnr = excluded.seqnr, updated_at = now()"
            ),
            naming,
        }
    }

    /// Creates the checkpoint table (`CREATE TABLE IF NOT EXISTS`).
    /// Idempotent. It does not create the schema of a
    /// [`PGNaming::Schema`] naming, which the storage driver's setup (or a
    /// migration) creates.
    ///
    /// # Errors
    ///
    /// [`RelayError::Checkpoint`] when a statement fails.
    pub async fn setup(&self) -> Result<(), RelayError> {
        for statement in PGSchema::relay_checkpoints(&self.naming) {
            sqlx::query(&statement)
                .execute(&self.pool)
                .await
                .map_err(checkpoint_error)?;
        }
        Ok(())
    }
}

#[async_trait]
impl CheckpointStore for PgCheckpointStore {
    async fn load(&self, relay: &str) -> Result<Option<i64>, RelayError> {
        sqlx::query_scalar(&self.select)
            .bind(relay)
            .fetch_optional(&self.pool)
            .await
            .map_err(checkpoint_error)
    }

    async fn save(&self, relay: &str, seq_nr: i64) -> Result<(), RelayError> {
        sqlx::query(&self.upsert)
            .bind(relay)
            .bind(seq_nr)
            .execute(&self.pool)
            .await
            .map_err(checkpoint_error)?;
        Ok(())
    }
}
