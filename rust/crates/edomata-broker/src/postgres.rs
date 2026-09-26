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
#[derive(Clone, Debug)]
pub struct LeaderLock {
    pool: PgPool,
    key: String,
    health_interval: Duration,
}

impl LeaderLock {
    /// A lock for the relay of `source`.
    pub fn new(pool: PgPool, source: &str) -> Self {
        Self {
            pool,
            key: format!("edomata-relay:{source}"),
            health_interval: Duration::from_secs(5),
        }
    }

    /// A lock with an explicit key.
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

    /// Tries to take the lock without waiting.
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
    /// Whether the lock's connection still answers.
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

    /// Releases the lock and closes the connection.
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
/// (DDL: [`PGSchema::relay_checkpoints`]).
#[derive(Clone, Debug)]
pub struct PgCheckpointStore {
    pool: PgPool,
    naming: PGNaming,
    select: String,
    upsert: String,
}

impl PgCheckpointStore {
    /// A store for the tables of `naming`.
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
