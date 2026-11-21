//! Transaction scopes: a hook that lets an application prepare every
//! transaction the event-sourcing driver opens (for example `SET LOCAL
//! ROLE` and a transaction-local tenant setting for Row-Level Security).

use std::sync::Arc;

use async_trait::async_trait;
use edomata_backend::BackendError;
use sqlx::pool::PoolConnection;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

use crate::error::map_sqlx;

/// Prepares the transactions opened by [`SqlxDriver`](crate::SqlxDriver)
/// once [`SqlxDriver::with_tx_scope`](crate::SqlxDriver::with_tx_scope) is
/// set.
///
/// With a scope, the driver never uses a bare pooled connection: every
/// statement (reads included) runs inside a transaction returned by
/// [`begin`](Self::begin), which is committed (or rolled back on error) by
/// the driver. An implementation opens the transaction and applies its
/// transaction-local settings before returning it, and must fail (never
/// fall back to an unprepared transaction) when it cannot.
///
/// Without a scope, the driver behaves exactly as before. The SaaS
/// Row-Level Security gateway of `edomata-saas-sqlx` is the intended
/// implementation.
///
/// ```
/// use async_trait::async_trait;
/// use edomata_backend::BackendError;
/// use edomata_sqlx::TxScope;
/// use sqlx::{PgPool, Postgres, Transaction};
///
/// #[derive(Debug)]
/// struct ReadCommitted;
///
/// #[async_trait]
/// impl TxScope for ReadCommitted {
///     async fn begin(&self, pool: &PgPool) -> Result<Transaction<'static, Postgres>, BackendError> {
///         let mut tx = pool.begin().await.map_err(BackendError::unknown)?;
///         sqlx::query("set local transaction isolation level read committed")
///             .execute(&mut *tx)
///             .await
///             .map_err(BackendError::unknown)?;
///         Ok(tx)
///     }
/// }
/// ```
#[async_trait]
pub trait TxScope: std::fmt::Debug + Send + Sync + 'static {
    /// Opens a transaction on `pool` and prepares it.
    ///
    /// # Errors
    ///
    /// A [`BackendError`] when the transaction cannot be opened or
    /// prepared (for example, when no access context is available).
    async fn begin(&self, pool: &PgPool) -> Result<Transaction<'static, Postgres>, BackendError>;
}

/// A pool and an optional scope: the only way the driver reaches the
/// database.
#[derive(Clone, Debug)]
pub(crate) struct Db {
    pool: PgPool,
    scope: Option<Arc<dyn TxScope>>,
}

impl Db {
    pub fn new(pool: PgPool, scope: Option<Arc<dyn TxScope>>) -> Self {
        Self { pool, scope }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn is_scoped(&self) -> bool {
        self.scope.is_some()
    }

    /// A transaction (prepared by the scope, if any).
    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>, BackendError> {
        match &self.scope {
            None => self.pool.begin().await.map_err(map_sqlx),
            Some(scope) => scope.begin(&self.pool).await,
        }
    }

    /// A connection for reads: a pooled connection without a scope, a
    /// prepared transaction with one.
    pub async fn reader(&self) -> Result<Reader, BackendError> {
        match &self.scope {
            None => Ok(Reader::Conn(self.pool.acquire().await.map_err(map_sqlx)?)),
            Some(scope) => Ok(Reader::Tx(scope.begin(&self.pool).await?)),
        }
    }
}

/// A connection for reads, see [`Db::reader`].
pub(crate) enum Reader {
    Conn(PoolConnection<Postgres>),
    Tx(Transaction<'static, Postgres>),
}

impl Reader {
    pub fn conn(&mut self) -> &mut PgConnection {
        match self {
            Reader::Conn(c) => c,
            Reader::Tx(t) => t,
        }
    }

    /// Ends the read (commits a scoped transaction).
    pub async fn finish(self) -> Result<(), BackendError> {
        match self {
            Reader::Conn(_) => Ok(()),
            Reader::Tx(t) => t.commit().await.map_err(map_sqlx),
        }
    }
}
